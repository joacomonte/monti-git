//! A merge, rebase or cherry-pick stopped on conflicts in the checkout: what's going on, each conflicted file's
//! blocks, picking a side per block (or for the whole file), and finishing it (continue) or going back (abort).
//! The app gets here from "Update from main" and "Pull and push" on the checked-out branch, or from git in a terminal.
use crate::actions::exclusive;
use crate::git::{self, read_opt, write};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub enum Operation {
    Merge,
    Rebase,
    CherryPick,
    Revert,
}

impl Operation {
    fn command(self) -> &'static str {
        match self {
            Operation::Merge => "merge",
            Operation::Rebase => "rebase",
            Operation::CherryPick => "cherry-pick",
            Operation::Revert => "revert",
        }
    }

    /// Which side of a conflict block is the user's work. A rebase replays your commits onto the other branch, so
    /// git's "ours" (the first side) is that branch and yours come second; merge, cherry-pick and revert are the other way.
    fn mine_first(self) -> bool {
        self != Operation::Rebase
    }
}

/// The operation in progress and its files still conflicted (empty: all resolved, ready to continue).
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Conflict {
    operation: Operation,
    files: Vec<String>,
    incoming: String, // what's coming in: the branch merged or rebased onto, or the commit picked
}

/// One `git rev-parse` for every marker git leaves while an operation is stopped. `files`: the unmerged ones (status "U").
pub fn state(repo: &Path, files: Vec<String>) -> Option<Conflict> {
    let markers = ["MERGE_HEAD", "rebase-merge", "rebase-apply", "CHERRY_PICK_HEAD", "REVERT_HEAD"];
    let args: Vec<&str> = std::iter::once("rev-parse").chain(markers.iter().flat_map(|m| ["--git-path", *m])).collect();
    let out = read_opt(&args, repo)?;
    let exists = |line: &str| repo.join(line).exists(); // relative to the repo, or absolute (join keeps it)
    let found: Vec<bool> = out.lines().map(exists).collect();
    let operation = match found[..] {
        [true, ..] => Operation::Merge,
        [_, true, ..] | [_, _, true, ..] => Operation::Rebase,
        [_, _, _, true, _] => Operation::CherryPick,
        [_, _, _, _, true] => Operation::Revert,
        _ => return None,
    };
    let incoming_ref = match operation {
        Operation::Merge => "MERGE_HEAD".to_string(),
        Operation::Rebase => read_opt(&["rev-parse", "--git-path", "rebase-merge/onto"], repo)
            .and_then(|p| std::fs::read_to_string(repo.join(p.trim())).ok())
            .map_or("HEAD".into(), |s| s.trim().to_string()),
        Operation::CherryPick => "CHERRY_PICK_HEAD".into(),
        Operation::Revert => "REVERT_HEAD".into(),
    };
    let incoming = read_opt(&["name-rev", "--name-only", "--exclude=refs/tags/*", &incoming_ref], repo).map_or_else(String::new, |s| s.trim().to_string());
    Some(Conflict { operation, files, incoming })
}

fn current(repo: &Path) -> Result<Conflict, String> {
    let unmerged = git::status(repo)?.files.into_iter().filter(|(_, letter)| *letter == 'U').map(|(path, _)| path).collect();
    state(repo, unmerged).ok_or_else(|| "No merge or rebase is stopped here anymore".to_string())
}

/// A conflicted file in order: text both sides agree on, and the blocks where they don't (the first side as git
/// wrote it, `mine` or not).
#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Part {
    Same { text: String },
    Conflict { mine: String, theirs: String },
}

