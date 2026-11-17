//! "Update from main": which way to bring the base's new commits into a branch (merge, rebase, or squash then rebase),
//! and why in one line. Only reads: the merge and the rebase are simulated in memory (`merge-tree`, `replay`), so the
//! branch, the checkout and the remote stay untouched. Judged on the last fetch.
use crate::actions::Method;
use crate::git::{self, read_opt, run_output};
use crate::refs;
use serde::Serialize;
use std::path::Path;

/// The facts the panel shows, and what they add up to.
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Advice {
    pub recommended: Option<Method>, // None: uncommitted changes in the way, nothing to pick yet
    pub reason: String,              // why, in one line
    pub own: usize,                  // the branch's commits not in the base
    pub incoming: usize,             // the base's commits not in the branch
    pub dirty: usize,                // uncommitted tracked files, when it's the checked-out branch
    pub pushed: bool,                // on the server under its own name: a rebase or squash needs a force push after
    pub others: Vec<String>,         // other people who pushed commits to it: rewriting them breaks their copy
    pub merges: usize,               // merge commits already in the branch (from the base or elsewhere)
    pub merge_conflicts: Vec<String>, // files a merge (and a squash + rebase: the same 3-way merge) stops on
    pub rebase_conflicts: Option<bool>, // a commit-by-commit replay stops somewhere; None: not simulated (merge commits)
}

pub fn advise(branch: &str, parent: &str, repo: &Path) -> Advice {
    let own = count(&[&format!("{parent}..{branch}")], repo);
    let merges = count(&["--merges", &format!("{parent}..{branch}")], repo);
    let dirty = if refs::current_branch(repo).as_deref() == Some(branch) {
        git::status(repo).map_or(0, |s| s.files.len().saturating_sub(s.untracked.len()))
    } else {
        0
    };
    let upstream = namesake_upstream(branch, repo);
    let mut advice = Advice {
        recommended: None,
        reason: String::new(),
        own,
        incoming: count(&[&format!("{branch}..{parent}")], repo),
        dirty,
        pushed: upstream.is_some(),
        others: upstream.map_or_else(Vec::new, |u| other_authors(&u, parent, repo)),
        merges,
        merge_conflicts: merge_conflicts(branch, parent, repo),
        // replay can't carry merge commits (it fails as if on a conflict), and those branches get Merge anyway.
        rebase_conflicts: if merges > 0 { None } else { rebase_conflicts(branch, parent, repo) },
    };
    (advice.recommended, advice.reason) = pick(&advice, parent);
    advice
}

/// The rules, in order: the first that applies decides. The reason is one short line: the method shows beside it, and
/// the force push a rewrite of a pushed branch needs is shown apart.
fn pick(a: &Advice, parent: &str) -> (Option<Method>, String) {
    let commits = plural(a.own, "commit");
    if a.dirty > 0 {
        return (None, format!("You have {}. Commit or stash them first.", plural(a.dirty, "uncommitted change")));
    }
    if a.own == 0 {
        return (Some(Method::Rebase), format!("No commits of yours yet: your branch just moves up to {parent}."));
    }
    if !a.others.is_empty() {
        return (Some(Method::Merge), format!("{} also pushed here. A rebase would rewrite their commits.", a.others.join(", ")));
    }
    if a.merges > 0 {
        return (Some(Method::Merge), "Your branch already has merge commits. A rebase would flatten them.".into());
    }
    let merge_clean = a.merge_conflicts.is_empty();
    // Without a replay simulation, a clean 3-way merge is the best guess for the rebase too.
    if !a.rebase_conflicts.unwrap_or(!merge_clean) {
        let whose = if a.pushed { "Only you pushed this branch" } else { "Your branch isn't pushed yet" };
        return (Some(Method::Rebase), format!("{whose}, and your {commits} apply cleanly on {parent}."));
    }
    if merge_clean && a.own > 1 {
        return (Some(Method::SquashRebase), format!("Your {commits} clash one by one, but apply cleanly as one."));
    }
    let files = plural(a.merge_conflicts.len(), "file");
    if a.own == 1 {
        return (Some(Method::Rebase), format!("Conflicts in {files} either way. With 1 commit you resolve them once."));
    }
    (Some(Method::Merge), format!("Conflicts in {files}. A merge resolves them once; a rebase, maybe once per commit."))
}

fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

