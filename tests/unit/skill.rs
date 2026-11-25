use super::*;

fn buffers() -> (Vec<u8>, Vec<u8>) {
    (Vec::new(), Vec::new())
}

// --- AC1: named skill prints the full embedded body byte-for-byte ---

#[test]
fn named_skill_prints_full_body_byte_for_byte() {
    let (mut out, mut err) = buffers();
    let code = skill_output(Some("jira-ticket"), &mut out, &mut err);
    assert_eq!(code, 0);
    assert_eq!(
        out,
        include_str!("../../.claude/skills/jira-ticket/SKILL.md").as_bytes()
    );
    assert!(err.is_empty());
}

// --- AC2: list prints name<TAB>description<newline> ---

#[test]
fn list_prints_name_tab_first_sentence_description() {
    let (mut out, mut err) = buffers();
    let code = skill_output(Some("list"), &mut out, &mut err);
    assert_eq!(code, 0);
    let expected = format!("jira-ticket\t{}\n", REGISTRY[0].description);
    assert_eq!(out, expected.as_bytes());
    assert!(err.is_empty());
}

// --- AC3: bare with a single registered skill prints the same body as AC1 ---

#[test]
fn bare_with_single_registered_skill_prints_same_body_as_named() {
    let (mut named_out, mut named_err) = buffers();
    skill_output(Some("jira-ticket"), &mut named_out, &mut named_err);

    let (mut bare_out, mut bare_err) = buffers();
    let code = skill_output(None, &mut bare_out, &mut bare_err);

    assert_eq!(code, 0);
    assert_eq!(bare_out, named_out);
    assert!(bare_err.is_empty());
}

// --- AC4: unknown skill errors to stderr, leaves stdout empty, exits 2 ---

#[test]
fn unknown_skill_errors_and_leaves_stdout_empty() {
    let (mut out, mut err) = buffers();
    let code = skill_output(Some("nope"), &mut out, &mut err);
    assert_eq!(code, 2);
    assert!(out.is_empty());
    let err_text = String::from_utf8(err).unwrap();
    assert!(err_text.contains("unknown skill: nope"));
}

// --- ADR 0030: the retired name `jira` is unknown now, no alias ---

#[test]
fn retired_name_jira_is_now_unknown() {
    let (mut out, mut err) = buffers();
    let code = skill_output(Some("jira"), &mut out, &mut err);
    assert_eq!(code, 2);
    assert!(out.is_empty());
    let err_text = String::from_utf8(err).unwrap();
    assert!(err_text.contains("unknown skill: jira"));
}

// --- edge case: empty-string name behaves as unknown, not as bare/list ---

#[test]
fn empty_string_name_is_treated_as_unknown() {
    let (mut out, mut err) = buffers();
    let code = skill_output(Some(""), &mut out, &mut err);
    assert_eq!(code, 2);
    assert!(out.is_empty());
    let err_text = String::from_utf8(err).unwrap();
    assert!(err_text.contains("unknown skill: "));
}

// --- edge case: body is never truncated with an extra trailing newline ---

#[test]
fn named_skill_body_has_no_extra_trailing_newline() {
    let (mut out, mut err) = buffers();
    skill_output(Some("jira-ticket"), &mut out, &mut err);
    let printed = String::from_utf8(out).unwrap();
    let source = include_str!("../../.claude/skills/jira-ticket/SKILL.md");
    assert_eq!(printed.len(), source.len());
}

// --- the body tells the agent to prefer the MCP tools, else use the CLI ---

fn served_body() -> String {
    let (mut out, mut err) = buffers();
    skill_output(Some("jira-ticket"), &mut out, &mut err);
    String::from_utf8(out).unwrap()
}

fn section_after(body: &str, heading: &str) -> String {
    let start = body
        .find(heading)
        .unwrap_or_else(|| panic!("missing heading: {heading}"))
        + heading.len();
    let rest = &body[start..];
    let end = rest.find("\n## ").unwrap_or(rest.len());
    rest[..end].to_string()
}

#[test]
fn body_prefers_mcp_tools_then_cli_before_commands() {
    let body = served_body();
    let p = body
        .find("\n## Prefer the MCP tools\n")
        .expect("missing heading: Prefer the MCP tools");
    let o = body
        .find("\n## Otherwise use the CLI\n")
        .expect("missing heading: Otherwise use the CLI");
    let c = body
        .find("\n## Commands\n")
        .expect("missing heading: Commands");
    assert!(p < o && o < c);
}

#[test]
fn prefer_section_names_connected_server_and_all_three_tools() {
    let section = section_after(&served_body(), "\n## Prefer the MCP tools\n");
    for needle in [
        "connected",
        "`jira_read`",
        "`jira_write`",
        "`jira_download_attachments`",
    ] {
        assert!(section.contains(needle), "prefer section lacks {needle}");
    }
}

#[test]
fn otherwise_section_runs_cli_with_json_flag() {
    let section = section_after(&served_body(), "\n## Otherwise use the CLI\n");
    assert!(section.contains("--json"));
}
