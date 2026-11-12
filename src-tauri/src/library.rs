//! Repos shown in the sidebar: every git repo directly inside the folders the user added, plus the repos added one by one.
use crate::{git, par};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoEntry {
    pub path: String,
    pub owner: String,
    pub last_activity: i64,     // unix seconds of the newest local branch commit, 0 = no commits yet
    pub folder: Option<String>, // the added folder it was found in; None = added on its own
    pub changes: Option<String>, // uncommitted changes, as a fingerprint that moves whenever they do; None = clean
}

/// .git is a folder in normal repos and a file in worktrees: both count.
pub fn is_repo(path: &Path) -> bool {
    path.join(".git").exists()
}

/// `hidden`: repos inside an added folder that the user took out of the sidebar one by one.
pub fn discover(folders: &[String], repos: &[String], hidden: &[String]) -> Vec<RepoEntry> {
    let mut found: Vec<(PathBuf, Option<String>)> = repos.iter().map(|r| (PathBuf::from(r), None)).collect();
    for folder in folders {
        let children = std::fs::read_dir(folder).into_iter().flatten().flatten().map(|e| e.path());
        found.extend(children.filter(|p| !hidden.iter().any(|h| Path::new(h) == p)).map(|p| (p, Some(folder.clone()))));
    }
    found.retain(|(p, _)| is_repo(p));
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found.dedup_by(|a, b| a.0 == b.0); // added on its own and inside a folder: listed once
    // Three git calls per repo, a few repos at a time, so a long list doesn't add up their latency (worst on Windows).
    par::map(&found, |(p, folder)| RepoEntry {
        path: p.to_string_lossy().into(),
        owner: owner(p),
        last_activity: last_activity(p),
        folder: folder.clone(),
        changes: changes(p),
    })
}

/// The changed files with their status and modification time, hashed: the same while nobody touches them, a new
/// value as soon as a file is edited, added, staged or reverted. None when there's nothing uncommitted.
fn changes(repo: &Path) -> Option<String> {
    let status = git::status(repo).ok()?;
    if status.files.is_empty() {
        return None;
    }
    // FNV-1a: stable across builds, so a fingerprint saved by one version still matches in the next.
    let mut hash: u64 = 0xcbf29ce484222325;
    let mut feed = |bytes: &[u8]| bytes.iter().for_each(|b| hash = (hash ^ *b as u64).wrapping_mul(0x100000001b3));
    for (path, letter) in &status.files {
        let modified = git::repo_file(repo, path).ok().and_then(|f| f.metadata().ok()?.modified().ok());
        let nanos = modified.and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos());
        feed(path.as_bytes());
        feed(&[*letter as u8]);
        feed(&nanos.to_le_bytes());
    }
    feed(status.staged.join("\0").as_bytes());
    Some(format!("{hash:016x}"))
}

fn last_activity(repo: &Path) -> i64 {
    git::read_opt(&["for-each-ref", "--sort=-committerdate", "--count=1", "--format=%(committerdate:unix)", "refs/heads"], repo)
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

/// "git@github.com:owner/repo.git" or "https://github.com/owner/repo" -> "owner".
fn owner(repo: &Path) -> String {
    let Some(url) = git::read_opt(&["remote", "get-url", "origin"], repo) else { return "Local".into() };
    let parts: Vec<&str> = url.trim().split(['/', ':']).filter(|s| !s.is_empty()).collect();
    if parts.len() >= 2 { parts[parts.len() - 2].into() } else { "Local".into() }
}

#[cfg(test)]
mod tests {
    use crate::testutil::Repo;

    #[test]
    fn a_hidden_repo_leaves_its_folder_but_not_when_added_on_its_own() {
        let a = Repo::new("library-hidden-a");
        let b = Repo::new("library-hidden-b");
        let folder = a.path().parent().unwrap().to_string_lossy().to_string();
        let hidden = [a.path().to_string_lossy().to_string()];
        let paths = |repos: &[String]| super::discover(&[folder.clone()], repos, &hidden).into_iter().map(|r| r.path).collect::<Vec<_>>();
        assert!(!paths(&[]).contains(&hidden[0]) && paths(&[]).contains(&b.path().to_string_lossy().to_string()));
        assert!(paths(&hidden).contains(&hidden[0]), "added on its own: listed again");
    }

    #[test]
    fn changes_fingerprint_moves_with_the_files() {
        let repo = Repo::new("library-changes");
        repo.commit("a.txt", "1", "init");
        assert_eq!(super::changes(repo.path()), None);
        std::fs::write(repo.path().join("a.txt"), "2").unwrap();
        let first = super::changes(repo.path()).unwrap();
        assert_eq!(super::changes(repo.path()).as_deref(), Some(first.as_str()), "untouched: same fingerprint");
        std::fs::write(repo.path().join("b.txt"), "new").unwrap();
        assert_ne!(super::changes(repo.path()).unwrap(), first, "a new file: new fingerprint");
    }
}
