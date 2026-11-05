//! How Monti Git marks the work it puts aside when leaving a branch: a stash named after the branch (Shelve),
//! a stash it pops right away (Carry), or a commit with a fixed subject (Commit).
use crate::git::{read_opt, Commit, Stat};
use std::path::Path;

pub const COMMIT_SUBJECT: &str = "WIP [monti-git]";
const SHELVE_PREFIX: &str = "monti-git: ";

pub fn shelve_message(branch: &str) -> String {
    format!("{SHELVE_PREFIX}{branch}")
}

pub fn carry_message(branch: &str) -> String {
    format!("monti-git-carry: {branch}")
}

/// Branch of a stash subject ("On <branch>: monti-git: <branch>"), if it is one of ours.
fn shelved_branch(subject: &str) -> Option<&str> {
    subject.split_once(&format!(": {SHELVE_PREFIX}")).map(|(_, branch)| branch)
}

/// Every stash Monti Git put aside: (reference like "stash@{2}", branch, commit, base commit, date).
fn shelved(repo: &Path) -> Vec<(String, String, String, String, i64)> {
    let list = read_opt(&["stash", "list", "--format=%gd%x1f%H%x1f%P%x1f%ct%x1f%gs"], repo).unwrap_or_default();
    list.lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.splitn(5, '\x1f').collect();
            let branch = shelved_branch(f.get(4)?)?;
            let base = f[2].split(' ').next()?;
            Some((f[0].into(), branch.into(), f[1].into(), base.into(), f[3].parse().unwrap_or_default()))
        })
        .collect()
}

/// The stash saved for `branch` when leaving it with Shelve, e.g. "stash@{2}".
pub fn shelved_stash(branch: &str, repo: &Path) -> Option<String> {
    shelved(repo).into_iter().find(|s| s.1 == branch).map(|s| s.0)
}

/// The stashes as graph rows forking off the commit they were saved on.
pub fn shelved_commits(repo: &Path) -> Vec<Commit> {
    shelved(repo)
        .into_iter()
        .map(|(_, branch, id, base, date)| Commit {
            stat: Some(stash_stat(&id, repo)),
            id,
            parents: vec![base],
            author: String::new(),
            date,
            refs: vec![],
            subject: String::new(),
            shelved: Some(branch),
        })
        .collect()
}

/// Files and lines of a stash, untracked files included. A stash never changes: cached by its hash.
fn stash_stat(id: &str, repo: &Path) -> Stat {
    use std::collections::HashMap;
    use std::sync::{LazyLock, Mutex};
    static CACHE: LazyLock<Mutex<HashMap<String, Stat>>> = LazyLock::new(Mutex::default);
    if let Some(stat) = CACHE.lock().unwrap().get(id) {
        return *stat;
    }
    let Some(out) = read_opt(&["stash", "show", "--include-untracked", "--numstat", "-z", id], repo) else { return Stat::default() };
    let mut stat = Stat::default();
    for record in out.split('\0').filter(|r| r.contains('\t')) {
        let mut f = record.split('\t');
        stat.files += 1;
        stat.added += f.next().and_then(|n| n.parse().ok()).unwrap_or(0); // "-" for binary files
        stat.removed += f.next().and_then(|n| n.parse().ok()).unwrap_or(0);
    }
    CACHE.lock().unwrap().insert(id.to_string(), stat);
    stat
}

#[cfg(test)]
mod tests {
    use crate::testutil::Repo;

    #[test]
    fn shelved_stash_becomes_a_row_off_its_commit() {
        let repo = Repo::new("shelved");
        repo.commit("a.txt", "one\n", "init");
        let head = repo.git(&["rev-parse", "HEAD"]).trim().to_string();
        std::fs::write(repo.path().join("a.txt"), "one\ntwo\n").unwrap();
        std::fs::write(repo.path().join("new.txt"), "x\n").unwrap();
        repo.git(&["stash", "push", "--include-untracked", "-m", &super::shelve_message("main")]);
        repo.git(&["stash", "push", "-m", "someone else's"]); // not ours: ignored (nothing to stash, so a no-op anyway)
        let rows = super::shelved_commits(repo.path());
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].shelved.as_deref(), Some("main"));
        assert_eq!(rows[0].parents, [head]);
        assert_eq!(rows[0].stat, Some(crate::git::Stat { files: 2, added: 2, removed: 0 }));
        assert_eq!(super::shelved_stash("main", repo.path()).as_deref(), Some("stash@{0}"));
    }
}