fn count(range: &[&str], repo: &Path) -> usize {
    let args: Vec<&str> = ["rev-list", "--count"].iter().copied().chain(range.iter().copied()).collect();
    read_opt(&args, repo).and_then(|s| s.trim().parse().ok()).unwrap_or(0)
}

/// The branch's copy on the server under its own name (what Push updates), unless it was deleted there.
fn namesake_upstream(branch: &str, repo: &Path) -> Option<String> {
    let out = read_opt(&["for-each-ref", "--format=%(upstream)%09%(upstream:track,nobracket)", &format!("refs/heads/{branch}")], repo)?;
    let (upstream, track) = out.trim_end_matches('\n').split_once('\t')?;
    (upstream.ends_with(&format!("/{branch}")) && track != "gone").then(|| upstream.to_string())
}

/// Authors other than the user of the pushed commits that aren't in the base.
fn other_authors(upstream: &str, parent: &str, repo: &Path) -> Vec<String> {
    let is_me = refs::me(repo);
    let out = read_opt(&["log", "--format=%an%x09%ae", upstream, "--not", parent], repo).unwrap_or_default();
    let mut others: Vec<String> = out.lines().filter_map(|l| l.split_once('\t')).filter(|(n, e)| !is_me(n, e)).map(|(n, _)| n.to_string()).collect();
    others.sort();
    others.dedup();
    others
}

/// The files a 3-way merge of the two would stop on; empty when it's clean (or git couldn't tell).
fn merge_conflicts(branch: &str, parent: &str, repo: &Path) -> Vec<String> {
    let Ok(out) = run_output(&["merge-tree", "--write-tree", "--name-only", "--no-messages", branch, parent], repo) else { return Vec::new() };
    if out.status.code() != Some(1) {
        return Vec::new();
    }
    // First line: the tree it would write; then one conflicted path per line.
    let mut files: Vec<String> = String::from_utf8_lossy(&out.stdout).lines().skip(1).filter(|l| !l.is_empty()).map(String::from).collect();
    files.dedup();
    files
}

