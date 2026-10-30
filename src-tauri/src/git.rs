//! Same approach as the Swift app (and vscode-git-graph): spawn the git binary and parse its stdout.
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Serialize, Clone, Debug)]
pub struct Commit {
    pub id: String, // full hash, or WORKTREE_ID for the uncommitted-changes row
    pub parents: Vec<String>,
    pub author: String,
    pub date: i64, // unix seconds
    pub refs: Vec<String>, // raw %D decorations, e.g. "HEAD -> main", "origin/main", "tag: v1.0"
    pub subject: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shelved: Option<String>, // stash row: the branch whose WIP Monti Git put aside
}

pub const WORKTREE_ID: &str = "WORKTREE";

impl Commit {
    pub fn is_worktree(&self) -> bool {
        self.id == WORKTREE_ID
    }
}

pub fn run(args: &[&str], repo: &Path) -> Result<String, String> {
    run_input(args, repo, None)
}

pub fn run_input(args: &[&str], repo: &Path, input: Option<&str>) -> Result<String, String> {
    let out = spawn(args, repo, input)?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Files that would conflict merging `theirs` into `ours`, without touching the working tree or index
/// (exit 1 = conflicts, with their paths after the tree id).
pub fn conflicts(ours: &str, theirs: &str, repo: &Path) -> Vec<String> {
    let Ok(out) = spawn(&["-c", "core.quotePath=false", "merge-tree", "--write-tree", "--name-only", "--no-messages", ours, theirs], repo, None) else { return vec![] };
    if out.status.code() != Some(1) {
        return vec![];
    }
    let mut files: Vec<String> = String::from_utf8_lossy(&out.stdout).lines().skip(1).filter(|l| !l.is_empty()).map(String::from).collect();
    files.dedup();
    files
}

/// True when merging `theirs` into `ours` would change nothing: its content is already there, e.g. after a
/// squash merge (new commit, same changes). Nothing is written to the working tree or index.
pub fn already_contains(ours: &str, theirs: &str, repo: &Path) -> bool {
    let Ok(out) = spawn(&["merge-tree", "--write-tree", ours, theirs], repo, None) else { return false };
    let merged = String::from_utf8_lossy(&out.stdout).lines().next().unwrap_or_default().trim().to_string();
    out.status.success() && ok(&["rev-parse", &format!("{ours}^{{tree}}")], repo).is_some_and(|tree| tree.trim() == merged)
}

fn spawn(args: &[&str], repo: &Path, input: Option<&str>) -> Result<std::process::Output, String> {
    let mut command = Command::new("git");
    command
        .args(args)
        .current_dir(repo)
        // Fail instead of hanging forever if a remote asks for a username/password.
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        // CREATE_NO_WINDOW: without it every git call flashes a console window.
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("couldn't run git: {e}"))?;
    if let Some(input) = input {
        // Patches are small: written whole before reading output, no pipe deadlock.
        let _ = child.stdin.take().unwrap().write_all(input.as_bytes());
    }
    // wait_with_output drains stdout and stderr together, so a chatty stderr can't block git.
    child.wait_with_output().map_err(|e| e.to_string())
}

fn ok(args: &[&str], repo: &Path) -> Option<String> {
    run(args, repo).ok()
}

