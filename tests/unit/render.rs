use super::*;
use crate::i18n::{set_language, t, LANG_MUTEX};
use crate::models::{Issue, IssueAssignee, IssueComment, IssueRow};

fn sample_issue() -> Issue {
    Issue {
        key: "PROJ-123".to_string(),
        summary: "Fix the login bug".to_string(),
        status: "In Progress".to_string(),
        status_category: Some("indeterminate".to_string()),
        issue_type: "Bug".to_string(),
        assignee: Some(IssueAssignee {
            display_name: "Alice Example".to_string(),
            account_id: Some("5b10a".to_string()),
        }),
        reporter: Some(IssueAssignee {
            display_name: "Charlie".to_string(),
            account_id: Some("rep-42".to_string()),
        }),
        priority: Some("High".to_string()),
        created: Some("2026-01-10T09:00:00.000+0000".to_string()),
        updated: Some("2026-06-29T12:00:00.000+0000".to_string()),
        description: Some(r#"{"type":"doc","version":1,"content":[{"type":"paragraph","content":[{"type":"text","text":"Login fails when MFA is enabled."}]}]}"#.to_string()),
        comments: vec![IssueComment {
            id: Some("100".to_string()),
            author: Some("Bob Dev".to_string()),
            body: "Reproduced on v2.1.".to_string(),
            created: Some("2026-06-29T10:00:00.000+0000".to_string()),
            updated: None,
        }],
    }
}

fn issue_no_comments() -> Issue {
    Issue {
        comments: vec![],
        ..sample_issue()
    }
}

// --- adf_to_plain_text ---

#[test]
fn adf_to_plain_text_paragraph_extracts_text() {
    let adf = r#"{"type":"doc","version":1,"content":[{"type":"paragraph","content":[{"type":"text","text":"Hello world"}]}]}"#;
    let result = adf_to_plain_text(adf);
    assert_eq!(result, "Hello world");
}

#[test]
fn adf_to_plain_text_multiple_paragraphs_separated_by_newline() {
    let adf = r#"{"type":"doc","version":1,"content":[{"type":"paragraph","content":[{"type":"text","text":"First"}]},{"type":"paragraph","content":[{"type":"text","text":"Second"}]}]}"#;
    let result = adf_to_plain_text(adf);
    assert!(result.contains("First"), "must contain First: {result}");
    assert!(result.contains("Second"), "must contain Second: {result}");
    assert!(
        result.contains('\n'),
        "paragraphs must be separated by newline"
    );
}

#[test]
fn adf_to_plain_text_hard_break_produces_newline() {
    let adf = r#"{"type":"doc","version":1,"content":[{"type":"paragraph","content":[{"type":"text","text":"Line 1"},{"type":"hardBreak"},{"type":"text","text":"Line 2"}]}]}"#;
    let result = adf_to_plain_text(adf);
    assert!(result.contains("Line 1"), "must have Line 1: {result}");
    assert!(result.contains("Line 2"), "must have Line 2: {result}");
    assert!(result.contains('\n'), "hardBreak must produce newline");
}

#[test]
fn adf_to_plain_text_bullet_list_produces_dash_items() {
    let adf = r#"{"type":"doc","version":1,"content":[{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"Item A"}]}]},{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"Item B"}]}]}]}]}"#;
    let result = adf_to_plain_text(adf);
    assert!(result.contains("Item A"), "must contain Item A: {result}");
    assert!(result.contains("Item B"), "must contain Item B: {result}");
    assert!(
        result.contains("- "),
        "bullet items must have dash prefix: {result}"
    );
}

#[test]
fn adf_to_plain_text_ordered_list_produces_numbered_items() {
    let adf = r#"{"type":"doc","version":1,"content":[{"type":"orderedList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"Item A"}]}]},{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"Item B"}]}]},{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"Item C"}]}]}]}]}"#;
    let result = adf_to_plain_text(adf);
    assert!(result.contains("1. Item A"), "must be numbered 1: {result}");
    assert!(result.contains("2. Item B"), "must be numbered 2: {result}");
    assert!(result.contains("3. Item C"), "must be numbered 3: {result}");
    assert!(
        !result.contains("- Item"),
        "ordered items must not have dash prefix: {result}"
    );
}

#[test]
fn adf_to_plain_text_nested_ordered_list_numbers_independently() {
    let adf = r#"{"type":"doc","version":1,"content":[{"type":"orderedList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"Outer A"}]},{"type":"orderedList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"Inner A"}]}]},{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"Inner B"}]}]}]}]},{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"Outer B"}]}]}]}]}"#;
    let result = adf_to_plain_text(adf);
    assert!(result.contains("1. Outer A"), "outer item 1: {result}");
    assert!(result.contains("2. Outer B"), "outer item 2: {result}");
    assert!(
        result.contains("1. Inner A"),
        "nested list must restart at 1: {result}"
    );
    assert!(result.contains("2. Inner B"), "nested item 2: {result}");
}

#[test]
fn adf_to_plain_text_non_adf_string_returned_as_is() {
    let plain = "just plain text";
    assert_eq!(adf_to_plain_text(plain), plain);
}

#[test]
fn adf_to_plain_text_empty_doc_returns_empty() {
    let adf = r#"{"type":"doc","version":1,"content":[]}"#;
    let result = adf_to_plain_text(adf);
    assert_eq!(result, "");
}

#[test]
fn adf_to_plain_text_unknown_node_falls_back_to_text_content() {
    let adf = r#"{"type":"doc","version":1,"content":[{"type":"customNode","content":[{"type":"paragraph","content":[{"type":"text","text":"buried text"}]}]}]}"#;
    let result = adf_to_plain_text(adf);
    assert!(
        result.contains("buried text"),
        "unknown node must fall back to child text: {result}"
    );
}

#[test]
fn adf_to_plain_text_heading_extracts_text() {
    let adf = r#"{"type":"doc","version":1,"content":[{"type":"heading","attrs":{"level":2},"content":[{"type":"text","text":"Section Title"}]}]}"#;
    let result = adf_to_plain_text(adf);
    assert!(
        result.contains("Section Title"),
        "heading text must be extracted: {result}"
    );
}

// --- render_issue_human ---

#[test]
fn render_issue_human_includes_summary() {
    let mut out = Vec::new();
    render_issue_human(
        &sample_issue(),
        "work",
        "https://work.atlassian.net",
        false,
        &mut out,
    );
    let text = std::str::from_utf8(&out).unwrap();
    assert!(
        text.contains("Fix the login bug"),
        "must include summary: {text}"
    );
}

#[test]
fn render_issue_human_includes_issue_key() {
    let mut out = Vec::new();
    render_issue_human(
        &sample_issue(),
        "work",
        "https://work.atlassian.net",
        false,
        &mut out,
    );
    let text = std::str::from_utf8(&out).unwrap();
    assert!(text.contains("PROJ-123"), "must include issue key: {text}");
}

#[test]
fn render_issue_human_includes_browse_url() {
    let mut out = Vec::new();
    render_issue_human(
        &sample_issue(),
        "work",
        "https://work.atlassian.net",
        false,
        &mut out,
    );
    let text = std::str::from_utf8(&out).unwrap();
    assert!(
        text.contains("https://work.atlassian.net/browse/PROJ-123"),
        "must include browse URL: {text}"
    );
}

#[test]
fn render_issue_human_includes_status() {
    let mut out = Vec::new();
    render_issue_human(
        &sample_issue(),
        "work",
        "https://work.atlassian.net",
        false,
        &mut out,
    );
    let text = std::str::from_utf8(&out).unwrap();
    assert!(text.contains("In Progress"), "must include status: {text}");
}

#[test]
fn render_issue_human_includes_assignee() {
    let mut out = Vec::new();
    render_issue_human(
        &sample_issue(),
        "work",
        "https://work.atlassian.net",
        false,
        &mut out,
    );
    let text = std::str::from_utf8(&out).unwrap();
    assert!(
        text.contains("Alice Example"),
        "must include assignee: {text}"
    );
}

#[test]
fn render_issue_human_flattens_adf_description() {
    let mut out = Vec::new();
    render_issue_human(
        &sample_issue(),
        "work",
        "https://work.atlassian.net",
        false,
        &mut out,
    );
    let text = std::str::from_utf8(&out).unwrap();
    assert!(
        text.contains("Login fails when MFA is enabled."),
        "ADF description must be flattened to plain text: {text}"
    );
}

#[test]
fn render_issue_human_includes_comments_by_default() {
    let mut out = Vec::new();
    render_issue_human(
        &sample_issue(),
        "work",
        "https://work.atlassian.net",
        false,
        &mut out,
    );
    let text = std::str::from_utf8(&out).unwrap();
    assert!(
        text.contains("Bob Dev"),
        "must include comment author: {text}"
    );
    assert!(
        text.contains("Reproduced on v2.1."),
        "must include comment body: {text}"
    );
}

#[test]
fn render_issue_human_no_comments_flag_suppresses_comments() {
    let issue = sample_issue();
    let mut with_comments = Vec::new();
    render_issue_human(
        &issue,
        "work",
        "https://work.atlassian.net",
        false,
        &mut with_comments,
    );
    let mut without_comments = Vec::new();
    render_issue_human(
        &issue,
        "work",
        "https://work.atlassian.net",
        true,
        &mut without_comments,
    );

    let with_text = std::str::from_utf8(&with_comments).unwrap();
    let without_text = std::str::from_utf8(&without_comments).unwrap();
    assert!(
        with_text.contains("Bob Dev"),
        "with comments must include Bob Dev"
    );
    assert!(
        !without_text.contains("Bob Dev"),
        "no-comments must suppress comment body"
    );
}

#[test]
fn render_issue_human_empty_comments_no_comments_section() {
    let mut out = Vec::new();
    render_issue_human(
        &issue_no_comments(),
        "work",
        "https://work.atlassian.net",
        false,
        &mut out,
    );
    let text = std::str::from_utf8(&out).unwrap();
    assert!(
        !text.contains("Bob Dev"),
        "no comments means no comment author: {text}"
    );
}

#[test]
fn render_issue_human_pt_br_translates_field_labels() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("pt_BR");
    let issue = Issue {
        assignee: None,
        ..sample_issue()
    };
    let mut out = Vec::new();
    render_issue_human(
        &issue,
        "work",
        "https://work.atlassian.net",
        false,
        &mut out,
    );
    let text = std::str::from_utf8(&out).unwrap();
    assert!(text.contains("Tipo:"), "must contain Tipo: {text}");
    assert!(
        text.contains("Prioridade:"),
        "must contain Prioridade: {text}"
    );
    assert!(
        text.contains("Responsável:"),
        "must contain Responsável: {text}"
    );
    assert!(text.contains("Relator:"), "must contain Relator: {text}");
    assert!(text.contains("Criado:"), "must contain Criado: {text}");
    assert!(
        text.contains("Atualizado:"),
        "must contain Atualizado: {text}"
    );
    assert!(
        text.contains("Descrição:"),
        "must contain Descrição: {text}"
    );
    assert!(
        text.contains("Comentários:"),
        "must contain Comentários: {text}"
    );
    assert!(
        text.contains("Não atribuído"),
        "unassigned issue must show Não atribuído: {text}"
    );
    set_language("en");
}

