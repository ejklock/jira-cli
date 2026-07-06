use super::*;
use crate::i18n::{set_language, t, LANG_MUTEX};
use crate::models::{Issue, IssueRow};
use crate::test_support::*;

fn sample_issue() -> Issue {
    Issue {
        summary: "Fix the login bug".to_string(),
        status: "In Progress".to_string(),
        assignee: Some(assignee("Alice Example", Some("5b10a"))),
        description: Some(plain_paragraph("Login fails when MFA is enabled.")),
        comments: vec![comment(
            Some("100"),
            Some("Bob Dev"),
            "Reproduced on v2.1.",
            Some("2026-06-29T10:00:00.000+0000"),
            None,
        )],
        ..issue("PROJ-123")
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
    let adf = doc(vec![paragraph(vec![text("Hello world")])]);
    let result = adf_to_plain_text(&adf);
    assert_eq!(result, "Hello world");
}

#[test]
fn adf_to_plain_text_multiple_paragraphs_separated_by_newline() {
    let adf = doc(vec![
        paragraph(vec![text("First")]),
        paragraph(vec![text("Second")]),
    ]);
    let result = adf_to_plain_text(&adf);
    assert!(result.contains("First"), "must contain First: {result}");
    assert!(result.contains("Second"), "must contain Second: {result}");
    assert!(
        result.contains('\n'),
        "paragraphs must be separated by newline"
    );
}

#[test]
fn adf_to_plain_text_hard_break_produces_newline() {
    let adf = doc(vec![paragraph(vec![
        text("Line 1"),
        hard_break(),
        text("Line 2"),
    ])]);
    let result = adf_to_plain_text(&adf);
    assert!(result.contains("Line 1"), "must have Line 1: {result}");
    assert!(result.contains("Line 2"), "must have Line 2: {result}");
    assert!(result.contains('\n'), "hardBreak must produce newline");
}

#[test]
fn adf_to_plain_text_bullet_list_produces_dash_items() {
    let adf = doc(vec![bullet_list(vec![
        list_item(vec![paragraph(vec![text("Item A")])]),
        list_item(vec![paragraph(vec![text("Item B")])]),
    ])]);
    let result = adf_to_plain_text(&adf);
    assert!(result.contains("Item A"), "must contain Item A: {result}");
    assert!(result.contains("Item B"), "must contain Item B: {result}");
    assert!(
        result.contains("- "),
        "bullet items must have dash prefix: {result}"
    );
}

#[test]
fn adf_to_plain_text_ordered_list_produces_numbered_items() {
    let adf = doc(vec![ordered_list(vec![
        list_item(vec![paragraph(vec![text("Item A")])]),
        list_item(vec![paragraph(vec![text("Item B")])]),
        list_item(vec![paragraph(vec![text("Item C")])]),
    ])]);
    let result = adf_to_plain_text(&adf);
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
    let adf = doc(vec![ordered_list(vec![
        list_item(vec![
            paragraph(vec![text("Outer A")]),
            ordered_list(vec![
                list_item(vec![paragraph(vec![text("Inner A")])]),
                list_item(vec![paragraph(vec![text("Inner B")])]),
            ]),
        ]),
        list_item(vec![paragraph(vec![text("Outer B")])]),
    ])]);
    let result = adf_to_plain_text(&adf);
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
    let adf = doc(vec![]);
    let result = adf_to_plain_text(&adf);
    assert_eq!(result, "");
}

#[test]
fn adf_to_plain_text_unknown_node_falls_back_to_text_content() {
    let adf = doc(vec![custom_node(
        "customNode",
        vec![paragraph(vec![text("buried text")])],
    )]);
    let result = adf_to_plain_text(&adf);
    assert!(
        result.contains("buried text"),
        "unknown node must fall back to child text: {result}"
    );
}

#[test]
fn adf_to_plain_text_heading_extracts_text() {
    let adf = doc(vec![heading(2, vec![text("Section Title")])]);
    let result = adf_to_plain_text(&adf);
    assert!(
        result.contains("Section Title"),
        "heading text must be extracted: {result}"
    );
}

// --- adf_to_rich ---

fn only_span(lines: &[RichLine]) -> &RichSpan {
    assert_eq!(lines.len(), 1, "expected a single line: {lines:?}");
    assert_eq!(lines[0].len(), 1, "expected a single run: {lines:?}");
    &lines[0][0]
}

#[test]
fn adf_to_rich_strong_mark_sets_bold_style() {
    let adf = marked_paragraph("Bold text", vec![mark("strong")]);
    let lines = adf_to_rich(&adf);
    let span = only_span(&lines);
    assert_eq!(span.text, "Bold text");
    assert!(span.style.bold, "strong mark must set bold: {span:?}");
    assert!(!span.style.italic);
}

#[test]
fn adf_to_rich_em_mark_sets_italic_style() {
    let adf = marked_paragraph("Italic text", vec![mark("em")]);
    let lines = adf_to_rich(&adf);
    let span = only_span(&lines);
    assert!(span.style.italic, "em mark must set italic: {span:?}");
    assert!(!span.style.bold);
}

#[test]
fn adf_to_rich_code_mark_sets_code_style() {
    let adf = marked_paragraph("a_var", vec![mark("code")]);
    let lines = adf_to_rich(&adf);
    let span = only_span(&lines);
    assert!(span.style.code, "code mark must set code: {span:?}");
}

#[test]
fn adf_to_rich_strike_mark_sets_strike_style() {
    let adf = marked_paragraph("gone", vec![mark("strike")]);
    let lines = adf_to_rich(&adf);
    let span = only_span(&lines);
    assert!(span.style.strike, "strike mark must set strike: {span:?}");
}

#[test]
fn adf_to_rich_underline_mark_sets_underline_style() {
    let adf = marked_paragraph("under", vec![mark("underline")]);
    let lines = adf_to_rich(&adf);
    let span = only_span(&lines);
    assert!(
        span.style.underline,
        "underline mark must set underline: {span:?}"
    );
    assert!(span.style.link.is_none());
}

#[test]
fn adf_to_rich_link_mark_sets_href_and_underline() {
    let adf = marked_paragraph("click here", vec![link_mark("https://example.com")]);
    let lines = adf_to_rich(&adf);
    let span = only_span(&lines);
    assert_eq!(
        span.style.link.as_deref(),
        Some("https://example.com"),
        "link mark must retain href: {span:?}"
    );
    assert!(
        span.style.underline,
        "link mark must also set underline: {span:?}"
    );
}

#[test]
fn adf_to_rich_composes_multiple_marks_on_one_run() {
    let adf = marked_paragraph("combo", vec![mark("strong"), mark("em"), mark("strike")]);
    let lines = adf_to_rich(&adf);
    let span = only_span(&lines);
    assert!(span.style.bold, "must be bold: {span:?}");
    assert!(span.style.italic, "must be italic: {span:?}");
    assert!(span.style.strike, "must be strike: {span:?}");
    assert!(!span.style.code);
    assert!(!span.style.underline);
}

#[test]
fn adf_to_rich_unmarked_text_run_yields_default_style() {
    let adf = plain_paragraph("plain");
    let lines = adf_to_rich(&adf);
    let span = only_span(&lines);
    assert_eq!(span.style, RichStyle::default());
}

#[test]
fn adf_to_rich_multiple_paragraphs_produce_separate_lines() {
    let adf = doc(vec![
        paragraph(vec![text("First")]),
        paragraph(vec![text("Second")]),
    ]);
    let lines = adf_to_rich(&adf);
    assert_eq!(
        lines.len(),
        2,
        "each paragraph must be its own line: {lines:?}"
    );
    assert_eq!(lines[0][0].text, "First");
    assert_eq!(lines[1][0].text, "Second");
}

#[test]
fn adf_to_rich_hard_break_splits_into_two_lines() {
    let adf = doc(vec![paragraph(vec![
        text("Line 1"),
        hard_break(),
        text("Line 2"),
    ])]);
    let lines = adf_to_rich(&adf);
    assert_eq!(
        lines.len(),
        2,
        "hardBreak must split into two lines: {lines:?}"
    );
    assert_eq!(lines[0][0].text, "Line 1");
    assert_eq!(lines[1][0].text, "Line 2");
}

#[test]
fn adf_to_rich_bullet_list_produces_dash_prefixed_lines() {
    let adf = doc(vec![bullet_list(vec![
        list_item(vec![paragraph(vec![text("Item A")])]),
        list_item(vec![paragraph(vec![text("Item B")])]),
    ])]);
    let lines = adf_to_rich(&adf);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0][0].text, "- ");
    assert_eq!(lines[0][1].text, "Item A");
    assert_eq!(lines[1][0].text, "- ");
    assert_eq!(lines[1][1].text, "Item B");
}