/// `refs`: only these branches (plus HEAD); None = every branch, remote and tag.
/// `refs` may hold any rev-list arguments (e.g. `--first-parent`, `^base`); `with_head` also lists HEAD's history.
/// `path`: only commits that touched it, parents rewritten to the previous such commit so the lines stay connected.
pub fn log(repo: &Path, refs: Option<&[String]>, with_head: bool, path: Option<&str>, limit: usize) -> Result<Vec<Commit>, String> {
    // \x1f between fields, \x1e between records: neither can appear in a subject line.
    let max = format!("--max-count={limit}");
    let mut args = vec!["-c", "log.showSignature=false", "log"];
    match refs {
        Some(refs) => args.extend(refs.iter().map(String::as_str)),
        None => args.extend(["--branches", "--remotes", "--tags"]),
    }
    // --topo-order: each branch's commits stay together instead of interleaving by date.
    if with_head {
        args.push("HEAD");
    }
    if path.is_some() {
        args.push("--parents");
    }
    args.extend(["--topo-order", &max, "--format=%H%x1f%P%x1f%an%x1f%at%x1f%D%x1f%s%x1e", "--"]);
    args.extend(path);
    let output = run(&args, repo)?;
    Ok(output
        .split('\u{1e}')
        .filter_map(|record| {
            let f: Vec<&str> = record.trim_matches('\n').split('\u{1f}').collect();
            if f.len() != 6 {
                return None;
            }
            Some(Commit {
                id: f[0].into(),
                parents: f[1].split_whitespace().map(String::from).collect(),
                author: f[2].into(),
                date: f[3].parse().unwrap_or(0),
                refs: if f[4].is_empty() { vec![] } else { f[4].split(", ").map(String::from).collect() },
                subject: f[5].into(),
                shelved: None,
            })
        })
        .collect())
}

/// The "Uncommitted changes" row, or None when the working tree is clean.
pub fn worktree_commit(repo: &Path) -> Option<Commit> {
    let changed = ok(&["status", "--porcelain"], repo).unwrap_or_default().lines().count();
    if changed == 0 {
        return None;
    }
    let head = ok(&["rev-parse", "HEAD"], repo).map(|s| s.trim().to_string());
    Some(Commit {
        id: WORKTREE_ID.into(),
        parents: head.into_iter().collect(),
        author: String::new(),
        date: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0),
        refs: vec![],
        subject: if changed == 1 { "1 changed file".into() } else { format!("{changed} changed files") },
        shelved: None,
    })
}

/// The changes Monti Git put aside when leaving a branch (a "monti-git: <branch>" stash, see actions.rs), as rows
/// forking off the commit they were saved on. Subject: "4 files · +120 −30".
pub fn shelved_commits(repo: &Path) -> Vec<Commit> {
    let list = ok(&["stash", "list", "--format=%H%x09%P%x09%ct%x09%gs"], repo).unwrap_or_default();
    list.lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.splitn(4, '\t').collect();
            let branch = f.get(3)?.split_once(": monti-git: ")?.1; // "On <branch>: monti-git: <branch>"
            let base = f[1].split(' ').next()?;
            // " 4 files changed, 120 insertions(+), 30 deletions(-)": untracked files included.
            let stat = ok(&["stash", "show", "--include-untracked", "--shortstat", f[0]], repo).unwrap_or_default();
            let count = |word: &str| stat.split(", ").find(|p| p.contains(word)).and_then(|p| p.split_whitespace().next()?.parse::<u32>().ok()).unwrap_or(0);
            let files = count("file");
            Some(Commit {
                id: f[0].into(),
                parents: vec![base.into()],
                author: String::new(),
                date: f[2].parse().unwrap_or(0),
                refs: vec![],
                subject: format!("{files} file{} · +{} −{}", if files == 1 { "" } else { "s" }, count("insertion"), count("deletion")),
                shelved: Some(branch.into()),
            })
        })
        .collect()
}

/// The ref behind `main_chain`: the one chosen for this repo, else main, else production (local first, then origin/).
pub fn main_ref(repo: &Path, preferred: Option<&str>) -> Option<String> {
    preferred
        .into_iter()
        .chain(["main", "production"])
        .flat_map(|name| [name.to_string(), format!("origin/{name}")])
        .find(|candidate| ok(&["rev-parse", "--verify", "--quiet", candidate], repo).is_some())
}

/// First-parent history of the branch pinned to the left. Empty if none exists.
pub fn main_chain(repo: &Path, main: Option<&str>) -> HashSet<String> {
    main.and_then(|r| ok(&["rev-list", "--first-parent", "--max-count=20000", r], repo))
        .map(|out| out.lines().map(String::from).collect())
        .unwrap_or_default()
}