#[test]
fn render_issue_human_en_labels_unchanged() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");
    let mut out = Vec::new();
    render_issue_human(
        &sample_issue(),
        "work",
        "https://work.atlassian.net",
        false,
        &mut out,
    );
    let text = std::str::from_utf8(&out).unwrap();
    assert!(text.contains("Status:"), "en label must be Status: {text}");
    assert!(text.contains("Type:"), "en label must be Type: {text}");
    assert!(
        text.contains("Assignee:"),
        "en label must be Assignee: {text}"
    );
    assert!(
        text.contains("Description:"),
        "en label must be Description: {text}"
    );
    assert!(
        text.contains("Comments:"),
        "en label must be Comments: {text}"
    );
    set_language("en");
}

#[test]
fn print_error_does_not_panic_on_empty_string() {
    print_error("");
}

#[test]
fn print_error_does_not_panic_on_normal_message() {
    print_error("Error: something went wrong");
}

// --- link_segments (kept for API compatibility) ---

#[test]
fn link_segments_no_url_returns_single_non_link_segment() {
    let segs = link_segments("plain text with no URL");
    assert_eq!(segs.len(), 1);
    assert!(!segs[0].is_link);
    assert_eq!(segs[0].text, "plain text with no URL");
}

#[test]
fn link_segments_https_url_splits_into_three_ordered_segments() {
    let line = "See https://example.com/path for details";
    let segs = link_segments(line);
    assert_eq!(
        segs.len(),
        3,
        "expected [before][url][after]: got {} segs",
        segs.len()
    );
    assert!(!segs[0].is_link, "prefix must be non-link");
    assert!(segs[1].is_link, "url must be link");
    assert!(!segs[2].is_link, "suffix must be non-link");
    assert_eq!(segs[0].text, "See ");
    assert_eq!(segs[1].text, "https://example.com/path");
    assert_eq!(segs[2].text, " for details");
}