/// Splits a file on git's markers (a diff3 "|||||||" base section is skipped). No markers: one Same part.
fn parse(text: &str, mine_first: bool) -> Vec<Part> {
    enum At { Out, First, Base, Second }
    let (mut parts, mut same, mut first, mut second, mut at) = (vec![], String::new(), String::new(), String::new(), At::Out);
    for line in text.split_inclusive('\n') {
        let marker = |m: &str| line.starts_with(m) && line[7..].trim_end_matches(['\n', '\r']).chars().next().is_none_or(|c| c == ' ');
        match at {
            At::Out if marker("<<<<<<<") => {
                if !same.is_empty() {
                    parts.push(Part::Same { text: std::mem::take(&mut same) });
                }
                at = At::First;
            }
            At::Out => same.push_str(line),
            At::First | At::Base if marker("=======") => at = At::Second,
            At::First if marker("|||||||") => at = At::Base,
            At::First => first.push_str(line),
            At::Base => {}
            At::Second if marker(">>>>>>>") => {
                let (a, b) = (std::mem::take(&mut first), std::mem::take(&mut second));
                parts.push(if mine_first { Part::Conflict { mine: a, theirs: b } } else { Part::Conflict { mine: b, theirs: a } });
                at = At::Out;
            }
            At::Second => second.push_str(line),
        }
    }
    if !same.is_empty() {
        parts.push(Part::Same { text: same });
    }
    parts
}

