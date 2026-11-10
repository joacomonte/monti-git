//! Stashes: every one as a graph row forking off the commit it was saved on, and what can be done with it (apply,
//! pop, drop, and the Undo of a drop). Rows and actions name a stash by its commit, never by "stash@{n}": that
//! number shifts whenever another stash is pushed or dropped.
use crate::actions::exclusive;
use crate::git::{read_opt, write, Commit, Stat};
use crate::wip;
use serde::Serialize;
use std::path::Path;

/// One stash: its reference right now ("stash@{2}"), commit, the commit it was saved on, date and message.
pub struct Stash {
    pub reference: String,
    pub id: String,
    base: String,
    date: i64,
    pub message: String, // "On main: my message", "WIP on main: abc123 subject"
}

/// What a stash row is: `shelved`, the branch whose work Monti Git put aside when leaving it (it comes back on
/// checking that branch out); None for any other stash.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct StashRow {
    pub shelved: Option<String>,
}

pub fn list(repo: &Path) -> Vec<Stash> {
    let list = read_opt(&["stash", "list", "--format=%gd%x1f%H%x1f%P%x1f%ct%x1f%gs"], repo).unwrap_or_default();
    list.lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.splitn(5, '\x1f').collect();
            let [reference, id, parents, date, message] = f[..] else { return None };
            Some(Stash { reference: reference.into(), id: id.into(), base: parents.split(' ').next()?.into(), date: date.parse().unwrap_or_default(), message: message.into() })
        })
        .collect()
}

/// The stashes as graph rows forking off the commit they were saved on.
pub fn rows(repo: &Path) -> Vec<Commit> {
    list(repo)
        .into_iter()
        .map(|s| Commit {
            stat: Some(stat(&s.id, repo)),
            stash: Some(StashRow { shelved: wip::shelved_branch(&s.message).map(String::from) }),
            parents: vec![s.base],
            author: String::new(),
            date: s.date,
            refs: vec![],
            subject: s.message,
            id: s.id,
        })
        .collect()
}

fn reference_of(id: &str, repo: &Path) -> Result<Stash, String> {
    list(repo).into_iter().find(|s| s.id == id).ok_or_else(|| "That stash isn't there anymore".to_string())
}

/// Puts a stash's changes back into the working tree; `pop` drops it once applied. Only onto a clean tree: then a
/// conflicting apply can be undone completely (and is), without touching anything of yours.
pub fn apply(id: &str, pop: bool, repo: &Path) -> Result<String, String> {
    exclusive(repo, || {
        let stash = reference_of(id, repo)?;
        if !crate::git::status(repo)?.files.is_empty() {
            return Err("You have uncommitted changes: commit or stash them first, so applying can't mix with them".into());
        }
        if !crate::actions::apply_cleanly(&stash.reference, repo) {
            return Err("It conflicts with the code here: nothing was changed, the stash is still there".into());
        }
        if pop {
            write(&["stash", "drop", &stash.reference], repo)?;
            return Ok("Applied and dropped the stash".into());
        }
        Ok("Applied the stash (it's still saved)".into())
    })
}

/// Deletes a stash. Its commit stays in git for a while, which is what the Undo (`restore`) brings back.
pub fn drop(id: &str, repo: &Path) -> Result<String, String> {
    exclusive(repo, || {
        let stash = reference_of(id, repo)?;
        write(&["stash", "drop", &stash.reference], repo)?;
        Ok(stash.message)
    })
}

/// Undo of a drop: the stash back in the list, with its message.
pub fn restore(id: &str, message: &str, repo: &Path) -> Result<(), String> {
    exclusive(repo, || write(&["stash", "store", "-m", message, id], repo).map(|_| ()))
}

/// Files and lines of a stash, untracked files included. A stash never changes: cached by its hash.
fn stat(id: &str, repo: &Path) -> Stat {
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
    fn every_stash_is_a_row_off_its_commit() {
        let repo = Repo::new("stashes");
        repo.commit("a.txt", "one\n", "init");
        let head = repo.git(&["rev-parse", "HEAD"]).trim().to_string();
        std::fs::write(repo.path().join("a.txt"), "one\ntwo\n").unwrap();
        std::fs::write(repo.path().join("new.txt"), "x\n").unwrap();
        repo.git(&["stash", "push", "--include-untracked", "-m", &crate::wip::shelve_message("main")]);
        std::fs::write(repo.path().join("a.txt"), "one\nthree\n").unwrap();
        repo.git(&["stash", "push", "-m", "mine"]);
        let rows = super::rows(repo.path());
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].stash.as_ref().unwrap().shelved, None);
        assert_eq!(rows[0].subject, "On main: mine");
        assert_eq!(rows[1].stash.as_ref().unwrap().shelved.as_deref(), Some("main"));
        assert_eq!(rows[1].parents, [head]);
        assert_eq!(rows[1].stat, Some(crate::git::Stat { files: 2, added: 2, removed: 0 }));
        assert_eq!(crate::wip::shelved_stash("main", repo.path()).as_deref(), Some("stash@{1}"));

        // Drop, then Undo brings it back; apply keeps it, pop takes it.
        let mine = rows[0].id.clone();
        let message = super::drop(&mine, repo.path()).unwrap();
        assert_eq!(super::list(repo.path()).len(), 1);
        super::restore(&mine, &message, repo.path()).unwrap();
        assert_eq!(super::list(repo.path())[0].id, mine);
        super::apply(&mine, false, repo.path()).unwrap();
        assert_eq!(std::fs::read_to_string(repo.path().join("a.txt")).unwrap(), "one\nthree\n");
        assert!(super::apply(&mine, true, repo.path()).is_err(), "uncommitted changes: refused");
        repo.git(&["checkout", "--", "a.txt"]);
        super::apply(&mine, true, repo.path()).unwrap();
        assert_eq!(super::list(repo.path()).len(), 1, "popped");
    }
}
