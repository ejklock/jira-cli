use std::path::{Path, PathBuf};

/// The gouqi async client's write-verb entry points (ADR 0015 §1 / ADR 0022 §5).
/// A bare `.post(`/`.put(`/`.delete(` token also matches unrelated methods of
/// the same name on other receivers (e.g. a local repository's `.delete(`),
/// so a real hit additionally requires a `.jira` receiver nearby — see
/// `has_jira_receiver_nearby`.
const WRITE_VERBS: &[&str] = &[
    "post_versioned",
    "put_versioned",
    "delete_versioned",
    ".post(",
    ".put(",
    ".delete(",
];

const RECEIVER_WINDOW: usize = 6;
const ENDPOINT_WINDOW: usize = 5;

/// Advance `*i` past a string/char literal opening at `*i` (whose quote byte
/// is `quote`), honoring backslash escapes so an escaped quote does not end
/// the literal early.
fn skip_string_literal(bytes: &[u8], i: &mut usize, quote: u8) {
    let len = bytes.len();
    *i += 1;
    while *i < len {
        if bytes[*i] == b'\\' {
            *i += 2;
        } else if bytes[*i] == quote {
            *i += 1;
            break;
        } else {
            *i += 1;
        }
    }
}

/// Find the byte index of a real `//` line comment start, skipping over `//`
/// occurrences inside string/char literals. Mirrors `tests/comment_policy.rs`'s
/// `find_line_comment` so both source-scan gates share the same pragmatic
/// string-literal awareness.
fn find_line_comment(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        match bytes[i] {
            b'"' | b'\'' => {
                let quote = bytes[i];
                skip_string_literal(bytes, &mut i, quote);
            }
            b'/' if i + 1 < len && bytes[i + 1] == b'/' => return Some(i),
            _ => i += 1,
        }
    }
    None
}

/// The index of `verb` in `line` when it occurs in real code (not inside a
/// `//` comment).
fn real_code_index(line: &str, verb: &str) -> Option<usize> {
    let idx = line.find(verb)?;
    match find_line_comment(line) {
        Some(comment_start) if idx >= comment_start => None,
        _ => Some(idx),
    }
}

/// A short single-word verb (`.post(`/`.put(`/`.delete(`) is only a genuine
/// gouqi write call when a `.jira` receiver appears within a few lines
/// above it — the method chains this crate writes always start `self.jira`,
/// possibly split across lines by rustfmt.
fn has_jira_receiver_nearby(lines: &[&str], line_idx: usize) -> bool {
    let start = line_idx.saturating_sub(RECEIVER_WINDOW);
    lines[start..=line_idx].iter().any(|l| l.contains(".jira"))
}

fn is_qualified_gouqi_verb(verb: &str) -> bool {
    matches!(
        verb,
        "post_versioned" | "put_versioned" | "delete_versioned"
    )
}

#[derive(Debug)]
struct WriteVerbSite {
    file: PathBuf,
    line_num: usize,
    verb: &'static str,
}

fn find_write_verb_sites_in_file(file: &Path, lines: &[&str]) -> Vec<WriteVerbSite> {
    let mut sites = Vec::new();
    for (idx, line) in lines.iter().enumerate() {
        for &verb in WRITE_VERBS {
            let Some(_) = real_code_index(line, verb) else {
                continue;
            };
            if is_qualified_gouqi_verb(verb) || has_jira_receiver_nearby(lines, idx) {
                sites.push(WriteVerbSite {
                    file: file.to_path_buf(),
                    line_num: idx + 1,
                    verb,
                });
            }
        }
    }
    sites
}

/// The call site's window (this line plus a few lines either side) must
/// mention either a `/comment` endpoint or the `/transitions` endpoint —
/// the literal check for Constitution Amendment 2's "never a write surface
/// beyond comment + transition" clause.
fn window_targets_allowed_write_endpoint(lines: &[&str], line_idx: usize) -> bool {
    let start = line_idx.saturating_sub(ENDPOINT_WINDOW);
    let end = (line_idx + ENDPOINT_WINDOW + 1).min(lines.len());
    lines[start..end]
        .iter()
        .any(|l| l.contains("/comment") || l.contains("/transitions"))
}

fn walk_rs_files(dir: &Path) -> Vec<PathBuf> {
    let mut result = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                result.extend(walk_rs_files(&path));
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                result.push(path);
            }
        }
    }
    result
}

fn describe_violation(site: &WriteVerbSite, reason: &str) -> String {
    format!(
        "{}:{}  write verb `{}` {reason}",
        site.file.display(),
        site.line_num,
        site.verb
    )
}

fn scan_violations(files: &[PathBuf], client_rs: &Path) -> Vec<String> {
    let mut violations = Vec::new();
    for file in files {
        let text = std::fs::read_to_string(file)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", file.display()));
        let lines: Vec<&str> = text.lines().collect();
        for site in find_write_verb_sites_in_file(file, &lines) {
            if site.file != client_rs {
                violations.push(describe_violation(&site, "found outside src/client.rs"));
                continue;
            }
            if !window_targets_allowed_write_endpoint(&lines, site.line_num - 1) {
                violations.push(describe_violation(
                    &site,
                    "call site does not target a /comment or /transitions endpoint",
                ));
            }
        }
    }
    violations
}

