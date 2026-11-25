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

    if variants.len() == 1 {
        return;
    }
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

fn all_descriptions(schema: &JsonObject) -> Vec<(String, String)> {
    schema["properties"]
        .as_object()
        .unwrap()
        .iter()
        .filter_map(|(field, value)| {
            value["description"]
                .as_str()
                .map(|text| (field.clone(), text.to_owned()))
        })
        .collect()
}

#[test]
fn mcp_tools_list_stays_under_the_size_budget() {
    let tools = JiraServer::tool_router().list_all();
    let size = serde_json::to_string(&tools).unwrap().len();
    assert!(size < 2600, "tools/list is {size} bytes");
}

fn assert_read_notes_stay(schema: &JsonObject) {
    for (field, text) in all_descriptions(schema) {
        if field != "operation" {
            assert!(text.contains("Taken by"), "read.{field}: {text}");
        }
    }
    for field in ["jql", "ref"] {
        let text = schema["properties"][field]["description"].as_str().unwrap();
        assert!(text.contains("(required)"), "read.{field}: {text}");
    }
}

fn assert_single_operation_is_not_repeated(name: &str, schema: &JsonObject) {
    for (field, text) in all_descriptions(schema) {
        assert!(!text.contains("Taken by"), "{name}.{field}: {text}");
    }
    let operation = schema["properties"]["operation"]["description"]
        .as_str()
        .unwrap_or_default();
    assert!(!operation.contains("The command to run."), "{name}");
}

#[test]
fn mcp_tools_list_marks_only_the_required_fields_of_a_single_operation_tool() {
    let description = |tool: &str, field: &str| {
        tool_named(tool).input_schema["properties"][field]["description"]
            .as_str()
            .unwrap_or_default()
            .to_owned()
    };
    assert!(description("jira_write", "body").contains("(required)"));
    assert!(description("jira_download_attachments", "ref").contains("(required)"));
    for (tool, field) in [
        ("jira_write", "ref"),
        ("jira_write", "instance"),
        ("jira_download_attachments", "directory"),
        ("jira_download_attachments", "instance"),
    ] {
        let text = description(tool, field);
        assert!(!text.contains("Taken by"), "{tool}.{field}: {text}");
        assert!(!text.contains("(required)"), "{tool}.{field}: {text}");
    }
}

#[test]
fn mcp_tools_list_drops_repeated_schema_text() {
    for tool in JiraServer::tool_router().list_all() {
        let schema = &tool.input_schema;
        assert!(!schema.contains_key("$schema"), "{} has $schema", tool.name);
        let mut texts = all_descriptions(schema);
        texts.push(("tool".into(), tool.description.clone().unwrap().to_string()));
        for (field, text) in &texts {
            assert!(!text.contains('\n'), "{}.{field} has a newline", tool.name);
        }
        if tool.name == "jira_read" {
            assert_read_notes_stay(schema);
        } else {
            assert_single_operation_is_not_repeated(&tool.name, schema);
        }
    }
}

#[test]
fn mcp_tools_list_keeps_every_field_type_default_and_enum() {
    let expected = json!({
        "jira_read": {
            "operation": {"type": "string", "enum": ["get", "current", "mine", "search"]},
            "branch": {"type": ["string", "null"]},
            "instance": {"type": ["string", "null"]},
            "jql": {"type": "string"},
            "limit": {"type": ["integer", "null"], "format": "uint64", "minimum": 0},
            "no_comments": {"type": "boolean", "default": false},
            "ref": {"type": "string"},
            "refresh": {"type": "boolean", "default": false}
        },
        "jira_write": {
            "operation": {"type": "string", "enum": ["comment"]},
            "body": {"type": "string"},
            "instance": {"type": ["string", "null"]},
            "ref": {"type": ["string", "null"]}
        },
        "jira_download_attachments": {
            "operation": {"type": "string", "enum": ["download"]},
            "directory": {"type": ["string", "null"]},
            "instance": {"type": ["string", "null"]},
            "ref": {"type": "string"}
        }
    });
    for (tool, fields) in expected.as_object().unwrap() {
        let schema = tool_named(tool).input_schema;
        let published = schema["properties"].as_object().unwrap();
        let names: BTreeSet<&String> = published.keys().collect();
        let wanted: BTreeSet<&String> = fields.as_object().unwrap().keys().collect();
        assert_eq!(names, wanted, "{tool} fields");
        assert_eq!(schema["required"], json!(["operation"]), "{tool}");
        for (field, keywords) in fields.as_object().unwrap() {
            for (keyword, value) in keywords.as_object().unwrap() {
                assert_eq!(
                    &published[field][keyword], value,
                    "{tool}.{field}.{keyword}"
                );
            }
            for keyword in ["default", "minimum", "format"] {
                assert_eq!(
                    published[field].get(keyword),
                    keywords.get(keyword),
                    "{tool}.{field}.{keyword}"
                );
            }
        }
    }
}

