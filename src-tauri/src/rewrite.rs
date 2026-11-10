//! "Edit commits": a branch's own commits since its base, reordered, squashed into the one above, reworded or
//! dropped. The new history is built in a temporary worktree, so a conflict changes nothing: the branch only moves
//! once every commit applied. Commits already on the server only with `rewrite`, and then the branch is force-pushed
//! (with lease, like amend).
use crate::actions::{exclusive, on_a_remote, push_branch, Pushed};
use crate::git::{self, read_opt, run, write, write_input};
use crate::refs::{self, current_branch};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// One of the branch's own commits, oldest first. `pushed`: on a server branch.
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Own {
    id: String,
    message: String, // subject, a blank line and the body, as git keeps it
    author: String,
    date: i64,
    pushed: bool,
}

/// The branch's own commits since `base`, oldest first. Refused with merge commits in them: replaying a merge one
/// commit at a time isn't the same history.
pub fn own_commits(branch: &str, base: &str, repo: &Path) -> Result<Vec<Own>, String> {
    let range = [format!("refs/heads/{branch}"), format!("^{base}")];
    if !run(&["rev-list", "--merges", &range[0], &range[1]], repo)?.trim().is_empty() {
        return Err(format!("{branch} has merge commits since {base}: editing them one by one isn't supported. Rebase it on {base} first."));
    }
    let out = run(&["log", "--reverse", "--format=%H%x1f%an%x1f%at%x1f%B%x1e", &range[0], &range[1]], repo)?;
    // Pushes send whole prefixes: once one commit isn't on a server, none after it is.
    let mut pushed = true;
    Ok(out
        .split('\x1e')
        .filter_map(|record| {
            let f: Vec<&str> = record.trim_start_matches('\n').splitn(4, '\x1f').collect();
            let [id, author, date, message] = f[..] else { return None };
            pushed = pushed && on_a_remote(id, repo);
            Some(Own { id: id.into(), message: message.trim_end().into(), author: author.into(), date: date.parse().unwrap_or_default(), pushed })
        })
        .collect())
}

#[derive(Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    Pick,
    Squash, // into the commit kept above it, keeping that one's message
    Drop,
}

/// One line of the new history, oldest first. `message`: a new message (reword), None keeps its own.
#[derive(Deserialize, Debug)]
pub struct Step {
    id: String,
    action: Action,
    message: Option<String>,
}

pub fn apply(branch: &str, base: &str, steps: &[Step], rewrite: bool, repo: &Path) -> Result<String, String> {
    exclusive(repo, || {
        let own = own_commits(branch, base, repo)?;
        let mut asked: Vec<&str> = steps.iter().map(|s| s.id.as_str()).collect();
        let mut have: Vec<&str> = own.iter().map(|c| c.id.as_str()).collect();
        asked.sort_unstable();
        have.sort_unstable();
        if asked != have {
            return Err(format!("{branch} changed since it was shown: nothing was changed. Open it again."));
        }
        if steps.iter().find(|s| s.action != Action::Drop).is_some_and(|s| s.action == Action::Squash) {
            return Err("The first commit kept can't be squashed: there's nothing above it to squash into".into());
        }
        let pushed = own.iter().any(|c| c.pushed);
        if pushed && !rewrite {
            return Err("Some of these commits are already pushed: changing them would rewrite shared history".into());
        }
        let tip = refs::tip(&format!("refs/heads/{branch}"), repo).ok_or_else(|| format!("No local branch {branch}"))?;
        let fork = run(&["merge-base", base, &tip], repo)?.trim().to_string();

        let dir = git::temp_dir("edit");
        let dir_str = dir.to_string_lossy().into_owned();
        write(&["worktree", "add", "--quiet", "--detach", &dir_str, &fork], repo)?;
        let built = build(steps, &own, &dir);
        let _ = write(&["worktree", "remove", "--force", &dir_str], repo);
        let new = built?;
        if new == tip {
            return Ok(format!("{branch} is already like that: nothing changed"));
        }

        // Move the branch only if it's still where we read it. Checked out: `reset --keep` carries uncommitted
        // changes along, and refuses (changing nothing) if they'd be overwritten.
        if current_branch(repo).as_deref() == Some(branch) {
            if refs::tip("HEAD", repo).as_deref() != Some(tip.as_str()) {
                return Err(format!("{branch} moved meanwhile: nothing was changed"));
            }
            write(&["reset", "--keep", "--quiet", &new], repo)?;
        } else {
            write(&["update-ref", "-m", "monti-git: edit commits", &format!("refs/heads/{branch}"), &new, &tip], repo)?;
        }
        let count = read_opt(&["rev-list", "--count", &new, &format!("^{fork}")], repo).map_or(0, |n| n.trim().parse().unwrap_or(0));
        let done = format!("Rewrote {branch}: {count} commit{}", if count == 1 { "" } else { "s" });
        if !pushed {
            return Ok(done);
        }
        Ok(match push_branch(branch, true, repo) {
            Ok(Pushed::Done) => format!("{done}, force-pushed"),
            Ok(Pushed::Behind) => format!("{done}, but not pushed: the server has commits you haven't fetched. Sync, then push with force"),
            Err(e) => format!("{done}, but the push failed: {}", e.lines().next().unwrap_or_default()),
        })
    })
}

