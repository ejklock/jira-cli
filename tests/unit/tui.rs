use super::*;

use crate::cli::{browse_tty_action, BrowseAction};

// ---- AC1: pure TTY-routing helper ----

#[test]
fn browse_tty_action_tty_yields_run_tui() {
    assert_eq!(browse_tty_action(true), BrowseAction::RunTui);
}

#[test]
fn browse_tty_action_non_tty_yields_tty_error() {
    assert_eq!(browse_tty_action(false), BrowseAction::TtyError);
}

// ---- AC2: non-TTY browse guard ----

#[tokio::test]
async fn non_tty_browse_writes_tty_error_to_stderr() {
    let instance = make_test_instance();
    let mut stderr = Vec::<u8>::new();

    let code = browse(&instance, false, &mut stderr).await;

    let output = String::from_utf8(stderr).expect("utf8");
    assert!(
        output.contains("Error: 'browse' requires an interactive terminal (TTY)."),
        "expected TTY error in stderr, got: {output:?}"
    );
    assert_ne!(code, 0, "non-TTY browse must return a non-zero exit code");
}

#[tokio::test]
async fn non_tty_browse_returns_non_zero_without_any_network_call() {
    // No client is constructed on the guard path; if browse attempts to access the
    // network the test would hang or panic because no mock server is running.
    let instance = make_test_instance();
    let mut stderr = Vec::<u8>::new();

    let code = browse(&instance, false, &mut stderr).await;

    assert_ne!(code, 0);
}

fn make_test_instance() -> crate::store::instances::Instance {
    crate::store::instances::Instance {
        name: "test".to_owned(),
        base_url: "https://test.atlassian.net".to_owned(),
        email: "test@example.com".to_owned(),
        token: "token".to_owned(),
        account_id: None,
    }
}