/// A conflicted file's parts, for the resolver.
pub fn read(path: &str, repo: &Path) -> Result<Vec<Part>, String> {
    let conflict = current(repo)?;
    let text = std::fs::read_to_string(git::repo_file(repo, path)?).map_err(|e| format!("Can't read {path} as text: {e}"))?;
    Ok(parse(&text, conflict.operation.mine_first()))
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum Pick {
    Mine,
    Theirs,
    Both, // mine, then theirs
}

/// Writes `path` with one pick per conflict block, then marks it resolved. Refused if its blocks changed since it was
/// read (`picks` no longer match them): nothing is written.
pub fn resolve(path: &str, picks: &[Pick], repo: &Path) -> Result<(), String> {
    exclusive(repo, || {
        let parts = read(path, repo)?;
        let blocks = parts.iter().filter(|p| matches!(p, Part::Conflict { .. })).count();
        if blocks != picks.len() {
            return Err(format!("{path} changed since it was shown: nothing was written. Look at it again."));
        }
        let mut picks = picks.iter();
        let text: String = parts
            .into_iter()
            .map(|part| match part {
                Part::Same { text } => text,
                Part::Conflict { mine, theirs } => match picks.next() {
                    Some(Pick::Mine) => mine,
                    Some(Pick::Theirs) => theirs,
                    _ => mine + &theirs,
                },
            })
            .collect();
        std::fs::write(git::repo_file(repo, path)?, text).map_err(|e| e.to_string())?;
        write(&["add", "--", path], repo).map(|_| ())
    })
}

/// The whole file as one side has it; a side that deleted it deletes it.
pub fn take(path: &str, mine: bool, repo: &Path) -> Result<(), String> {
    exclusive(repo, || {
        let conflict = current(repo)?;
        let side = if mine == conflict.operation.mine_first() { "--ours" } else { "--theirs" };
        if write(&["checkout", side, "--", path], repo).is_ok() {
            write(&["add", "--", path], repo).map(|_| ())
        } else {
            write(&["rm", "-q", "--", path], repo).map(|_| ()) // that side doesn't have it
        }
    })
}

/// Finishes the operation once nothing is conflicted. A rebase may stop again on its next commit: that's a status,
/// and the resolver shows the new conflicts.
pub fn continue_operation(repo: &Path) -> Result<String, String> {
    exclusive(repo, || {
        let conflict = current(repo)?;
        if !conflict.files.is_empty() {
            return Err(format!("{} still conflicted: resolve every file first", conflict.files.join(", ")));
        }
        // core.editor=true: keep git's prepared message instead of opening an editor.
        let args: &[&str] = match conflict.operation {
            Operation::Merge => &["-c", "core.editor=true", "commit", "--no-edit", "--no-verify"],
            op => &["-c", "core.editor=true", op.command(), "--continue"],
        };
        match write(args, repo) {
            Ok(_) => Ok(format!("Finished the {}", conflict.operation.command())),
            Err(_) if current(repo).is_ok_and(|c| !c.files.is_empty()) => Ok("The next commit has conflicts too: resolve them and continue".into()),
            Err(error) => Err(error.trim().to_string()),
        }
    })
}

/// Back to where the branch was before the operation started.
pub fn abort(repo: &Path) -> Result<String, String> {
    exclusive(repo, || {
        let conflict = current(repo)?;
        write(&[conflict.operation.command(), "--abort"], repo)?;
        Ok(format!("Aborted the {}: back to where it was", conflict.operation.command()))
    })
}

#[cfg(test)]
mod tests {
    use super::{Part, Pick};
    use crate::testutil::Repo;

    #[test]
    fn parses_blocks_with_the_users_side_first() {
        let text = "a\n<<<<<<< HEAD\nmine\n||||||| base\nold\n=======\ntheirs\n>>>>>>> feat\nb\n";
        let conflict = Part::Conflict { mine: "mine\n".into(), theirs: "theirs\n".into() };
        assert_eq!(super::parse(text, true), [Part::Same { text: "a\n".into() }, conflict, Part::Same { text: "b\n".into() }]);
        let rebase = Part::Conflict { mine: "theirs\n".into(), theirs: "mine\n".into() };
        assert_eq!(super::parse(text, false)[1], rebase, "a rebase's own commit is the second side");
        assert_eq!(super::parse("<<<<<<<< not a marker\n", true).len(), 1);
    }

    #[test]
    fn update_stops_on_conflicts_then_resolves_and_continues() {
        let repo = Repo::new("resolve");
        repo.commit("a.txt", "1\n", "init");
        repo.git(&["checkout", "-q", "-b", "feat"]);
        repo.commit("a.txt", "feat\n", "feat change");
        repo.git(&["checkout", "-q", "main"]);
        repo.commit("a.txt", "main\n", "main change");
        repo.git(&["checkout", "-q", "feat"]);
        assert_eq!(crate::actions::update("feat", "main", false, repo.path()).unwrap(), crate::actions::Integrated::Conflicts);
        let state = super::current(repo.path()).unwrap();
        assert_eq!((state.operation, state.files.as_slice(), state.incoming.as_str()), (super::Operation::Merge, ["a.txt".to_string()].as_slice(), "main"));
        assert!(super::continue_operation(repo.path()).is_err(), "still conflicted");
        assert!(super::resolve("a.txt", &[], repo.path()).is_err(), "wrong number of picks: untouched");
        super::resolve("a.txt", &[Pick::Both], repo.path()).unwrap();
        assert_eq!(std::fs::read_to_string(repo.path().join("a.txt")).unwrap(), "feat\nmain\n");
        super::continue_operation(repo.path()).unwrap();
        assert!(super::current(repo.path()).is_err(), "merge finished");
        assert_eq!(repo.git(&["rev-list", "--count", "--merges", "HEAD"]).trim(), "1");

        // A rebase onto main: the user's commit is "mine" though git writes it second; abort goes back.
        repo.git(&["reset", "-q", "--hard", "HEAD~1"]);
        assert_eq!(crate::actions::update("feat", "main", true, repo.path()).unwrap(), crate::actions::Integrated::Conflicts);
        assert_eq!(super::read("a.txt", repo.path()).unwrap(), [Part::Conflict { mine: "feat\n".into(), theirs: "main\n".into() }]);
        super::abort(repo.path()).unwrap();
        assert_eq!(std::fs::read_to_string(repo.path().join("a.txt")).unwrap(), "feat\n");
        assert_eq!(crate::actions::update("feat", "main", true, repo.path()).unwrap(), crate::actions::Integrated::Conflicts);
        super::take("a.txt", true, repo.path()).unwrap();
        super::continue_operation(repo.path()).unwrap();
        assert_eq!(repo.git(&["log", "-1", "--format=%s"]).trim(), "feat change");
        assert_eq!(std::fs::read_to_string(repo.path().join("a.txt")).unwrap(), "feat\n");
    }
}
