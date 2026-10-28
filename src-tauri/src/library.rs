//! Repos shown in the sidebar: every git repo directly inside the scan folder, plus the ones opened by hand.
use crate::git;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
pub struct RepoEntry {
    pub path: String,
    pub owner: String,
    #[serde(rename = "lastActivity")]
    pub last_activity: i64, // unix seconds of the newest local branch commit, 0 = no commits yet
}

pub fn default_folder() -> PathBuf {
    let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_default();
    Path::new(&home).join("Documents").join("Dev")
}

pub fn discover(folder: &Path, extras: &[String]) -> Vec<RepoEntry> {
    let scanned = std::fs::read_dir(folder).into_iter().flatten().flatten().map(|e| e.path());
    let mut paths: Vec<PathBuf> = scanned.chain(extras.iter().map(PathBuf::from))
        .filter(|p| p.join(".git").exists()) // .git is a folder in normal repos and a file in worktrees: both count
        .collect();
    paths.sort();
    paths.dedup();
    paths.iter().map(|p| RepoEntry { path: p.to_string_lossy().into(), owner: owner(p), last_activity: last_activity(p) }).collect()
}

fn last_activity(repo: &Path) -> i64 {
    git::run(&["for-each-ref", "--sort=-committerdate", "--count=1", "--format=%(committerdate:unix)", "refs/heads"], repo)
        .ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0)
}

/// "git@github.com:owner/repo.git" or "https://github.com/owner/repo" -> "owner".
fn owner(repo: &Path) -> String {
    let Ok(url) = git::run(&["remote", "get-url", "origin"], repo) else { return "Local".into() };
    let parts: Vec<&str> = url.trim().split(['/', ':']).filter(|s| !s.is_empty()).collect();
    if parts.len() >= 2 { parts[parts.len() - 2].into() } else { "Local".into() }
}
