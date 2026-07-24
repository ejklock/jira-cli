use super::*;
use crate::client::GouqiJiraClient;
use crate::store::instances::Instance;
use crate::test_support::*;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn make_instance(base_url: &str) -> Instance {
    Instance {
        name: "test-instance".to_string(),
        base_url: base_url.to_string(),
        email: "user@example.com".to_string(),
        token: "test-api-token".to_string(),
        account_id: None,
    }
}

// --- download_dir_for (AC1, BDR 0020 S4) ---

#[test]
fn download_dir_for_joins_downloads_and_issue_key_under_root() {
    let dir = download_dir_for(Path::new("/home/user/.config/jira"), "ABC-1");
    assert_eq!(
        dir,
        PathBuf::from("/home/user/.config/jira/downloads/ABC-1")
    );
}

// --- dedupe_filename (AC1, BDR 0020 S6) ---

#[test]
fn dedupe_filename_returns_input_unchanged_when_not_taken() {
    let taken = vec!["other.txt".to_owned()];
    assert_eq!(dedupe_filename(&taken, "report.pdf"), "report.pdf");
}

#[test]
fn dedupe_filename_inserts_suffix_before_extension_when_taken() {
    let taken = vec!["report.pdf".to_owned()];
    assert_eq!(dedupe_filename(&taken, "report.pdf"), "report (2).pdf");
}

#[test]
fn dedupe_filename_increments_suffix_until_unused() {
    let taken = vec!["report.pdf".to_owned(), "report (2).pdf".to_owned()];
    assert_eq!(dedupe_filename(&taken, "report.pdf"), "report (3).pdf");
}

#[test]
fn dedupe_filename_handles_a_filename_with_no_extension() {
    let taken = vec!["README".to_owned()];
    assert_eq!(dedupe_filename(&taken, "README"), "README (2)");
}

#[test]
fn dedupe_filename_result_is_never_already_taken_property() {
    for run_len in 0..50u32 {
        let mut taken: Vec<String> = vec!["file.txt".to_owned()];
        for n in 2..=(run_len + 1) {
            taken.push(format!("file ({n}).txt"));
        }
        let result = dedupe_filename(&taken, "file.txt");
        assert!(
            !taken.contains(&result),
            "dedupe_filename returned an already-taken name: {result} (taken={taken:?})"
        );
    }
}

// --- download_all (AC2, BDR 0020 S4, S7) ---

#[tokio::test]
async fn download_all_writes_every_attachment_with_served_bytes() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/attachment/1/screenshot.png"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"png-bytes".to_vec()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/attachment/2/notes.txt"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"notes content".to_vec()))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let issue = Issue {
        attachments: vec![
            attachment(
                "screenshot.png",
                &format!("{}/attachment/1/screenshot.png", server.uri()),
                Some("image/png"),
                Some(9),
            ),
            attachment(
                "notes.txt",
                &format!("{}/attachment/2/notes.txt", server.uri()),
                None,
                None,
            ),
        ],
        ..issue("ABC-1")
    };
    let dir = tempfile::tempdir().unwrap();

    let saved = download_all(&client, &issue, dir.path()).await.unwrap();

    assert_eq!(saved.len(), 2);
    let png_path = dir.path().join("screenshot.png");
    let txt_path = dir.path().join("notes.txt");
    assert_eq!(std::fs::read(&png_path).unwrap(), b"png-bytes");
    assert_eq!(std::fs::read(&txt_path).unwrap(), b"notes content");
    assert_eq!(saved[0].filename, "screenshot.png");
    assert_eq!(saved[0].bytes, 9);
    assert_eq!(saved[0].path, png_path);
    assert_eq!(saved[1].filename, "notes.txt");
    assert_eq!(saved[1].bytes, 13);
    assert_eq!(saved[1].path, txt_path);
}