#[test]
fn adf_to_rich_ordered_list_produces_numbered_lines() {
    let adf = doc(vec![ordered_list(vec![
        list_item(vec![paragraph(vec![text("Item A")])]),
        list_item(vec![paragraph(vec![text("Item B")])]),
    ])]);
    let lines = adf_to_rich(&adf);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0][0].text, "1. ");
    assert_eq!(lines[1][0].text, "2. ");
}

#[test]
fn adf_to_rich_code_block_marks_code_style() {
    let adf = doc(vec![code_block(vec![text("let x = 1;")])]);
    let lines = adf_to_rich(&adf);
    let span = only_span(&lines);
    assert_eq!(span.text, "let x = 1;");
    assert!(
        span.style.code,
        "codeBlock content must carry code style: {span:?}"
    );
}

#[test]
fn adf_to_rich_heading_extracts_text_as_own_line() {
    let adf = doc(vec![heading(2, vec![text("Section Title")])]);
    let lines = adf_to_rich(&adf);
    let span = only_span(&lines);
    assert_eq!(span.text, "Section Title");
}

#[test]
fn adf_to_rich_blockquote_flattens_child_paragraph_into_own_line() {
    let adf = doc(vec![blockquote(vec![paragraph(vec![text("Quoted text")])])]);
    let lines = adf_to_rich(&adf);
    let span = only_span(&lines);
    assert_eq!(
        span.text, "Quoted text",
        "blockquote must flatten its paragraph child into its own line: {span:?}"
    );
}

