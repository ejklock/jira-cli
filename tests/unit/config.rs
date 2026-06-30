use super::*;
use std::env;

#[test]
fn load_uses_env_var_when_set() {
    env::set_var("JIRA_DB", "/tmp/test-override.db");
    let cfg = load();
    assert_eq!(cfg.db_path, PathBuf::from("/tmp/test-override.db"));
    env::remove_var("JIRA_DB");
}

#[test]
fn load_falls_back_to_home_config_dir() {
    env::remove_var("JIRA_DB");
    let cfg = load();
    let home = dirs::home_dir().unwrap();
    let expected = home.join(".config").join("jira").join("jira.db");
    assert_eq!(cfg.db_path, expected);
}

#[test]
fn task_cache_ttl_hours_is_24() {
    env::remove_var("JIRA_DB");
    let cfg = load();
    assert_eq!(cfg.task_cache_ttl_hours, 24);
}
