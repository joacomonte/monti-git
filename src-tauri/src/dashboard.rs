//! The dashboard: in every repo, the branches with a commit in the last days, each with its card facts (behind the
//! base, where it's merged, commits not pushed), against that repo's own base and environments.
use crate::branches::{self, Status};
use crate::{git, par, refs};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
        // Work waiting on this computer always shows, however old its last commit: the checked-out branch with
        // uncommitted work (even the base or an environment), and branches with commits not pushed yet.
        let status = git::status(repo).ok();
        let changed = status.as_ref().map_or(0, |s| s.files.len());
        let current = status.and_then(|s| s.branch);
        let dirty: Option<String> = current.clone().filter(|_| changed > 0);
        let keep: Vec<String> = dirty.iter().cloned().chain(refs::ahead_of_upstream(repo)).collect();
        let recent: Vec<refs::Recent> =
            refs::recent(repo, since, only_mine, &keep).into_iter().filter(|b| dirty.as_deref() == Some(b.name.as_str()) || !is_target(&b.name)).collect();
        // A target has nothing to compare with (it's what the others compare against): no status, so no merge line or Safe to delete.
        let names: Vec<String> = recent.iter().filter(|b| !is_target(&b.name)).map(|b| b.name.clone()).collect();
        let mut statuses = statuses_of(repo, &names, base.as_deref(), &targets);
        let branches = recent.into_iter().map(|branch| Active { status: statuses.remove(&branch.name), branch }).collect();
        RepoActivity { repo: picks.path.clone(), base, current, changed, branches }
    });
    all.retain(|r| !r.branches.is_empty());
    all.sort_by_key(|r| std::cmp::Reverse(r.branches[0].branch.date));
    all
}

/// Each name's card facts against the base and targets (none without a base).
fn statuses_of(repo: &Path, names: &[String], base: Option<&str>, targets: &[String]) -> HashMap<String, Status> {
    match base {
        Some(base) if !names.is_empty() => branches::status(repo, names, base, targets).into_iter().map(|s| (s.reference().to_string(), s)).collect(),
        _ => HashMap::new(),
    }
}

/// One branch safe to delete, with its repo.
#[derive(Serialize)]
pub struct Safe {
    repo: String,
    #[serde(flatten)]
    branch: Active,
}

/// A page of Safe to delete; `next`: where the following page starts (None: that was all).
#[derive(Serialize)]
pub struct SafePage {
    items: Vec<Safe>,
    next: Option<usize>,
}

/// Dashboard › Safe to delete, a page at a time: every repo's branches newest first (a cheap listing), checked in
/// chunks from `start` (the expensive part) until `limit` are found merged everywhere. A page only pays for the
/// branches it walked; the next one starts where it stopped.
pub fn safe_to_delete(repos: &[RepoPicks], only_mine: bool, start: usize, limit: usize) -> SafePage {
    const CHUNK: usize = 40;
    // Per repo: its base and targets, and its branches minus those (the cards' rule).
    let per_repo: Vec<(Option<String>, Vec<String>, Vec<refs::Recent>)> = par::map(repos, |picks| {
        let repo = Path::new(&picks.path);
        let base = refs::main_ref(repo, picks.preferred.as_deref()).map(|(name, _)| name);
        let environments = refs::branches(repo, base.as_deref(), picks.environments.as_deref()).environments;
        let targets: Vec<String> = base.iter().cloned().chain(environments).collect();
        let is_target = |name: &str| targets.iter().any(|t| refs::local_name(t) == refs::local_name(name));
        let recent = refs::recent(repo, i64::MIN, only_mine, &[]).into_iter().filter(|b| !is_target(&b.name)).collect();
        (base, targets, recent)
    });
    let mut all: Vec<(usize, &refs::Recent)> = per_repo.iter().enumerate().flat_map(|(i, (_, _, list))| list.iter().map(move |b| (i, b))).collect();
    all.sort_by(|a, b| b.1.date.cmp(&a.1.date).then(a.0.cmp(&b.0)).then(a.1.name.cmp(&b.1.name))); // stable across pages
    let mut items = vec![];
    let mut at = start;
    while at < all.len() && items.len() < limit {
        let chunk = &all[at..(at + CHUNK).min(all.len())];
        let mut found: HashMap<(usize, String), Status> = HashMap::new();
        for (i, (base, targets, _)) in per_repo.iter().enumerate() {
            let names: Vec<String> = chunk.iter().filter(|(r, _)| *r == i).map(|(_, b)| b.name.clone()).collect();
            found.extend(statuses_of(Path::new(&repos[i].path), &names, base.as_deref(), targets).into_iter().map(|(name, s)| ((i, name), s)));
        }
        for (r, b) in chunk {
            at += 1;
            if let Some(status) = found.remove(&(*r, b.name.clone())).filter(Status::merged_everywhere) {
                items.push(Safe { repo: repos[*r].path.clone(), branch: Active { branch: (*b).clone(), status: Some(status) } });
                if items.len() == limit {
                    break;
                }
            }
        }
    }
    SafePage { items, next: (at < all.len()).then_some(at) }
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
        let found = crate::refs::recent(repo.path(), week, false, &[]);
        let feat = found.iter().find(|b| b.name == "feat").expect("feat is in");
        assert_eq!(feat.subject, "teammate push");
        assert!(!found.iter().any(|b| b.name == "origin/feat"), "one card per branch");
    }

    /// Safe to delete comes a page at a time, newest first, and the next page picks up where the last one stopped.
    #[test]
    fn safe_to_delete_pages() {
        let repo = Repo::new("dashboard-safe");
        repo.commit("a.txt", "1", "init");
        for name in ["old", "mid", "new"] {
            repo.git(&["checkout", "-q", "-b", name, "main"]);
            repo.commit(&format!("{name}.txt"), "1", name);
            repo.git(&["checkout", "-q", "main"]);
            repo.git(&["merge", "-q", "--no-ff", "-m", &format!("merge {name}"), name]);
        }
        repo.git(&["checkout", "-q", "-b", "open", "main"]);
        repo.commit("open.txt", "1", "not merged");
        let picks = vec![super::RepoPicks { path: repo.path().display().to_string(), preferred: Some("main".into()), environments: Some(vec![]) }];
        let first = super::safe_to_delete(&picks, false, 0, 2);
        assert_eq!(first.items.len(), 2);
        let rest = super::safe_to_delete(&picks, false, first.next.expect("more to look at"), 2);
        let names: Vec<&str> = first.items.iter().chain(&rest.items).map(|s| s.branch.branch.name.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(sorted, ["mid", "new", "old"], "all merged ones, never the open one");
        assert_eq!(rest.next, None);
    }
}