#[test]
fn link_segments_empty_line_returns_single_non_link_segment() {
    let segs = link_segments("");
    assert_eq!(segs.len(), 1);
    assert!(!segs[0].is_link);
}

// --- render_issue_table ---

fn make_issue_row(
    key: &str,
    issue_type: &str,
    status: &str,
    assignee: Option<&str>,
    summary: &str,
) -> IssueRow {
    IssueRow {
        key: key.to_string(),
        issue_type: issue_type.to_string(),
        summary: summary.to_string(),
        status: status.to_string(),
        assignee: assignee.map(|s| s.to_string()),
    }
}

#[test]
fn render_issue_table_prints_header_with_required_columns() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");
    let rows = vec![make_issue_row(
        "PROJ-1",
        "Task",
        "Open",
        Some("Alice"),
        "Do the thing",
    )];
    let mut out = Vec::new();
    render_issue_table(&mut out, &rows);
    let text = std::str::from_utf8(&out).unwrap();
    assert!(text.contains("KEY"), "header must contain KEY: {text}");
    assert!(text.contains("TYPE"), "header must contain TYPE: {text}");
    assert!(
        text.contains("STATUS"),
        "header must contain STATUS: {text}"
    );
    assert!(
        text.contains("ASSIGNEE"),
        "header must contain ASSIGNEE: {text}"
    );
    assert!(
        text.contains("SUMMARY"),
        "header must contain SUMMARY: {text}"
    );
    set_language("en");
}

