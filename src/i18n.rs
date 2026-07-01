use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

use regex::Regex;

pub const SUPPORTED: [&str; 2] = ["en", "pt_BR"];

static LANGUAGE: RwLock<String> = RwLock::new(String::new());

// Serializes every language-dependent test across modules — LANGUAGE is a process global.
#[cfg(test)]
pub(crate) static LANG_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

static CATALOG: OnceLock<HashMap<String, String>> = OnceLock::new();

fn pt_br_catalog() -> &'static HashMap<String, String> {
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("../locales/pt_BR.json"))
            .expect("pt_BR.json is embedded and must be valid JSON")
    })
}

/// Strip the encoding suffix (e.g. ".UTF-8"), normalize hyphens to underscores,
/// and map to a supported locale code.
///
/// Returns `Some("pt_BR")` for `pt_BR` or `pt-BR` variants, `Some("en")` for `en`
/// or `en_*` family variants. Everything else returns `None`.
///
/// Issue 0006 AC1: an explicit `en` must win over a stored `pt_BR` (env > DB
/// precedence), so `en` must be recognized here rather than falling through.
pub fn normalize_locale(raw: &str) -> Option<&'static str> {
    let locale = raw.split('.').next().unwrap_or("").replace('-', "_");
    if locale == "pt_BR" {
        return Some("pt_BR");
    }
    if locale == "en" || locale.starts_with("en_") {
        return Some("en");
    }
    None
}

/// Return the active language applying precedence: `env_value` > `db_value` > `"en"`.
///
/// Unknown or empty values fall through to the next source in the chain.
/// Issue 0006 AC1: an explicit `en` env value wins over a stored `pt_BR` DB value.
pub fn resolve_language(env_value: Option<&str>, db_value: Option<&str>) -> String {
    for candidate in [env_value, db_value].into_iter().flatten() {
        if candidate.is_empty() {
            continue;
        }
        if let Some(code) = normalize_locale(candidate) {
            if SUPPORTED.contains(&code) {
                return code.to_owned();
            }
        }
    }
    "en".to_owned()
}

/// Set the process-global display language.
/// Panics only if the lock is poisoned (unrecoverable).
pub fn set_language(lang: &str) {
    let mut guard = LANGUAGE.write().expect("language lock poisoned");
    *guard = lang.to_owned();
}

/// Return the current display language code.
pub fn current_language() -> String {
    let guard = LANGUAGE.read().expect("language lock poisoned");
    if guard.is_empty() {
        "en".to_owned()
    } else {
        guard.clone()
    }
}

/// Translate `s` using the active-language catalog. Mirrors Python `__(s)`.
///
/// Under `"pt_BR"`, returns the catalog translation for known keys and `s` unchanged
/// for unknown keys. Under any other language, returns `s` unchanged (identity).
pub fn t(s: &str) -> String {
    if current_language() == "pt_BR" {
        pt_br_catalog()
            .get(s)
            .cloned()
            .unwrap_or_else(|| s.to_owned())
    } else {
        s.to_owned()
    }
}

static PLACEHOLDER_RE: OnceLock<Regex> = OnceLock::new();

fn placeholder_re() -> &'static Regex {
    PLACEHOLDER_RE.get_or_init(|| Regex::new(r"\{(\w+)\}").expect("placeholder regex is valid"))
}

/// Translate `template` then substitute `{name}` tokens in the result.
///
/// Lookup order: `t(template)` performs the catalog lookup first (so the key
/// is always the English placeholder template, never a runtime value), then
/// each `{name}` token in the translated string is replaced with the
/// corresponding value from `args`.  Tokens with no matching arg are left
/// intact.  A single pass over the translated string is used, so a runtime
/// value that itself contains a `{token}` string is never re-interpreted.
pub fn tf(template: &str, args: &[(&str, &str)]) -> String {
    let translated = t(template);
    let re = placeholder_re();
    re.replace_all(&translated, |caps: &regex::Captures| {
        let token = &caps[1];
        args.iter()
            .find(|(name, _)| *name == token)
            .map(|(_, value)| *value)
            .unwrap_or(&caps[0])
            .to_owned()
    })
    .into_owned()
}

#[cfg(test)]
#[path = "../tests/unit/i18n.rs"]
mod tests;