/// Replays the branch's commits onto the base one by one, in memory, like a rebase: does any of them conflict?
/// None when git can't tell (a git without `replay`).
fn rebase_conflicts(branch: &str, parent: &str, repo: &Path) -> Option<bool> {
    // --ref-action=print: without it, replay itself moves the branch.
    let out = run_output(&["replay", "--ref-action=print", "--onto", parent, &format!("{parent}..{branch}")], repo).ok()?;
    match out.status.code() {
        Some(0) => Some(false),
        Some(1) if out.stderr.is_empty() => Some(true),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::advise;
    use crate::actions::Method;
    use crate::testutil::Repo;

    /// main with a.txt and b.txt; `feat` branches off it.
    fn repo(name: &str) -> Repo {
        let repo = Repo::new(name);
        repo.commit("a.txt", "1\n", "init a");
        repo.commit("b.txt", "1\n", "init b");
        repo.git(&["checkout", "-q", "-b", "feat"]);
        repo
    }

    fn on_main(repo: &Repo, file: &str, content: &str) {
        repo.git(&["checkout", "-q", "main"]);
        repo.commit(file, content, &format!("main: {file}"));
        repo.git(&["checkout", "-q", "feat"]);
    }

    /// A server with `feat` pushed and tracked, as Push leaves it.
    fn push(repo: &Repo, server: &Repo) {
        repo.git(&["remote", "add", "origin", &server.path().display().to_string()]);
        repo.git(&["push", "-q", "-u", "origin", "feat"]);
    }

    #[test]
    fn local_branch_without_conflicts_rebases() {
        let repo = repo("advice-local");
        repo.commit("f.txt", "1\n", "mine");
        on_main(&repo, "a.txt", "2\n");
        on_main(&repo, "b.txt", "2\n");
        let advice = advise("feat", "main", repo.path());
        assert_eq!((advice.recommended, advice.own, advice.incoming, advice.pushed), (Some(Method::Rebase), 1, 2, false));
        assert!(advice.merge_conflicts.is_empty() && advice.rebase_conflicts == Some(false));
        assert_eq!(repo.git(&["rev-parse", "feat"]), repo.git(&["rev-parse", "HEAD"]), "replay didn't move the branch");
        assert_eq!(repo.git(&["rev-list", "--count", "main..feat"]).trim(), "1");
    }

    #[test]
    fn branch_someone_else_pushed_to_merges() {
        let server = Repo::new("advice-shared-server");
        let repo = repo("advice-shared");
        repo.commit("f.txt", "1\n", "mine");
        repo.git(&["-c", "user.name=Ana", "-c", "user.email=ana@example.com", "commit", "-q", "--allow-empty", "-m", "Ana's"]);
        push(&repo, &server);
        on_main(&repo, "a.txt", "2\n");
        let advice = advise("feat", "main", repo.path());
        assert_eq!((advice.recommended, advice.pushed, advice.others.as_slice()), (Some(Method::Merge), true, ["Ana".to_string()].as_slice()));
    }

    #[test]
    fn pushed_by_me_alone_rebases_and_says_force_push() {
        let server = Repo::new("advice-solo-server");
        let repo = repo("advice-solo");
        for i in 1..=3 {
            repo.commit("f.txt", &format!("{i}\n"), &format!("mine {i}"));
        }
        push(&repo, &server);
        on_main(&repo, "a.txt", "2\n");
        let advice = advise("feat", "main", repo.path());
        assert_eq!((advice.recommended, advice.pushed), (Some(Method::Rebase), true));
        assert!(advice.others.is_empty() && advice.reason.starts_with("Only you pushed"), "{}", advice.reason);
    }

    #[test]
    fn conflicts_only_commit_by_commit_squash() {
        let repo = repo("advice-squash");
        repo.commit("a.txt", "mine\n", "touch a");
        repo.commit("a.txt", "1\n", "put a back");
        on_main(&repo, "a.txt", "main\n");
        let advice = advise("feat", "main", repo.path());
        assert_eq!((advice.recommended, advice.rebase_conflicts), (Some(Method::SquashRebase), Some(true)));
        assert!(advice.merge_conflicts.is_empty());

        crate::actions::update("feat", "main", Method::SquashRebase, repo.path()).unwrap();
        assert_eq!(repo.git(&["rev-list", "--count", "main..feat"]).trim(), "1", "one commit on top of main");
        assert_eq!(repo.git(&["log", "-1", "--format=%P"]).trim(), repo.git(&["rev-parse", "main"]).trim());
        assert_eq!(std::fs::read_to_string(repo.path().join("a.txt")).unwrap(), "main\n");
        assert!(repo.git(&["log", "-1", "--format=%B"]).contains("put a back"), "keeps every message");
    }

    #[test]
    fn many_commits_with_real_conflicts_merge() {
        let repo = repo("advice-many");
        for i in 1..=15 {
            repo.commit("a.txt", &format!("mine {i}\n"), &format!("mine {i}"));
        }
        on_main(&repo, "a.txt", "main\n");
        let advice = advise("feat", "main", repo.path());
        assert_eq!((advice.recommended, advice.merge_conflicts.as_slice()), (Some(Method::Merge), ["a.txt".to_string()].as_slice()));
        assert!(crate::actions::update("feat", "main", Method::SquashRebase, repo.path()).unwrap_err().contains("a.txt"), "squash refuses on conflicts");
        assert_eq!(repo.git(&["rev-list", "--count", "main..feat"]).trim(), "15", "and changes nothing");
    }

    #[test]
    fn branch_with_merges_from_main_merges() {
        let repo = repo("advice-merged");
        repo.commit("f.txt", "1\n", "mine");
        on_main(&repo, "a.txt", "2\n");
        repo.git(&["merge", "-q", "--no-edit", "main"]);
        on_main(&repo, "b.txt", "2\n");
        let advice = advise("feat", "main", repo.path());
        assert_eq!((advice.recommended, advice.merges, advice.rebase_conflicts), (Some(Method::Merge), 1, None));
    }

    #[test]
    fn uncommitted_changes_block_until_committed_or_stashed() {
        let repo = repo("advice-dirty");
        repo.commit("f.txt", "1\n", "mine");
        on_main(&repo, "a.txt", "2\n");
        std::fs::write(repo.path().join("f.txt"), "wip\n").unwrap();
        let advice = advise("feat", "main", repo.path());
        assert_eq!((advice.recommended, advice.dirty), (None, 1));
        repo.git(&["stash", "-q"]);
        assert_eq!(advise("feat", "main", repo.path()).recommended, Some(Method::Rebase));
    }
}