#[test]
fn render_issue_table_row_contains_all_fields() {
    let rows = vec![make_issue_row(
        "PROJ-42",
        "Bug",
        "In Progress",
        Some("Bob"),
        "Fix the crash",
    )];
    let mut out = Vec::new();
    render_issue_table(&mut out, &rows);
    let text = std::str::from_utf8(&out).unwrap();
    assert!(text.contains("PROJ-42"), "row must contain key: {text}");
    assert!(text.contains("Bug"), "row must contain type: {text}");
    assert!(
        text.contains("In Progress"),
        "row must contain status: {text}"
    );
    assert!(text.contains("Bob"), "row must contain assignee: {text}");
    assert!(
        text.contains("Fix the crash"),
        "row must contain summary: {text}"
    );
}

#[test]
fn render_issue_table_unassigned_renders_as_unassigned_label() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");
    let rows = vec![make_issue_row(
        "PROJ-5",
        "Story",
        "Open",
        None,
        "Unowned work",
    )];
    let mut out = Vec::new();
    render_issue_table(&mut out, &rows);
    let text = std::str::from_utf8(&out).unwrap();
    assert!(
        text.contains("Unassigned"),
        "None assignee must render as Unassigned: {text}"
    );
    set_language("en");
}

#[test]
fn render_issue_table_multiple_rows_each_rendered() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");
    let rows = vec![
        make_issue_row("PROJ-1", "Task", "Open", Some("Alice"), "First task"),
        make_issue_row("PROJ-2", "Bug", "Done", None, "Second task"),
    ];
    let mut out = Vec::new();
    render_issue_table(&mut out, &rows);
    let text = std::str::from_utf8(&out).unwrap();
    assert!(text.contains("PROJ-1"), "must contain first key: {text}");
    assert!(text.contains("PROJ-2"), "must contain second key: {text}");
    assert!(
        text.contains("First task"),
        "must contain first summary: {text}"
    );
    assert!(
        text.contains("Second task"),
        "must contain second summary: {text}"
    );
    assert!(
        text.contains("Unassigned"),
        "None assignee must render as Unassigned: {text}"
    );
    set_language("en");
}