#[test]
fn find_line_comment_ignores_write_verbs_inside_prose() {
    let line = "/// gouqi has no `put_versioned`/`delete_versioned` helpers.";
    assert_eq!(real_code_index(line, "put_versioned"), None);
    assert_eq!(real_code_index(line, "delete_versioned"), None);
}

#[test]
fn has_jira_receiver_nearby_true_when_jira_appears_within_window() {
    let lines = vec!["let raw = self", ".jira", ".put(\"api\", &endpoint, body)"];
    assert!(has_jira_receiver_nearby(&lines, 2));
}

#[test]
fn has_jira_receiver_nearby_false_for_unrelated_receiver() {
    let lines = vec!["let deleted = match repo.delete(name) {"];
    assert!(!has_jira_receiver_nearby(&lines, 0));
}

#[test]
fn window_targets_allowed_write_endpoint_true_when_nearby_line_has_comment_path() {
    let lines = vec![
        "let endpoint = v3_write_endpoint(&format!(\"/issue/{key}/comment/{id}\"));",
        ".put(\"api\", &endpoint, body)",
    ];
    assert!(window_targets_allowed_write_endpoint(&lines, 1));
}

#[test]
fn window_targets_allowed_write_endpoint_true_when_nearby_line_has_transitions_path() {
    let lines = vec![
        "let endpoint = format!(\"/issue/{key}/transitions\");",
        ".post_versioned(\"api\", Some(\"3\"), &endpoint, body)",
    ];
    assert!(window_targets_allowed_write_endpoint(&lines, 1));
}

#[test]
fn scan_violations_flags_unconfined_write_verb_outside_client_rs() {
    let tmp = std::env::temp_dir().join("write_surface_violation_test");
    std::fs::create_dir_all(&tmp).unwrap();
    let other = tmp.join("commands.rs");
    std::fs::write(
        &other,
        "fn f() {\n    self.jira.delete(\"api\", \"/issue/1\");\n}\n",
    )
    .unwrap();

    let files = vec![other.clone()];
    let violations = scan_violations(&files, &tmp.join("client.rs"));

    assert_eq!(violations.len(), 1);
    assert!(violations[0].contains("outside src/client.rs"));
    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn scan_violations_flags_client_rs_write_verb_not_targeting_allowed_endpoint() {
    let tmp = std::env::temp_dir().join("write_surface_non_comment_test");
    std::fs::create_dir_all(&tmp).unwrap();
    let client_rs = tmp.join("client.rs");
    std::fs::write(
        &client_rs,
        "fn f() {\n    self.jira.delete(\"api\", \"/issue/1/worklog/1\");\n}\n",
    )
    .unwrap();

    let files = vec![client_rs.clone()];
    let violations = scan_violations(&files, &client_rs);

    assert_eq!(violations.len(), 1);
    assert!(violations[0].contains("does not target a /comment or /transitions endpoint"));
    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn scan_violations_is_clean_for_a_confined_comment_write_call() {
    let tmp = std::env::temp_dir().join("write_surface_clean_test");
    std::fs::create_dir_all(&tmp).unwrap();
    let client_rs = tmp.join("client.rs");
    std::fs::write(
        &client_rs,
        "fn f() {\n    self.jira.delete(\"api\", \"/issue/1/comment/1\");\n}\n",
    )
    .unwrap();

    let files = vec![client_rs.clone()];
    let violations = scan_violations(&files, &client_rs);

    assert!(
        violations.is_empty(),
        "unexpected violations: {violations:?}"
    );
    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn scan_violations_is_clean_for_a_confined_transition_write_call() {
    let tmp = std::env::temp_dir().join("write_surface_transition_clean_test");
    std::fs::create_dir_all(&tmp).unwrap();
    let client_rs = tmp.join("client.rs");
    std::fs::write(
        &client_rs,
        "fn f() {\n    self.jira.post_versioned(\"api\", Some(\"3\"), \"/issue/1/transitions\", body);\n}\n",
    )
    .unwrap();

    let files = vec![client_rs.clone()];
    let violations = scan_violations(&files, &client_rs);

    assert!(
        violations.is_empty(),
        "a transition POST window must be clean: {violations:?}"
    );
    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn every_gouqi_write_verb_call_site_is_confined_to_client_rs_comment_endpoints() {
    let src_dir = Path::new("src");
    let files = walk_rs_files(src_dir);
    assert!(
        !files.is_empty(),
        "walk_rs_files found no .rs files under src/"
    );

    let client_rs = Path::new("src").join("client.rs");
    let violations = scan_violations(&files, &client_rs);

    if !violations.is_empty() {
        let report = violations.join("\n");
        panic!("write surface violations found:\n{report}");
    }
}
