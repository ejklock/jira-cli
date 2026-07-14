use super::*;

fn s(v: &str) -> String {
    v.to_owned()
}

fn argv(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| s.to_string()).collect()
}

fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
    let mut all = vec!["jira"];
    all.extend_from_slice(args);
    Cli::try_parse_from(all)
}

#[test]
fn bare_ref_prepends_get() {
    let result = normalize_argv(&argv(&["PROJ-123"]), None);
    assert_eq!(result, argv(&["get", "PROJ-123"]));
}

#[test]
fn bare_ref_with_flags_prepends_get() {
    let result = normalize_argv(&argv(&["PROJ-123", "--json"]), None);
    assert_eq!(result, argv(&["get", "PROJ-123", "--json"]));
}

#[test]
fn known_command_passes_through_unchanged() {
    for cmd in KNOWN_COMMANDS {
        let input = argv(&[cmd]);
        let result = normalize_argv(&input, None);
        assert_eq!(result, input, "command '{cmd}' should pass through");
    }
}

#[test]
fn flag_first_argv_passes_through_unchanged() {
    let input = argv(&["--help"]);
    let result = normalize_argv(&input, None);
    assert_eq!(result, input);
}

#[test]
fn empty_argv_with_none_branch_passes_through() {
    let result = normalize_argv(&[], None);
    assert_eq!(result, argv(&[]));
}

#[test]
fn empty_argv_with_non_matching_branch_passes_through() {
    let result = normalize_argv(&[], Some("main"));
    assert_eq!(result, argv(&[]));
}

#[test]
fn branch_pattern_accepts_valid_branches() {
    assert!(branch_matches_issue_pattern("feature/PROJ-123"));
    assert!(branch_matches_issue_pattern("hotfix/AB-1"));
    assert!(branch_matches_issue_pattern("fix/ABC-999"));
}

#[test]
fn branch_pattern_accepts_any_prefix_with_key() {
    assert!(branch_matches_issue_pattern("chore/PROJ-123"));
    assert!(branch_matches_issue_pattern("story/EJ-PROJ-123-foo"));
}

#[test]
fn branch_pattern_rejects_branches_without_key() {
    assert!(!branch_matches_issue_pattern("main"));
    assert!(!branch_matches_issue_pattern("HEAD"));
    assert!(!branch_matches_issue_pattern("feature/login"));
}

#[test]
fn branch_pattern_rejects_missing_dash() {
    assert!(!branch_matches_issue_pattern("feature/PROJ123"));
}

// ---- extract_issue_key unit tests (BDR 0004) ----

#[test]
fn extract_issue_key_from_prefixed_branch() {
    assert_eq!(
        extract_issue_key("feature/PROJ-123-add-login"),
        Some(s("PROJ-123"))
    );
}

#[test]
fn extract_issue_key_from_bare_branch() {
    assert_eq!(extract_issue_key("PROJ-123"), Some(s("PROJ-123")));
}

#[test]
fn extract_issue_key_from_short_project_key() {
    assert_eq!(extract_issue_key("bugfix/ABC-9"), Some(s("ABC-9")));
}

#[test]
fn extract_issue_key_returns_none_for_main() {
    assert_eq!(extract_issue_key("main"), None);
}

#[test]
fn extract_issue_key_returns_none_for_feature_without_key() {
    assert_eq!(extract_issue_key("feature/login"), None);
}

#[test]
fn extract_issue_key_returns_first_match_when_multiple_keys_present() {
    assert_eq!(extract_issue_key("PROJ-1-then-DEF-2"), Some(s("PROJ-1")));
}

#[test]
fn extract_issue_key_returns_none_for_empty_string() {
    assert_eq!(extract_issue_key(""), None);
}

#[test]
fn extract_issue_key_returns_none_for_lowercase_key() {
    assert_eq!(extract_issue_key("feature/proj-123"), None);
}

#[test]
fn parse_setup_add_with_all_flags() {
    let cli = parse(&[
        "setup",
        "add",
        "--name",
        "work",
        "--url",
        "https://org.atlassian.net",
        "--email",
        "a@b.com",
    ])
    .unwrap();
    let Command::Setup(opts) = cli.command.unwrap() else {
        panic!("expected Setup")
    };
    let SetupCmd::Add(add) = opts.subcommand else {
        panic!("expected Add")
    };
    assert_eq!(add.name.as_deref(), Some("work"));
    assert_eq!(add.url.as_deref(), Some("https://org.atlassian.net"));
    assert_eq!(add.email.as_deref(), Some("a@b.com"));
}