#[test]
fn frontmatter_reader_returns_name_and_description_of_the_embedded_skill() {
    let entry = &crate::skill::REGISTRY[0];

    let parsed = skills::parse_frontmatter(entry.body).expect("the embedded skill parses");

    assert_eq!(parsed.name, "jira-ticket");
    assert!(
        parsed.description.starts_with(entry.description),
        "{}",
        parsed.description
    );
}

#[test]
fn frontmatter_reader_refuses_text_that_is_not_a_named_described_skill() {
    for text in [
        "name: n\ndescription: d\n---\nbody\n",
        "---\nname: n\ndescription: d\nbody\n",
        "---\ndescription: d\n---\n",
        "---\nname: n\n---\n",
        "---\nname:\ndescription: d\n---\n",
        "---\nname: n\ndescription:   \n---\n",
    ] {
        assert!(skills::parse_frontmatter(text).is_err(), "{text:?}");
    }
}

#[test]
fn frontmatter_reader_keeps_everything_after_the_first_key_separator() {
    let text = "---\nname: n\ndescription: use it: a: b\n---\nbody\n";

    let parsed = skills::parse_frontmatter(text).expect("the frontmatter parses");

    assert_eq!(parsed.description, "use it: a: b");
}

fn read_op(value: Value) -> ReadOperation {
    serde_json::from_value(value).unwrap()
}

fn search_with(limit: Option<u64>) -> ReadOperation {
    match limit {
        Some(limit) => {
            read_op(json!({"operation": "search", "jql": "project = P", "limit": limit}))
        }
        None => read_op(json!({"operation": "search", "jql": "project = P"})),
    }
}

#[test]
fn a_download_and_a_search_above_the_default_limit_are_tasks_for_a_declaring_client() {
    use tasks::{runs_as_task, Call};
    assert!(runs_as_task(true, &Call::Download));
    assert!(runs_as_task(true, &Call::Read(&search_with(Some(51)))));
    assert!(runs_as_task(true, &Call::Read(&search_with(Some(500)))));
}

#[test]
fn a_search_at_or_below_the_default_limit_or_without_one_stays_synchronous() {
    use tasks::{runs_as_task, Call};
    for limit in [Some(50), Some(0), Some(1), None] {
        assert!(
            !runs_as_task(true, &Call::Read(&search_with(limit))),
            "{limit:?}"
        );
    }
}

#[test]
fn get_current_and_mine_stay_synchronous_whatever_their_size() {
    use tasks::{runs_as_task, Call};
    let reads = [
        read_op(json!({"operation": "get", "ref": "some-issue"})),
        read_op(json!({"operation": "current"})),
        read_op(json!({"operation": "mine", "limit": 500})),
    ];
    for read in &reads {
        assert!(!runs_as_task(true, &Call::Read(read)));
    }
}

#[test]
fn a_client_without_the_extension_never_gets_a_task() {
    use tasks::{runs_as_task, Call};
    assert!(!runs_as_task(false, &Call::Download));
    assert!(!runs_as_task(false, &Call::Read(&search_with(Some(500)))));
}
