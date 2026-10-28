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
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn ok(args: &[&str], repo: &Path) -> Option<String> {
    run(args, repo).ok()
}

/// `refs`: only these branches (plus HEAD); None = every branch, remote and tag.
pub fn log(repo: &Path, refs: Option<&[String]>, limit: usize) -> Result<Vec<Commit>, String> {
    // \x1f between fields, \x1e between records: neither can appear in a subject line.
    let max = format!("--max-count={limit}");
    let mut args = vec!["-c", "log.showSignature=false", "log"];
    match refs {
        Some(refs) => args.extend(refs.iter().map(String::as_str)),
        None => args.extend(["--branches", "--remotes", "--tags"]),
    }
    // --topo-order: each branch's commits stay together instead of interleaving by date.
    args.extend(["HEAD", "--topo-order", &max, "--format=%H%x1f%P%x1f%an%x1f%at%x1f%D%x1f%s%x1e", "--"]);
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
    })
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