#[test]
fn adf_to_rich_panel_flattens_child_paragraph_into_own_line() {
    let adf = doc(vec![panel(vec![paragraph(vec![text("Panel text")])])]);
    let lines = adf_to_rich(&adf);
    let span = only_span(&lines);
    assert_eq!(
        span.text, "Panel text",
        "panel must flatten its paragraph child into its own line: {span:?}"
    );
}

#[test]
fn adf_to_rich_rule_produces_dash_line() {
    let adf = doc(vec![rule_block()]);
    let lines = adf_to_rich(&adf);
    let span = only_span(&lines);
    assert_eq!(
        span.text, "---",
        "rule must render as its own dash line: {span:?}"
    );
}

#[test]
fn adf_to_rich_non_adf_string_returns_single_unstyled_line() {
    let plain = "just plain text";
    let lines = adf_to_rich(plain);
    let span = only_span(&lines);
    assert_eq!(span.text, plain);
    assert_eq!(span.style, RichStyle::default());
}

#[test]
fn adf_to_rich_empty_doc_returns_empty_vec() {
    let adf = doc(vec![]);
    let lines = adf_to_rich(&adf);
    assert!(lines.is_empty(), "empty doc must yield no lines: {lines:?}");
}

// --- adf_to_rich: table (issue 0034 / ADR 0014 §6 / BDR 0007 S9) ---

#[test]
fn adf_to_rich_table_header_and_two_rows_render_one_line_per_row_with_separators() {
    let adf = doc(vec![table(vec![
        table_row(vec![
            table_header(vec![paragraph(vec![text("Name")])]),
            table_header(vec![paragraph(vec![text("Status")])]),
        ]),
        table_row(vec![
            table_cell(vec![paragraph(vec![text("Alice")])]),
            table_cell(vec![paragraph(vec![text("Open")])]),
        ]),
        table_row(vec![
            table_cell(vec![paragraph(vec![text("Bob")])]),
            table_cell(vec![paragraph(vec![text("Done")])]),
        ]),
    ])]);
    let lines = adf_to_rich(&adf);
    assert_eq!(
        lines.len(),
        3,
        "table must render one line per row (header + 2 data rows): {lines:?}"
    );

    assert_eq!(lines[0][0].text, "Name");
    assert!(
        lines[0][0].style.bold,
        "header cell run must be bold: {:?}",
        lines[0][0]
    );
    assert_eq!(lines[0][1].text, " │ ");
    assert_eq!(lines[0][2].text, "Status");
    assert!(
        lines[0][2].style.bold,
        "header cell run must be bold: {:?}",
        lines[0][2]
    );

    assert_eq!(lines[1][0].text, "Alice");
    assert!(
        !lines[1][0].style.bold,
        "data-row cell run must not be bold: {:?}",
        lines[1][0]
    );
    assert_eq!(lines[1][1].text, " │ ");
    assert_eq!(lines[1][2].text, "Open");

    assert_eq!(lines[2][0].text, "Bob");
    assert_eq!(lines[2][1].text, " │ ");
    assert_eq!(lines[2][2].text, "Done");
}