#[tokio::test]
async fn download_all_dedupes_duplicate_filenames_before_writing() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/attachment/1/report.pdf"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"first".to_vec()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/attachment/2/report.pdf"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"second".to_vec()))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let issue = Issue {
        attachments: vec![
            attachment(
                "report.pdf",
                &format!("{}/attachment/1/report.pdf", server.uri()),
                None,
                None,
            ),
            attachment(
                "report.pdf",
                &format!("{}/attachment/2/report.pdf", server.uri()),
                None,
                None,
            ),
        ],
        ..issue("ABC-2")
    };
    let dir = tempfile::tempdir().unwrap();

    let saved = download_all(&client, &issue, dir.path()).await.unwrap();

    assert_eq!(saved[0].filename, "report.pdf");
    assert_eq!(saved[1].filename, "report (2).pdf");
    assert_eq!(
        std::fs::read(dir.path().join("report.pdf")).unwrap(),
        b"first"
    );
    assert_eq!(
        std::fs::read(dir.path().join("report (2).pdf")).unwrap(),
        b"second"
    );
}

#[tokio::test]
async fn download_all_with_zero_attachments_returns_empty_and_writes_nothing() {
    let server = MockServer::start().await;
    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let issue = Issue {
        attachments: vec![],
        ..issue("ABC-3")
    };
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("would-be-created");

    let saved = download_all(&client, &issue, &target).await.unwrap();

    assert!(saved.is_empty());
    assert!(
        !target.exists(),
        "no directory should be created when there are no attachments"
    );
}

#[tokio::test]
async fn download_all_propagates_a_failed_download() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/attachment/missing.png"))
        .respond_with(ResponseTemplate::new(404))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let issue = Issue {
        attachments: vec![attachment(
            "missing.png",
            &format!("{}/attachment/missing.png", server.uri()),
            None,
            None,
        )],
        ..issue("ABC-4")
    };
    let dir = tempfile::tempdir().unwrap();

    let result = download_all(&client, &issue, dir.path()).await;

    assert!(result.is_err());
}

// --- output formatting (AC3, BDR 0020 S5) ---

#[test]
fn format_saved_human_prints_one_saved_line_per_file() {
    let saved = vec![
        SavedAttachment {
            filename: "a.txt".to_owned(),
            path: PathBuf::from("/tmp/a.txt"),
            bytes: 5,
        },
        SavedAttachment {
            filename: "b.txt".to_owned(),
            path: PathBuf::from("/tmp/b.txt"),
            bytes: 9,
        },
    ];

    let out = format_saved_human(&saved);

    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines, vec!["saved /tmp/a.txt (5)", "saved /tmp/b.txt (9)"]);
}

#[test]
fn format_saved_human_of_empty_list_is_empty_string() {
    assert_eq!(format_saved_human(&[]), "");
}

#[test]
fn saved_to_json_lists_each_saved_attachment_as_a_curated_object() {
    let saved = vec![
        SavedAttachment {
            filename: "a.txt".to_owned(),
            path: PathBuf::from("/tmp/a.txt"),
            bytes: 5,
        },
        SavedAttachment {
            filename: "b.txt".to_owned(),
            path: PathBuf::from("/tmp/b.txt"),
            bytes: 9,
        },
    ];

    let json = saved_to_json("ABC-1", &saved);

    assert!(!json.contains('\n'), "the json output must be minified");
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["issue_key"], "ABC-1");
    let items = parsed["saved"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["filename"], "a.txt");
    assert_eq!(items[0]["path"], "/tmp/a.txt");
    assert_eq!(items[0]["bytes"], 5);
    assert_eq!(items[1]["filename"], "b.txt");
    assert_eq!(items[1]["path"], "/tmp/b.txt");
    assert_eq!(items[1]["bytes"], 9);
}

#[test]
fn saved_to_json_of_empty_list_lists_no_items() {
    let json = saved_to_json("ABC-5", &[]);

    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["issue_key"], "ABC-5");
    assert_eq!(parsed["saved"].as_array().unwrap().len(), 0);
}

#[test]
fn no_attachments_message_names_the_issue_key() {
    assert_eq!(no_attachments_message("ABC-1"), "ABC-1 has no attachments.");
}