/// Replays the steps in `dir` (detached at the fork point); returns the new tip. Hooks don't run: these commits
/// already passed them once.
fn build(steps: &[Step], own: &[Own], dir: &Path) -> Result<String, String> {
    for step in steps.iter().filter(|s| s.action != Action::Drop) {
        let picked = match step.action {
            Action::Squash => write(&["cherry-pick", "--no-commit", &step.id], dir)
                .and_then(|_| write(&["commit", "-q", "--amend", "--no-edit", "--no-verify", "--allow-empty"], dir)),
            _ => write(&["cherry-pick", "--allow-empty", "--keep-redundant-commits", &step.id], dir),
        };
        if picked.is_err() {
            let subject = own.iter().find(|c| c.id == step.id).and_then(|c| c.message.lines().next()).unwrap_or_default();
            return Err(format!("“{subject}” conflicts in that order: nothing was changed. Try another order, or squash it instead."));
        }
        if let Some(message) = step.message.as_deref().map(str::trim).filter(|m| !m.is_empty()) {
            write_input(&["commit", "-q", "--amend", "--no-verify", "--allow-empty", "-F", "-"], dir, &format!("{message}\n"))?;
        }
    }
    Ok(run(&["rev-parse", "HEAD"], dir)?.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::{Action, Step};
    use crate::testutil::Repo;

    fn step(id: &str, action: Action, message: Option<&str>) -> Step {
        Step { id: id.into(), action, message: message.map(String::from) }
    }

    #[test]
    fn reorders_squashes_rewords_and_drops() {
        let server = Repo::new("edit-server");
        server.commit("a.txt", "1", "init");
        let repo = Repo::new("edit-local");
        repo.git(&["remote", "add", "origin", &server.path().display().to_string()]);
        repo.git(&["fetch", "-q", "origin"]);
        repo.git(&["checkout", "-q", "-b", "main", "origin/main"]);
        repo.git(&["checkout", "-q", "-b", "feat", "--no-track"]);
        repo.commit("b.txt", "1", "add b");
        repo.commit("c.txt", "1", "add c");
        repo.commit("b.txt", "2", "fix b");
        repo.commit("d.txt", "1", "add d");
        let own = super::own_commits("feat", "main", repo.path()).unwrap();
        assert_eq!(own.iter().map(|c| c.message.as_str()).collect::<Vec<_>>(), ["add b", "add c", "fix b", "add d"]);
        assert!(own.iter().all(|c| !c.pushed));
        let id = |n: usize| own[n].id.as_str();
        assert!(super::apply("feat", "main", &[step(id(0), Action::Pick, None)], false, repo.path()).is_err(), "missing commits: refused");
        let plan = [step(id(0), Action::Pick, Some("b, done right")), step(id(2), Action::Squash, None), step(id(1), Action::Pick, None), step(id(3), Action::Drop, None)];
        super::apply("feat", "main", &plan, false, repo.path()).unwrap();
        assert_eq!(repo.git(&["log", "--reverse", "--format=%s", "main..feat"]).trim(), "b, done right\nadd c");
        assert_eq!(std::fs::read_to_string(repo.path().join("b.txt")).unwrap(), "2", "the fix went into b");
        assert!(!repo.path().join("d.txt").exists(), "dropped, and the checkout followed");

        // Pushed: only with rewrite, then force-pushed.
        repo.git(&["push", "-q", "-u", "origin", "feat"]);
        let own = super::own_commits("feat", "main", repo.path()).unwrap();
        assert!(own.iter().all(|c| c.pushed));
        let squash = [step(&own[0].id, Action::Pick, None), step(&own[1].id, Action::Squash, None)];
        assert!(super::apply("feat", "main", &squash, false, repo.path()).unwrap_err().contains("already pushed"));
        let done = super::apply("feat", "main", &squash, true, repo.path()).unwrap();
        assert!(done.ends_with("force-pushed"), "{done}");
        assert_eq!(server.git(&["rev-list", "--count", "main..feat"]).trim(), "1");
    }
}
