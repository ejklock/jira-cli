#![allow(dead_code)]

use std::path::PathBuf;

pub struct Config {
    pub db_path: PathBuf,
    pub task_cache_ttl_hours: u32,
}

pub fn load() -> Config {
    let db_path = resolve_db_path();
    Config {
        db_path,
        task_cache_ttl_hours: 24,
    }
}

fn resolve_db_path() -> PathBuf {
    if let Ok(val) = std::env::var("JIRA_DB") {
        return PathBuf::from(val);
    }
    jira_config_dir().join("jira.db")
}

/// The per-user config root, `~/.config/jira/` — shared by the db path
/// (`resolve_db_path`) and the attachment download dir (ADR 0029 §2).
pub fn jira_config_dir() -> PathBuf {
    dirs::home_dir()
        .expect("cannot determine home directory")
        .join(".config")
        .join("jira")
}

#[cfg(test)]
#[path = "../tests/unit/config.rs"]
mod tests;