/// Local and remote branches whose newest commit was authored by this repo's user.name or user.email.
pub fn my_branches(repo: &Path) -> Vec<String> {
    let config = |key| ok(&["config", key], repo).unwrap_or_default().trim().to_lowercase();
    let (name, email) = (config("user.name"), config("user.email"));
    let output = ok(&["for-each-ref", "--format=%(refname)%09%(authorname)%09%(authoremail:trim)", "refs/heads", "refs/remotes"], repo)
        .unwrap_or_default();
    output
        .lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() != 3 || f[0].to_lowercase().ends_with("/head") {
                return None; // origin/HEAD is just an alias
            }
            let mine = (!name.is_empty() && f[1].to_lowercase() == name) || (!email.is_empty() && f[2].to_lowercase() == email);
            mine.then(|| f[0].to_string())
        })
        .collect()
}

/// Files changed between `base` and `target`, path -> status letter (A, M, D, R…). `base` None = root commit.
pub fn changed_files(base: Option<&str>, target: &str, repo: &Path) -> HashMap<String, String> {
    let mut args = vec!["-c", "core.quotePath=false"];
    match base {
        Some(base) => args.extend(["diff", "--name-status", "-M", base, target]),
        None => args.extend(["diff-tree", "--root", "-r", "--no-commit-id", "--name-status", "-M", target]),
    }
    let mut result = HashMap::new();
    for line in ok(&args, repo).unwrap_or_default().lines() {
        let f: Vec<&str> = line.split('\t').collect(); // "M\tpath" or "R100\told\tnew"
        if f.len() > 1 {
            result.insert(f[f.len() - 1].to_string(), f[0][..1].to_string());
        }
    }
    result
}

/// Uncommitted files, path -> status letter; untracked files count as added.
pub fn worktree_files(repo: &Path) -> HashMap<String, String> {
    let mut result = HashMap::new();
    for line in ok(&["-c", "core.quotePath=false", "status", "--porcelain", "-uall"], repo).unwrap_or_default().lines() {
        if line.len() <= 3 {
            continue;
        }
        let xy: Vec<char> = line[..2].chars().collect();
        let path = line[3..].rsplit(" -> ").next().unwrap_or_default(); // renames: "old -> new"
        let status = if line.starts_with("??") { 'A' } else if xy[0] == ' ' { xy[1] } else { xy[0] };
        result.insert(path.to_string(), status.to_string());
    }
    result
}

/// Files with staged changes (new files included once `git add`ed).
pub fn staged_files(repo: &Path) -> Vec<String> {
    ok(&["-c", "core.quotePath=false", "diff", "--cached", "--name-only", "--no-renames"], repo).unwrap_or_default().lines().map(String::from).collect()
}

/// Lines added/removed per file (None base = root commit, None target = working tree vs HEAD). Binary files are left out.
pub fn line_counts(base: Option<&str>, target: Option<&str>, repo: &Path) -> HashMap<String, (u32, u32)> {
    let args: Vec<&str> = match (base, target) {
        (_, None) => vec!["diff", "--numstat", "-z", "-M", "HEAD"],
        (Some(base), Some(target)) => vec!["diff", "--numstat", "-z", "-M", base, target],
        (None, Some(target)) => vec!["diff-tree", "--root", "-r", "--no-commit-id", "--numstat", "-z", "-M", target],
    };
    let output = ok(&args, repo).unwrap_or_default();
    // -z records: "added\tremoved\tpath\0", renames: "added\tremoved\t\0old\0new\0".
    let mut tokens = output.split('\0');
    let mut result = HashMap::new();
    while let Some(record) = tokens.next() {
        let f: Vec<&str> = record.split('\t').collect();
        if f.len() != 3 {
            continue;
        }
        let mut path = f[2].to_string();
        if path.is_empty() {
            tokens.next();
            path = tokens.next().unwrap_or_default().to_string();
        }
        if let (Ok(a), Ok(r)) = (f[0].parse(), f[1].parse()) {
            result.insert(path, (a, r));
        }
    }
    result
}

