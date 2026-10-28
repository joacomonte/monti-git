//! Repos shown in the sidebar: every git repo directly inside the folders the user added, plus the repos added one by one.
use crate::git;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
pub struct RepoEntry {
    pub path: String,
    pub owner: String,
    #[serde(rename = "lastActivity")]
    pub last_activity: i64, // unix seconds of the newest local branch commit, 0 = no commits yet
    pub folder: Option<String>, // the added folder it was found in; None = added on its own
}

/// .git is a folder in normal repos and a file in worktrees: both count.
pub fn is_repo(path: &Path) -> bool {
    path.join(".git").exists()
}

pub fn discover(folders: &[String], repos: &[String]) -> Vec<RepoEntry> {
    let mut found: Vec<(PathBuf, Option<String>)> = repos.iter().map(|r| (PathBuf::from(r), None)).collect();
    for folder in folders {
        let children = std::fs::read_dir(folder).into_iter().flatten().flatten().map(|e| e.path());
        found.extend(children.map(|p| (p, Some(folder.clone()))));
    }
    found.retain(|(p, _)| is_repo(p));
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found.dedup_by(|a, b| a.0 == b.0); // added on its own and inside a folder: listed once
    found.into_iter()
        .map(|(p, folder)| RepoEntry { path: p.to_string_lossy().into(), owner: owner(&p), last_activity: last_activity(&p), folder })
        .collect()
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
