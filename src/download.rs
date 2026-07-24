#![allow(dead_code)]

use crate::client::JiraClient;
use crate::models::{Attachment, Issue};
use anyhow::Result;
use std::path::{Path, PathBuf};

/// Where downloaded attachments for `issue_key` are written:
/// `root/downloads/issue_key` (ADR 0029 §2, BDR 0020 S4).
pub fn download_dir_for(root: &Path, issue_key: &str) -> PathBuf {
    root.join("downloads").join(issue_key)
}

/// Disambiguate `filename` against already-`taken` names by inserting
/// ` (2)`, ` (3)`, … before the extension until the result is unused (BDR
/// 0020 S6). Never returns a name already present in `taken`.
pub fn dedupe_filename(taken: &[String], filename: &str) -> String {
    if !taken.iter().any(|t| t == filename) {
        return filename.to_owned();
    }
    let (stem, ext) = split_extension(filename);
    let mut suffix = 2;
    loop {
        let candidate = format!("{stem} ({suffix}){ext}");
        if !taken.iter().any(|t| t == &candidate) {
            return candidate;
        }
        suffix += 1;
    }
}

/// Split `filename` into `(stem, ".ext")`. A leading-dot dotfile (`stem`
/// empty before the last `.`) or a filename with no `.` keeps the whole name
/// as the stem with an empty extension.
fn split_extension(filename: &str) -> (String, String) {
    match filename.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem.to_owned(), format!(".{ext}")),
        _ => (filename.to_owned(), String::new()),
    }
}

/// One attachment successfully written to disk by [`download_all`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedAttachment {
    pub filename: String,
    pub path: PathBuf,
    pub bytes: u64,
}

/// Download every attachment on `issue` through `client`'s same-origin seam
/// and write each to `dir` (created if absent), disambiguating duplicate
/// filenames as they land (BDR 0020 S4, S6, S7). Zero attachments writes no
/// files and returns an empty `Vec`.
pub async fn download_all(
    client: &dyn JiraClient,
    issue: &Issue,
    dir: &Path,
) -> Result<Vec<SavedAttachment>> {
    if issue.attachments.is_empty() {
        return Ok(Vec::new());
    }
    std::fs::create_dir_all(dir)?;

    let mut taken = Vec::with_capacity(issue.attachments.len());
    let mut saved = Vec::with_capacity(issue.attachments.len());
    for attachment in &issue.attachments {
        let one = download_one(client, attachment, dir, &taken).await?;
        taken.push(one.filename.clone());
        saved.push(one);
    }
    Ok(saved)
}

async fn download_one(
    client: &dyn JiraClient,
    attachment: &Attachment,
    dir: &Path,
    taken: &[String],
) -> Result<SavedAttachment> {
    let bytes = client
        .download_attachment(&attachment.url)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let filename = dedupe_filename(taken, &attachment.filename);
    let path = dir.join(&filename);
    std::fs::write(&path, &bytes)?;
    Ok(SavedAttachment {
        filename,
        bytes: bytes.len() as u64,
        path,
    })
}

/// One `saved <path> (<bytes>)` line per file, joined by newlines (BDR 0020
/// S5 human mode).
pub fn format_saved_human(saved: &[SavedAttachment]) -> String {
    saved
        .iter()
        .map(|s| format!("saved {} ({})", s.path.display(), s.bytes))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Minified curated JSON object listing each saved attachment's
/// `{ filename, path, bytes }` (BDR 0020 S5 `--json` mode).
pub fn saved_to_json(issue_key: &str, saved: &[SavedAttachment]) -> String {
    let items: Vec<serde_json::Value> = saved
        .iter()
        .map(|s| {
            serde_json::json!({
                "filename": s.filename,
                "path": s.path.to_string_lossy(),
                "bytes": s.bytes,
            })
        })
        .collect();
    let obj = serde_json::json!({
        "issue_key": issue_key,
        "saved": items,
    });
    serde_json::to_string(&obj).expect("saved attachment object is always serialisable")
}

/// The clean no-op message for an issue with zero attachments (BDR 0020 S7).
pub fn no_attachments_message(issue_key: &str) -> String {
    format!("{issue_key} has no attachments.")
}

#[cfg(test)]
#[path = "../tests/unit/download.rs"]
mod tests;
