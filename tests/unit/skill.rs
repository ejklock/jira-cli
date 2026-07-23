use super::*;

fn buffers() -> (Vec<u8>, Vec<u8>) {
    (Vec::new(), Vec::new())
}

// --- AC1: named skill prints the full embedded body byte-for-byte ---

#[test]
fn named_skill_prints_full_body_byte_for_byte() {
    let (mut out, mut err) = buffers();
    let code = skill_output(Some("jira"), &mut out, &mut err);
    assert_eq!(code, 0);
    assert_eq!(
        out,
        include_str!("../../.claude/skills/jira/SKILL.md").as_bytes()
    );
    assert!(err.is_empty());
}

// --- AC2: list prints name<TAB>description<newline> ---

#[test]
fn list_prints_name_tab_first_sentence_description() {
    let (mut out, mut err) = buffers();
    let code = skill_output(Some("list"), &mut out, &mut err);
    assert_eq!(code, 0);
    let expected = format!("jira\t{}\n", REGISTRY[0].description);
    assert_eq!(out, expected.as_bytes());
    assert!(err.is_empty());
}

// --- AC3: bare with a single registered skill prints the same body as AC1 ---

#[test]
fn bare_with_single_registered_skill_prints_same_body_as_named() {
    let (mut named_out, mut named_err) = buffers();
    skill_output(Some("jira"), &mut named_out, &mut named_err);

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
    skill_output(Some("jira"), &mut out, &mut err);
    let printed = String::from_utf8(out).unwrap();
    let source = include_str!("../../.claude/skills/jira/SKILL.md");
    assert_eq!(printed.len(), source.len());
}