#[test]
fn render_issue_table_headers_remain_english_under_en() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");
    let rows = vec![make_issue_row(
        "PROJ-1",
        "Task",
        "Open",
        Some("Alice"),
        "Do the thing",
    )];
    let mut out = Vec::new();
    render_issue_table(&mut out, &rows);
    let text = std::str::from_utf8(&out).unwrap();
    assert!(text.contains("KEY"), "en header must be KEY: {text}");
    assert!(
        text.contains("SUMMARY"),
        "en header must be SUMMARY: {text}"
    );
    assert!(
        text.contains("ASSIGNEE"),
        "en header must be ASSIGNEE: {text}"
    );
    set_language("en");
}

#[test]
fn render_issue_table_headers_translated_under_pt_br() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("pt_BR");
    let rows = vec![make_issue_row(
        "PROJ-1",
        "Task",
        "Open",
        Some("Alice"),
        "Do the thing",
    )];
    let mut out = Vec::new();
    render_issue_table(&mut out, &rows);
    let text = std::str::from_utf8(&out).unwrap();
    assert!(text.contains("CHAVE"), "pt_BR header must be CHAVE: {text}");
    assert!(
        text.contains("RESUMO"),
        "pt_BR header must be RESUMO: {text}"
    );
    assert!(
        text.contains("RESPONSÁVEL"),
        "pt_BR header must be RESPONSÁVEL: {text}"
    );
    set_language("en");
}

#[test]
fn render_issue_table_jira_data_not_translated_under_pt_br() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("pt_BR");
    let rows = vec![make_issue_row(
        "PROJ-99",
        "Bug",
        "Open",
        Some("Alice"),
        "Some task",
    )];
    let mut out = Vec::new();
    render_issue_table(&mut out, &rows);
    let text = std::str::from_utf8(&out).unwrap();
    assert!(
        text.contains("CHAVE"),
        "header must be translated to CHAVE under pt_BR: {text}"
    );
    assert!(
        text.contains("Open"),
        "Jira data status 'Open' must NOT be translated even though 'Open' exists in catalog: {text}"
    );
    assert!(
        !text.contains("Aberto"),
        "data row must never show 'Aberto' (catalog translation of 'Open'): {text}"
    );
    set_language("en");
}

#[test]
fn render_issue_table_none_assignee_translated_under_pt_br() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("pt_BR");
    let rows = vec![make_issue_row(
        "PROJ-5",
        "Story",
        "Open",
        None,
        "Unowned work",
    )];
    let mut out = Vec::new();
    render_issue_table(&mut out, &rows);
    let text = std::str::from_utf8(&out).unwrap();
    assert!(
        text.contains("Não atribuído"),
        "None assignee must render as 'Não atribuído' under pt_BR: {text}"
    );
    set_language("en");
}

#[test]
fn chrome_keys_resolve_to_non_identity_translations_under_pt_br() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("pt_BR");
    assert_eq!(t("KEY"), "CHAVE", "KEY must translate to CHAVE under pt_BR");
    assert_eq!(t("TYPE"), "TIPO", "TYPE must translate to TIPO under pt_BR");
    assert_eq!(
        t("STATUS"),
        "STATUS",
        "STATUS must be present in catalog under pt_BR"
    );
    assert_eq!(
        t("ASSIGNEE"),
        "RESPONSÁVEL",
        "ASSIGNEE must translate to RESPONSÁVEL under pt_BR"
    );
    assert_eq!(
        t("SUMMARY"),
        "RESUMO",
        "SUMMARY must translate to RESUMO under pt_BR"
    );
    assert_eq!(
        t("Unassigned"),
        "Não atribuído",
        "Unassigned must translate to Não atribuído under pt_BR"
    );
    assert_eq!(
        t("No issues."),
        "Nenhuma issue encontrada.",
        "No issues. must translate under pt_BR"
    );
    assert_eq!(
        t("Error: search requires a JQL query."),
        "Erro: é necessário informar uma consulta JQL.",
        "search error must translate under pt_BR"
    );
    assert_eq!(
        t("Error: not in a git repository / no current branch."),
        "Erro: fora de um repositório git / sem branch atual.",
        "no-branch error must translate under pt_BR"
    );
    set_language("en");
}
