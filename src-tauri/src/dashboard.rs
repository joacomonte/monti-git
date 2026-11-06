//! The dashboard: in every repo, the branches with a commit in the last days, each with its card facts (behind the
//! base, where it's merged, commits not pushed), against that repo's own base and environments.
use crate::branches::{self, Status};
use crate::{git, par, refs};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// A repo and the user's picks for it (prefs "mainBranches" and "environments"; None = detected).
#[derive(Deserialize)]
pub struct RepoPicks {
    path: String,
    preferred: Option<String>,
    environments: Option<Vec<String>>,
}

#[derive(Serialize)]
pub struct RepoActivity {
    repo: String,
    base: Option<String>,
    current: Option<String>, // the checked-out branch
    changed: usize,          // files with uncommitted changes on it: its card offers Commit first
    branches: Vec<Active>,
}

#[derive(Serialize)]
pub struct Active {
    #[serde(flatten)]
    branch: refs::Recent,
    status: Option<Status>, // None without a base to compare with
}

/// Newest repo first (by its newest branch); repos with nothing in the window are left out. `only_mine`: only the
/// branches whose newest commit is yours.
pub fn activity(repos: &[RepoPicks], days: u32, only_mine: bool) -> Vec<RepoActivity> {
    let since = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64) - i64::from(days) * 86_400;
    let mut all: Vec<RepoActivity> = par::map(repos, |picks| {
        let repo = Path::new(&picks.path);
        let base = refs::main_ref(repo, picks.preferred.as_deref()).map(|(name, _)| name);
        let environments = refs::branches(repo, base.as_deref(), picks.environments.as_deref()).environments;
        let targets: Vec<String> = base.iter().cloned().chain(environments.iter().cloned()).collect();
        // The base and the environments are what the others compare against: not cards of their own.
        let is_target = |name: &str| targets.iter().any(|t| refs::local_name(t) == refs::local_name(name));
        // The checked-out branch with uncommitted work always shows, however old its last commit and even when it's the
        // base or an environment: that work is recent.
        let status = git::status(repo).ok();
        let changed = status.as_ref().map_or(0, |s| s.files.len());
        let current = status.and_then(|s| s.branch);
        let keep = current.as_deref().filter(|_| changed > 0);
        let recent: Vec<refs::Recent> =
            refs::recent(repo, since, only_mine, keep).into_iter().filter(|b| keep == Some(b.name.as_str()) || !is_target(&b.name)).collect();
        let names: Vec<String> = recent.iter().filter(|b| !is_target(&b.name)).map(|b| b.name.clone()).collect();
        let mut statuses = match &base {
            Some(base) if !names.is_empty() => branches::status(repo, &names, base, &targets),
            _ => vec![],
        }
        .into_iter();
        // A target has nothing to compare with (it's what the others compare against): no status, so no merge line or Safe to delete.
        let branches = recent
            .into_iter()
            .map(|branch| {
                let status = if is_target(&branch.name) { None } else { base.as_ref().and_then(|_| statuses.next()) };
                Active { branch, status }
            })
            .collect();
        RepoActivity { repo: picks.path.clone(), base, current, changed, branches }
    });
    all.retain(|r| !r.branches.is_empty());
    all.sort_by_key(|r| std::cmp::Reverse(r.branches[0].branch.date));
    all
}

#[cfg(test)]
mod tests {
    use crate::testutil::Repo;

    #[test]
    fn recent_branches_leave_the_base_out() {
        let repo = Repo::new("dashboard");
        repo.commit("a.txt", "1", "init");
        repo.git(&["checkout", "-q", "-b", "feat/new"]);
        repo.commit("b.txt", "1", "fresh work");
        let picks = vec![super::RepoPicks { path: repo.path().display().to_string(), preferred: Some("main".into()), environments: Some(vec![]) }];
        let found = super::activity(&picks, 7, false);
        assert_eq!(found.len(), 1);
        let names: Vec<&str> = found[0].branches.iter().map(|b| b.branch.name.as_str()).collect();
        assert_eq!(names, ["feat/new"], "main is the base: not a card");
        assert_eq!(found[0].branches[0].branch.subject, "fresh work");
        assert!(found[0].branches[0].status.is_some());
    }

    /// Your local copy is old but a teammate pushed to it this week: it shows, named by your branch, with their commit.
    #[test]
    fn a_newer_server_copy_keeps_the_branch_in() {
        let repo = Repo::new("dashboard-teammate");
        repo.commit("a.txt", "1", "init");
        repo.git(&["checkout", "-q", "-b", "feat"]);
        std::fs::write(repo.path().join("b.txt"), "1").unwrap();
        repo.git(&["add", "b.txt"]);
        let old = std::process::Command::new("git").arg("-C").arg(repo.path()).args(["commit", "-q", "-m", "old local"])
            .env("GIT_COMMITTER_DATE", "2020-01-01T00:00:00").env("GIT_AUTHOR_DATE", "2020-01-01T00:00:00").status().unwrap();
        assert!(old.success());
        repo.git(&["checkout", "-q", "-b", "tmp"]);
        repo.commit("b.txt", "2", "teammate push");
        repo.git(&["update-ref", "refs/remotes/origin/feat", "HEAD"]);
        repo.git(&["checkout", "-q", "feat"]);
        repo.git(&["branch", "-q", "-D", "tmp"]);
        let week = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64 - 7 * 86_400;
        let found = crate::refs::recent(repo.path(), week, false, None);
        let feat = found.iter().find(|b| b.name == "feat").expect("feat is in");
        assert_eq!(feat.subject, "teammate push");
        assert!(!found.iter().any(|b| b.name == "origin/feat"), "one card per branch");
    }
}