#[test]
fn adf_to_rich_table_empty_cell_yields_empty_segment_between_separators() {
    let adf = doc(vec![table(vec![table_row(vec![
        table_cell(vec![paragraph(vec![text("Left")])]),
        table_cell(vec![]),
        table_cell(vec![paragraph(vec![text("Right")])]),
    ])])]);
    let lines = adf_to_rich(&adf);
    assert_eq!(lines.len(), 1);
    let text: String = lines[0].iter().map(|s| s.text.as_str()).collect();
    assert_eq!(
        text, "Left │  │ Right",
        "an empty cell must still leave separators on either side of it: {text}"
    );
}

#[test]
fn adf_to_rich_table_cell_with_marks_preserves_them() {
    let adf = doc(vec![table(vec![table_row(vec![table_cell(vec![
        paragraph(vec![marked_text(
            "click",
            vec![mark("strong"), link_mark("https://example.com")],
        )]),
    ])])])]);
    let lines = adf_to_rich(&adf);
    assert_eq!(lines.len(), 1);
    let span = &lines[0][0];
    assert_eq!(span.text, "click");
    assert!(
        span.style.bold,
        "existing strong mark must survive: {span:?}"
    );
    assert_eq!(
        span.style.link.as_deref(),
        Some("https://example.com"),
        "existing link mark must survive: {span:?}"
    );
}

#[test]
fn adf_to_rich_table_cell_with_nested_list_flattens_to_text() {
    let adf = doc(vec![table(vec![table_row(vec![table_cell(vec![
        bullet_list(vec![
            list_item(vec![paragraph(vec![text("Item A")])]),
            list_item(vec![paragraph(vec![text("Item B")])]),
        ]),
    ])])])]);
    let lines = adf_to_rich(&adf);
    assert_eq!(lines.len(), 1);
    let text: String = lines[0].iter().map(|s| s.text.as_str()).collect();
    assert_eq!(
        text.trim_end(),
        "Item A Item B",
        "nested list content must flatten to plain inline text, no bullet markers: {text}"
    );
}

#[test]
fn adf_to_rich_table_between_paragraphs_keeps_surrounding_content_intact() {
    let adf = doc(vec![
        paragraph(vec![text("Before")]),
        table(vec![table_row(vec![table_cell(vec![paragraph(vec![
            text("Cell"),
        ])])])]),
        paragraph(vec![text("After")]),
    ]);
    let lines = adf_to_rich(&adf);
    assert_eq!(
        lines.len(),
        3,
        "surrounding paragraphs must be preserved around the table: {lines:?}"
    );
    assert_eq!(lines[0][0].text, "Before");
    assert_eq!(lines[1][0].text, "Cell");
    assert_eq!(lines[2][0].text, "After");
}

#[test]
fn adf_to_rich_table_with_no_rows_renders_nothing() {
    let adf = doc(vec![table(vec![])]);
    let lines = adf_to_rich(&adf);
    assert!(
        lines.is_empty(),
        "a table with no rows must render no lines, no panic: {lines:?}"
    );
}

