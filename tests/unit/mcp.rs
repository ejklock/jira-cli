use super::*;
use serde_json::json;
use std::collections::BTreeSet;

fn operation_names(schema: &JsonObject) -> Vec<String> {
    schema["properties"]["operation"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect()
}

fn tool_named(name: &str) -> rmcp::model::Tool {
    JiraServer::tool_router()
        .list_all()
        .into_iter()
        .find(|tool| tool.name == name)
        .unwrap_or_else(|| panic!("tool {name} is not listed"))
}

#[test]
fn mcp_tools_list_names_exactly_the_three_tools() {
    let mut names: Vec<String> = JiraServer::tool_router()
        .list_all()
        .into_iter()
        .map(|tool| tool.name.to_string())
        .collect();
    names.sort();
    assert_eq!(
        names,
        ["jira_download_attachments", "jira_read", "jira_write"]
    );
}

#[test]
fn mcp_tools_list_carries_the_annotations_of_each_tool() {
    let read = tool_named("jira_read").annotations.unwrap();
    assert_eq!(read.read_only_hint, Some(true));

    let write = tool_named("jira_write").annotations.unwrap();
    assert_eq!(write.destructive_hint, Some(true));
    assert_ne!(write.read_only_hint, Some(true));

    let download = tool_named("jira_download_attachments").annotations.unwrap();
    assert_eq!(download.read_only_hint, Some(false));
    assert_eq!(download.destructive_hint, Some(false));
    assert_eq!(download.idempotent_hint, Some(true));
}

#[test]
fn mcp_tools_list_publishes_flat_object_schemas_requiring_operation() {
    for tool in JiraServer::tool_router().list_all() {
        let schema = &tool.input_schema;
        assert_eq!(schema["type"], "object", "{} type", tool.name);
        assert_eq!(schema["required"], json!(["operation"]), "{}", tool.name);
        for keyword in ["oneOf", "anyOf", "allOf"] {
            assert!(
                !schema.contains_key(keyword),
                "{} has top-level {keyword}",
                tool.name
            );
        }
    }
}

/// Every operation variant of `T` as the derive describes it before
/// flattening: name, accepted fields, required fields.
fn typed_variants<T: JsonSchema + 'static>() -> Vec<(String, BTreeSet<String>, BTreeSet<String>)> {
    let schema = schema_for_input::<T>().unwrap();
    schema["oneOf"]
        .as_array()
        .expect("the typed enum derives a oneOf of operations")
        .iter()
        .map(|variant| {
            let name = variant["properties"]["operation"]["const"]
                .as_str()
                .unwrap()
                .to_owned();
            let fields = variant["properties"]
                .as_object()
                .unwrap()
                .keys()
                .filter(|field| *field != "operation")
                .cloned()
                .collect();
            let required = variant["required"]
                .as_array()
                .map(|items| items.iter().filter_map(Value::as_str))
                .into_iter()
                .flatten()
                .filter(|field| *field != "operation")
                .map(str::to_owned)
                .collect();
            (name, fields, required)
        })
        .collect()
}

fn assert_flat_schema_matches_typed_enum<T: JsonSchema + 'static>(tool: &str) {
    let flat = tool_named(tool).input_schema;
    let variants = typed_variants::<T>();

    let expected_operations: Vec<String> = variants.iter().map(|(name, ..)| name.clone()).collect();
    assert_eq!(operation_names(&flat), expected_operations, "{tool}");

    let expected_fields: BTreeSet<String> = variants
        .iter()
        .flat_map(|(_, fields, _)| fields.iter().cloned())
        .collect();
    let published: BTreeSet<String> = flat["properties"]
        .as_object()
        .unwrap()
        .keys()
        .filter(|field| *field != "operation")
        .cloned()
        .collect();
    assert_eq!(published, expected_fields, "{tool} published fields");

    for (name, fields, required) in &variants {
        for field in fields {
            let description = flat["properties"][field]["description"]
                .as_str()
                .unwrap_or_else(|| panic!("{tool}.{field} has no description"));
            let label = if required.contains(field) {
                format!("{name} (required)")
            } else {
                name.clone()
            };
            assert!(
                description.contains(&label),
                "{tool}.{field} description must name {label}: {description}"
            );
        }
    }
}

#[test]
fn mcp_schema_of_jira_read_matches_its_typed_operations() {
    assert_flat_schema_matches_typed_enum::<ReadOperation>("jira_read");
    assert_eq!(
        operation_names(&tool_named("jira_read").input_schema),
        ["get", "current", "mine", "search"]
    );
}

#[test]
fn mcp_schema_of_jira_write_matches_its_typed_operations() {
    assert_flat_schema_matches_typed_enum::<WriteOperation>("jira_write");
    assert_eq!(
        operation_names(&tool_named("jira_write").input_schema),
        ["comment"]
    );
}

#[test]
fn mcp_schema_of_jira_download_attachments_matches_its_typed_operations() {
    assert_flat_schema_matches_typed_enum::<DownloadOperation>("jira_download_attachments");
    assert_eq!(
        operation_names(&tool_named("jira_download_attachments").input_schema),
        ["download"]
    );
}

#[test]
fn mcp_schema_flattening_publishes_a_field_only_one_operation_takes() {
    let variant = |name: &str, field: &str| {
        json!({
            "properties": {
                "operation": { "const": name },
                field: { "type": "string" }
            },
            "required": ["operation", field]
        })
    };
    let mut schema = JsonObject::new();
    schema.insert(
        "oneOf".into(),
        json!([variant("first", "alpha"), variant("second", "beta")]),
    );

    flatten_operations(&mut schema);

    assert_eq!(operation_names(&schema), ["first", "second"]);
    assert_eq!(
        schema["properties"]["alpha"]["description"],
        "Taken by: first (required)."
    );
    assert_eq!(
        schema["properties"]["beta"]["description"],
        "Taken by: second (required)."
    );
}

#[test]
#[should_panic(
    expected = "operation second declares field ref differently from an earlier operation"
)]
fn mcp_schema_flattening_rejects_a_field_declared_differently_by_two_operations() {
    let variant = |name: &str, doc: &str| {
        json!({
            "properties": {
                "operation": { "const": name },
                "ref": { "type": "string", "description": doc }
            },
            "required": ["operation", "ref"]
        })
    };
    let mut schema = JsonObject::new();
    schema.insert(
        "oneOf".into(),
        json!([
            variant("first", "Issue key."),
            variant("second", "Issue URL.")
        ]),
    );

    flatten_operations(&mut schema);
}

#[test]
fn mcp_schema_parse_rejects_a_field_the_operation_does_not_take() {
    let arguments = json!({"operation": "comment", "body": "hi", "jql": "x"});
    let error = parse_operation::<WriteOperation>(arguments.as_object().unwrap().clone())
        .err()
        .expect("an extra field is invalid");
    assert_eq!(error.code, rmcp::model::ErrorCode::INVALID_PARAMS);
}