#[test]
fn parse_setup_add_without_flags_is_ok() {
    let cli = parse(&["setup", "add"]).unwrap();
    let Command::Setup(opts) = cli.command.unwrap() else {
        panic!()
    };
    let SetupCmd::Add(add) = opts.subcommand else {
        panic!()
    };
    assert!(add.name.is_none());
    assert!(add.url.is_none());
    assert!(add.email.is_none());
}

#[test]
fn parse_setup_list() {
    let cli = parse(&["setup", "list"]).unwrap();
    let Command::Setup(opts) = cli.command.unwrap() else {
        panic!()
    };
    assert!(matches!(opts.subcommand, SetupCmd::List));
}

#[test]
fn parse_setup_remove_requires_name() {
    let err = parse(&["setup", "remove"]);
    assert!(err.is_err());
    let cli = parse(&["setup", "remove", "--name", "work"]).unwrap();
    let Command::Setup(opts) = cli.command.unwrap() else {
        panic!()
    };
    let SetupCmd::Remove(r) = opts.subcommand else {
        panic!()
    };
    assert_eq!(r.name, "work");
}

#[test]
fn parse_setup_test_optional_name() {
    let cli = parse(&["setup", "test"]).unwrap();
    let Command::Setup(opts) = cli.command.unwrap() else {
        panic!()
    };
    let SetupCmd::Test(t) = opts.subcommand else {
        panic!()
    };
    assert!(t.name.is_none());

    let cli2 = parse(&["setup", "test", "--name", "work"]).unwrap();
    let Command::Setup(opts2) = cli2.command.unwrap() else {
        panic!()
    };
    let SetupCmd::Test(t2) = opts2.subcommand else {
        panic!()
    };
    assert_eq!(t2.name.as_deref(), Some("work"));
}

#[test]
fn parse_get_with_ref_and_display_flags() {
    let cli = parse(&[
        "get",
        "PROJ-123",
        "--instance",
        "work",
        "--json",
        "--refresh",
    ])
    .unwrap();
    let Command::Get(g) = cli.command.unwrap() else {
        panic!()
    };
    assert_eq!(g.ref_, "PROJ-123");
    assert_eq!(g.display.instance.as_deref(), Some("work"));
    assert!(g.display.json);
    assert!(g.display.refresh);
}

#[test]
fn parse_current_with_instance_flag() {
    let cli = parse(&["current", "--instance", "work"]).unwrap();
    let Command::Current(d) = cli.command.unwrap() else {
        panic!()
    };
    assert_eq!(d.instance.as_deref(), Some("work"));
}

#[test]
fn parse_mine_no_flags() {
    let cli = parse(&["mine"]).unwrap();
    assert!(matches!(cli.command, Some(Command::Mine(_))));
}

#[test]
fn parse_list_alias_for_mine() {
    let cli = parse(&["list"]).unwrap();
    assert!(matches!(cli.command, Some(Command::Mine(_))));
}

#[test]
fn parse_search_no_flags() {
    let cli = parse(&["search"]).unwrap();
    assert!(matches!(cli.command, Some(Command::Search(_))));
}

#[test]
fn parse_unknown_subcommand_returns_error() {
    let err = parse(&["unknown-cmd"]);
    assert!(err.is_err());
}

#[test]
fn parse_no_subcommand_yields_none_command() {
    let cli = parse(&[]).unwrap();
    assert!(cli.command.is_none());
}

#[test]
fn parse_missing_required_ref_for_get_returns_error() {
    let err = parse(&["get"]);
    assert!(err.is_err());
}

#[test]
fn bare_no_command_action_tty_yields_run_mine() {
    assert_eq!(bare_no_command_action(true), BareNoCommandAction::RunMine);
}

#[test]
fn bare_no_command_action_non_tty_yields_help_exit2() {
    assert_eq!(
        bare_no_command_action(false),
        BareNoCommandAction::HelpExit2
    );
}

// ---- command_surface truth table (ADR 0025, BDR 0016 S1/S3/S4) ----

#[test]
fn command_surface_tty_without_json_is_interactive() {
    assert_eq!(command_surface(true, false), Surface::Interactive);
}

#[test]
fn command_surface_tty_with_json_is_agent() {
    assert_eq!(command_surface(true, true), Surface::Agent);
}

#[test]
fn command_surface_non_tty_without_json_is_agent() {
    assert_eq!(command_surface(false, false), Surface::Agent);
}

#[test]
fn command_surface_non_tty_with_json_is_agent() {
    assert_eq!(command_surface(false, true), Surface::Agent);
}