#[test]
fn adf_to_rich_table_row_with_no_cells_renders_an_empty_line_without_panic() {
    let adf = doc(vec![table(vec![table_row(vec![])])]);
    let lines = adf_to_rich(&adf);
    assert_eq!(lines.len(), 1);
    assert!(
        lines[0].is_empty(),
        "a row with no cells must render as an empty line, no panic: {:?}",
        lines[0]
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

// --- days_from_civil / relative_due (issue 0025 / ADR 0013) ---

#[test]
fn days_from_civil_unix_epoch_is_zero() {
    assert_eq!(days_from_civil(1970, 1, 1), 0);
}

#[test]
fn days_from_civil_known_reference_date() {
    assert_eq!(days_from_civil(2000, 3, 1), 11_017);
}

#[test]
fn days_from_civil_day_before_epoch_is_negative_one() {
    assert_eq!(days_from_civil(1969, 12, 31), -1);
}

#[test]
fn relative_due_buckets_by_day_delta() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");
    let due_days = days_from_civil(2026, 7, 3);
    let cases: [(i64, &str); 7] = [
        (0, "today"),
        (1, "tomorrow"),
        (2, "in 2 days"),
        (5, "in 5 days"),
        (-1, "overdue by 1 day"),
        (-2, "overdue by 2 days"),
        (-10, "overdue by 10 days"),
    ];
    for (delta, expected) in cases {
        let today = due_days - delta;
        assert_eq!(
            relative_due("2026-07-03", today).as_deref(),
            Some(expected),
            "delta {delta} must bucket to {expected:?}"
        );
    }
    set_language("en");
}

#[test]
fn relative_due_two_days_boundary_is_plural_not_singular() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");
    let due_days = days_from_civil(2026, 7, 3);
    let result = relative_due("2026-07-03", due_days - 2);
    assert_eq!(result.as_deref(), Some("in 2 days"));
    assert_ne!(
        result.as_deref(),
        Some("in 1 day"),
        "delta 2 must never fall into the singular tomorrow-style bucket"
    );
    set_language("en");
}

#[test]
fn relative_due_overdue_boundary_singular_only_at_exactly_one_day() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");
    let due_days = days_from_civil(2026, 7, 3);
    assert_eq!(
        relative_due("2026-07-03", due_days + 1).as_deref(),
        Some("overdue by 1 day")
    );
    assert_eq!(
        relative_due("2026-07-03", due_days + 2).as_deref(),
        Some("overdue by 2 days"),
        "delta -2 must use the plural overdue template, not singular"
    );
    set_language("en");
}

#[test]
fn relative_due_pt_br_translates_every_bucket() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("pt_BR");
    let due_days = days_from_civil(2026, 7, 3);
    assert_eq!(
        relative_due("2026-07-03", due_days).as_deref(),
        Some("hoje")
    );
    assert_eq!(
        relative_due("2026-07-03", due_days - 1).as_deref(),
        Some("amanhã")
    );
    assert_eq!(
        relative_due("2026-07-03", due_days - 3).as_deref(),
        Some("em 3 dias")
    );
    assert_eq!(
        relative_due("2026-07-03", due_days + 1).as_deref(),
        Some("atrasada há 1 dia")
    );
    assert_eq!(
        relative_due("2026-07-03", due_days + 4).as_deref(),
        Some("atrasada há 4 dias")
    );
    set_language("en");
}

#[test]
fn relative_due_unparseable_input_returns_none() {
    let today = days_from_civil(2026, 7, 3);
    assert_eq!(relative_due("not-a-date", today), None);
    assert_eq!(relative_due("2026/07/03", today), None);
    assert_eq!(relative_due("2026-07", today), None);
    assert_eq!(relative_due("2026-07-03-extra", today), None);
    assert_eq!(relative_due("", today), None);
}

// --- render_issue_human: Due line (issue 0025 / ADR 0013) ---

#[test]
fn render_issue_human_emits_due_line_after_updated_when_duedate_parses() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");
    let issue = Issue {
        duedate: Some(duedate_offset_from_today(3)),
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
    assert!(
        text.contains("Due: in 3 days"),
        "must show Due: in 3 days after Updated: {text}"
    );
    let updated_pos = text.find("Updated:").expect("Updated line must be present");
    let due_pos = text.find("Due:").expect("Due line must be present");
    assert!(
        due_pos > updated_pos,
        "Due line must come after Updated: {text}"
    );
    set_language("en");
}

#[test]
fn render_issue_human_pt_br_due_line_translated() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("pt_BR");
    let issue = Issue {
        duedate: Some(duedate_offset_from_today(3)),
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
    assert!(
        text.contains("Prazo: em 3 dias"),
        "must show Prazo: em 3 dias: {text}"
    );
    set_language("en");
}

#[test]
fn render_issue_human_omits_due_line_when_duedate_is_none() {
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
    assert!(
        !text.contains("Due:"),
        "no duedate must omit the Due line: {text}"
    );
    set_language("en");
}

#[test]
fn render_issue_human_omits_due_line_when_duedate_unparseable() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");
    let issue = Issue {
        duedate: Some("not-a-date".to_string()),
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
    assert!(
        !text.contains("Due:"),
        "unparseable duedate must omit the Due line: {text}"
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
        duedate: None,
        project: None,
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