/// Every tracked file at `rev`; None = working tree (tracked + untracked, minus ignored).
pub fn all_files(rev: Option<&str>, repo: &Path) -> Vec<String> {
    let mut args = vec!["-c", "core.quotePath=false"];
    match rev {
        Some(rev) => args.extend(["ls-tree", "-r", "--name-only", rev]),
        None => args.extend(["ls-files", "--cached", "--others", "--exclude-standard"]),
    }
    ok(&args, repo).unwrap_or_default().lines().map(String::from).collect()
}

/// Unified diff of one file (None base = root commit, None target = working tree vs HEAD).
/// An untracked file comes back as a diff that adds every line.
pub fn file_diff(path: &str, base: Option<&str>, target: Option<&str>, repo: &Path) -> String {
    let args: Vec<&str> = match (base, target) {
        (_, None) => vec!["diff", "HEAD", "--", path],
        (Some(base), Some(target)) => vec!["diff", base, target, "--", path],
        (None, Some(target)) => vec!["show", "--format=", "--patch", target, "--", path],
    };
    let diff = ok(&[&["-c", "log.showSignature=false"][..], &args].concat(), repo).unwrap_or_default();
    if diff.is_empty() && target.is_none() {
        if let Ok(text) = std::fs::read_to_string(repo.join(path)) {
            let body: Vec<String> = text.strip_suffix('\n').unwrap_or(&text).split('\n').map(|l| format!("+{l}")).collect();
            return format!("@@ -0,0 +1,{} @@ new file\n{}", body.len(), body.join("\n"));
        }
    }
    diff
}

/// Which of `hashes` (a linear run, newest first) changed `path`.
pub fn commits_touching(path: &str, hashes: &[String], repo: &Path) -> Vec<String> {
    let (Some(newest), Some(oldest)) = (hashes.first(), hashes.last()) else { return vec![] };
    let range = format!("{oldest}^..{newest}");
    ok(&["log", "--format=%H", &range, "--", path], repo).unwrap_or_default().lines().map(String::from).collect()
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
        repo.git(&["stash", "push", "--include-untracked", "-m", "monti-git: main"]);
        repo.git(&["stash", "push", "-m", "someone else's"]); // not ours: ignored (nothing to stash, so a no-op anyway)
        let rows = super::shelved_commits(repo.path());
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].shelved.as_deref(), Some("main"));
        assert_eq!(rows[0].parents, [head]);
        assert_eq!(rows[0].subject, "2 files · +2 −0");
    }

    #[test]
    fn conflicts_and_containment() {
        let repo = Repo::new("conflicts");
        repo.commit("a.txt", "base\n", "init");
        repo.git(&["checkout", "-q", "-b", "mine"]);
        repo.commit("a.txt", "mine\n", "mine");
        repo.git(&["checkout", "-q", "-b", "theirs", "main"]);
        repo.commit("a.txt", "theirs\n", "theirs");
        repo.commit("b.txt", "new\n", "other file");
        assert_eq!(super::conflicts("mine", "theirs", repo.path()), ["a.txt"]);
        assert!(super::conflicts("main", "theirs", repo.path()).is_empty()); // fast-forward: nothing to resolve
        assert!(super::already_contains("theirs", "main", repo.path()));
        assert!(!super::already_contains("main", "theirs", repo.path()));
    }

    #[test]
    fn log_of_a_file_keeps_its_line_connected() {
        let repo = Repo::new("file-log");
        repo.commit("a.txt", "1", "a 1");
        repo.commit("b.txt", "1", "b only");
        repo.commit("a.txt", "2", "a 2");
        let commits = super::log(repo.path(), None, true, Some("a.txt"), 50).unwrap();
        assert_eq!(commits.iter().map(|c| c.subject.as_str()).collect::<Vec<_>>(), ["a 2", "a 1"]);
        assert_eq!(commits[0].parents, [commits[1].id.clone()]); // rewritten past "b only"
    }
}
