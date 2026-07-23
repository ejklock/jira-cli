/// One entry in the embedded agent-skill registry (ADR 0028 §3).
///
/// `description` is the first sentence of the skill's `SKILL.md` frontmatter
/// description, verbatim; `body` is the full skill markdown.
pub struct SkillEntry {
    pub name: &'static str,
    pub description: &'static str,
    pub body: &'static str,
}

const REGISTRY: &[SkillEntry] = &[SkillEntry {
    name: "jira",
    description: "Read Jira Cloud issue data — an issue, your assignments, or a JQL search — as machine-readable JSON from the `jira` CLI, non-interactively without the TUI.",
    body: include_str!("../.claude/skills/jira/SKILL.md"),
}];

/// Serve `jira skill [name]` (ADR 0028, BDR 0019): `list` prints every
/// registered skill as `name<TAB>description`; a known `name` prints that
/// skill's full body; an unknown `name` errors to `err` and returns `2`;
/// omitted `name` prints the single registered skill's body, or falls back
/// to the list when more than one is ever registered.
pub fn skill_output(
    name: Option<&str>,
    out: &mut impl std::io::Write,
    err: &mut impl std::io::Write,
) -> i32 {
    match name {
        Some("list") => write_list(out),
        Some(requested) => write_named(requested, out, err),
        None => write_bare(out),
    }
}

fn write_list(out: &mut impl std::io::Write) -> i32 {
    for entry in REGISTRY {
        writeln!(out, "{}\t{}", entry.name, entry.description).ok();
    }
    0
}

fn write_named(
    requested: &str,
    out: &mut impl std::io::Write,
    err: &mut impl std::io::Write,
) -> i32 {
    match REGISTRY.iter().find(|entry| entry.name == requested) {
        Some(entry) => {
            write!(out, "{}", entry.body).ok();
            0
        }
        None => {
            writeln!(err, "unknown skill: {requested}").ok();
            2
        }
    }
}

fn write_bare(out: &mut impl std::io::Write) -> i32 {
    match REGISTRY {
        [entry] => {
            write!(out, "{}", entry.body).ok();
            0
        }
        _ => write_list(out),
    }
}

#[cfg(test)]
#[path = "../tests/unit/skill.rs"]
mod tests;
