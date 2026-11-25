//! The embedded agent skills as the MCP skills extension serves them: one `SKILL.md` per
//! registered skill, as a resource and as a catalog entry. Everything comes from embedded text;
//! no request argument ever reaches the filesystem.

use rmcp::model::{Resource, Skill, SkillFrontmatter, SkillResource, SkillResources};

use crate::skill::{SkillEntry, REGISTRY};

const MARKDOWN: &str = "text/markdown";
const FRONTMATTER_FENCE: &str = "---";

struct SkillFile {
    uri: String,
    text: &'static str,
    skill: Skill,
    resource: Resource,
}

/// The embedded skills: their files and catalog entries, built once at startup.
pub(super) struct SkillCatalog {
    files: Vec<SkillFile>,
}

impl SkillCatalog {
    pub(super) fn embedded() -> Result<Self, String> {
        Self::from_entries(REGISTRY)
    }

    fn from_entries(entries: &[SkillEntry]) -> Result<Self, String> {
        let files = entries
            .iter()
            .map(|entry| {
                let uri = format!("skill://{}/SKILL.md", entry.name);
                let frontmatter = parse_frontmatter(entry.body)
                    .map_err(|error| format!("skill {}: {error}", entry.name))?;
                let resource = Resource::new(&uri, &frontmatter.name)
                    .with_description(&frontmatter.description)
                    .with_mime_type(MARKDOWN);
                let files = vec![SkillResource::from_bytes(&uri, entry.body.as_bytes())];
                let skill = Skill::new(&uri, frontmatter, SkillResources::Files(files));
                Ok(SkillFile {
                    uri,
                    text: entry.body,
                    skill,
                    resource,
                })
            })
            .collect::<Result<_, String>>()?;
        Ok(Self { files })
    }

    pub(super) fn resources(&self) -> Vec<Resource> {
        self.files
            .iter()
            .map(|file| file.resource.clone())
            .collect()
    }

    /// The text served at exactly `uri`.
    pub(super) fn read(&self, uri: &str) -> Option<&'static str> {
        self.files
            .iter()
            .find(|file| file.uri == uri)
            .map(|file| file.text)
    }

    pub(super) fn skills(&self) -> Vec<Skill> {
        self.files.iter().map(|file| file.skill.clone()).collect()
    }

    /// The entry whose `SKILL.md` is served at exactly `uri`.
    pub(super) fn skill(&self, uri: &str) -> Option<&Skill> {
        self.files
            .iter()
            .find(|file| file.uri == uri)
            .map(|file| &file.skill)
    }
}

/// Reads the `name` and `description` lines between the opening and closing `---` fences. A
/// plain `key: value` reader is enough: the embedded files carry only those two keys.
pub(super) fn parse_frontmatter(text: &str) -> Result<SkillFrontmatter, String> {
    let mut lines = text.lines();
    if lines.next() != Some(FRONTMATTER_FENCE) {
        return Err("the text does not open with a frontmatter".to_string());
    }
    let (mut name, mut description, mut closed) = (None, None, false);
    for line in lines {
        if line == FRONTMATTER_FENCE {
            closed = true;
            break;
        }
        match line.split_once(':') {
            Some(("name", value)) => name = Some(value.trim()),
            Some(("description", value)) => description = Some(value.trim()),
            _ => {}
        }
    }
    if !closed {
        return Err("the frontmatter is not closed".to_string());
    }
    let required = |key: &str, value: Option<&str>| match value {
        Some(value) if !value.is_empty() => Ok(value.to_string()),
        _ => Err(format!("the frontmatter needs a non-empty `{key}`")),
    };
    Ok(SkillFrontmatter::new(
        required("name", name)?,
        required("description", description)?,
    ))
}
