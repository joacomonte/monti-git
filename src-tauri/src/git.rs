//! The git binary, spawned and parsed (same approach as the Swift app and vscode-git-graph). Every call goes through
//! `run` (reads) or `write` (changes the repo), with one fixed environment, and is recorded for the Logs panel.
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

// MARK: Running git

/// Settings every call gets, whatever the user's git config says: raw UTF-8 paths, no colors or signatures in output,
/// `a/` `b/` diff prefixes, and a stalled HTTPS transfer gives up after 30 s instead of hanging (ssh keeps its own timeouts).
const CONFIG: [(&str, &str); 7] = [
    ("core.quotePath", "false"),
    ("log.showSignature", "false"),
    ("color.ui", "false"),
    ("diff.noprefix", "false"),
    ("diff.mnemonicPrefix", "false"),
    ("http.lowSpeedLimit", "1000"),
    ("http.lowSpeedTime", "30"),
];

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Read,
    Write,
}

/// A read: log, status, diff… Fails with git's stderr.
pub fn run(args: &[&str], repo: &Path) -> Result<String, String> {
    output(args, repo, None, Kind::Read)
}

/// A change to the repo (checkout, commit, fetch…): shown apart in the Logs panel.
pub fn write(args: &[&str], repo: &Path) -> Result<String, String> {
    output(args, repo, None, Kind::Write)
}

/// A change that reads `input` on stdin (a commit message, a patch).
pub fn write_input(args: &[&str], repo: &Path, input: &str) -> Result<String, String> {
    output(args, repo, Some(input), Kind::Write)
}

/// A read whose failure means "nothing": a missing ref, no upstream, no match.
pub fn read_opt(args: &[&str], repo: &Path) -> Option<String> {
    run(args, repo).ok()
}

fn output(args: &[&str], repo: &Path, input: Option<&str>, kind: Kind) -> Result<String, String> {
    let out = spawn(args, repo, input, kind)?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim_end().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn spawn(args: &[&str], repo: &Path, input: Option<&str>, kind: Kind) -> Result<Output, String> {
    let started = Instant::now();
    let mut command = Command::new("git");
    command
        .args(args)
        .current_dir(repo)
        .env("GIT_TERMINAL_PROMPT", "0") // fail instead of waiting for a username/password nobody can type
        .env("GIT_OPTIONAL_LOCKS", "0") // background reads never take index.lock away from the user's own git
        .env("LC_ALL", "C") // parsed output (ahead/behind, errors) in English whatever the system language
        .env("GIT_CONFIG_COUNT", CONFIG.len().to_string())
        .stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (i, (key, value)) in CONFIG.iter().enumerate() {
        command.env(format!("GIT_CONFIG_KEY_{i}"), key).env(format!("GIT_CONFIG_VALUE_{i}"), value);
    }
    #[cfg(windows)]
    {
        // CREATE_NO_WINDOW: without it every git call flashes a console window.
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let mut child = command.spawn().map_err(|e| format!("couldn't run git: {e}"))?;
    if let Some(input) = input {
        // Messages and patches are small: written whole before reading output, no pipe deadlock.
        child.stdin.take().expect("piped").write_all(input.as_bytes()).map_err(|e| format!("couldn't write to git: {e}"))?;
    }
    // wait_with_output drains stdout and stderr together, so a chatty stderr can't block git.
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    record(args, repo, started, &out, kind);
    Ok(out)
}

/// A path the UI sent, inside `repo`: relative and without `..`, so it can't reach files elsewhere.
pub fn repo_file(repo: &Path, path: &str) -> Result<PathBuf, String> {
    let relative = Path::new(path);
    if path.is_empty() || !relative.components().all(|c| matches!(c, Component::Normal(_) | Component::CurDir)) {
        return Err(format!("Not a file of this repo: {path}"));
    }
    Ok(repo.join(relative))
}

/// A fresh path for a temporary worktree (`git worktree add` creates it), unique per call.
pub fn temp_dir(purpose: &str) -> PathBuf {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    std::env::temp_dir().join(format!("monti-git-{purpose}-{}-{nanos}", std::process::id()))
}

// MARK: Logs panel

/// One git command the app ran, for the Logs panel: what, where, how it went and everything it printed.
#[derive(Serialize, Clone)]
pub struct LogEntry {
    id: u64,
    at: u64, // unix ms
    repo: String,
    args: Vec<String>,
    ok: bool,
    code: Option<i32>,
    ms: u64,
    stdout: String,
    stderr: String,
    write: bool, // changes the repo (checkout, pull, commit…), vs a read (log, rev-list, status…)
    truncated: bool,
}

/// Session-only, newest last, kept within a byte budget. Reads and changes apart, so the hundreds of reads per
/// refresh never push changes out.
#[derive(Default)]
struct Bucket {
    entries: VecDeque<LogEntry>,
    bytes: usize,
}

static LOG: LazyLock<Mutex<[Bucket; 2]>> = LazyLock::new(Mutex::default);
static NEXT_LOG: AtomicU64 = AtomicU64::new(1);
const LOG_BUDGET: usize = 8 * 1024 * 1024; // per bucket

fn record(args: &[&str], repo: &Path, started: Instant, out: &Output, kind: Kind) {
    let cap = if kind == Kind::Write { 256 * 1024 } else { 8 * 1024 }; // reads (whole logs, diffs) can be megabytes
    let mut truncated = false;
    let mut text = |bytes: &[u8]| {
        let s = String::from_utf8_lossy(bytes);
        if s.len() <= cap {
            return s.into_owned();
        }
        truncated = true;
        let mut end = cap;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        s[..end].to_string()
    };
    let (stdout, stderr) = (text(&out.stdout), text(&out.stderr));
    let entry = LogEntry {
        id: NEXT_LOG.fetch_add(1, Ordering::Relaxed),
        at: SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64),
        repo: repo.to_string_lossy().into_owned(),
        args: args.iter().map(|a| a.to_string()).collect(),
        ok: out.status.success(),
        code: out.status.code(),
        ms: started.elapsed().as_millis() as u64,
        write: kind == Kind::Write,
        truncated,
        stdout,
        stderr,
    };
    let size = entry.stdout.len() + entry.stderr.len() + 256;
    let mut log = LOG.lock().unwrap();
    let bucket = &mut log[kind as usize];
    bucket.entries.push_back(entry);
    bucket.bytes += size;
    while bucket.bytes > LOG_BUDGET || bucket.entries.len() > 500 {
        let Some(old) = bucket.entries.pop_front() else { break };
        bucket.bytes -= old.stdout.len() + old.stderr.len() + 256;
    }
}

/// Commands logged after `since` (an id), oldest first.
pub fn logs(since: u64) -> Vec<LogEntry> {
    let log = LOG.lock().unwrap();
    let mut out: Vec<LogEntry> = log.iter().flat_map(|b| b.entries.iter()).filter(|e| e.id > since).cloned().collect();
    out.sort_by_key(|e| e.id);
    out
}

// MARK: Commits

#[derive(Serialize, Clone, Debug)]
pub struct Commit {
    pub id: String, // full hash, or WORKTREE_ID for the uncommitted-changes row
    pub parents: Vec<String>,
    pub author: String,
    pub date: i64,         // unix seconds
    pub refs: Vec<String>, // raw %D decorations, e.g. "HEAD -> main", "origin/main", "tag: v1.0"
    pub subject: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stash: Option<crate::stash::StashRow>, // a stash's row (its subject: the stash message)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stat: Option<Stat>, // uncommitted and stash rows: what they hold
}

/// Size of a set of changes: files, lines added and removed.
#[derive(Serialize, Clone, Copy, Debug, Default, PartialEq)]
pub struct Stat {
    pub files: u32, // the uncommitted row counts only files
    pub added: u32,
    pub removed: u32,
}

pub const WORKTREE_ID: &str = "WORKTREE";

impl Commit {
    pub fn is_worktree(&self) -> bool {
        self.id == WORKTREE_ID
    }
}

/// What one `git log` lists. `include`/`exclude`: revisions (`exclude` as `^rev`); empty `include` = every branch,
/// remote and tag. `decorate`: only these refs get names (None = all). `path`: only commits that touched it, parents
/// rewritten to the previous such commit so the lines stay connected; `simplify` does the same for decorated commits.
#[derive(Default)]
pub struct LogQuery<'a> {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub decorate: Option<Vec<String>>,
    pub first_parent: bool,
    pub simplify: bool,
    pub with_head: bool,
    pub path: Option<&'a str>,
    pub limit: usize,
}

/// \x1f between fields, \x1e between records: neither can appear in a subject line.
const LOG_FORMAT: &str = "--format=%H%x1f%P%x1f%an%x1f%at%x1f%D%x1f%s%x1e";

pub fn log(repo: &Path, q: &LogQuery) -> Result<Vec<Commit>, String> {
    let mut args: Vec<String> = vec!["log".into()];
    if let Some(decorate) = &q.decorate {
        args.extend(decorate.iter().map(|r| format!("--decorate-refs={r}")));
    }
    if q.first_parent {
        args.push("--first-parent".into());
    }
    args.push("--topo-order".into()); // each branch's commits stay together instead of interleaving by date
    if q.simplify {
        args.push("--simplify-by-decoration".into());
    }
    if q.simplify || q.path.is_some() {
        args.push("--parents".into());
    }
    args.extend([format!("--max-count={}", q.limit), LOG_FORMAT.into()]);
    if q.include.is_empty() {
        args.extend(["--branches", "--remotes", "--tags"].map(String::from));
    }
    args.extend(q.include.iter().cloned());
    args.extend(q.exclude.iter().map(|r| format!("^{r}")));
    if q.with_head {
        args.push("HEAD".into());
    }
    args.push("--".into());
    args.extend(q.path.map(String::from));
    let out = run(&args.iter().map(String::as_str).collect::<Vec<_>>(), repo)?;
    Ok(out.split('\u{1e}').filter_map(parse_commit).collect())
}

/// These commits only, none of their ancestors.
pub fn commits(ids: &[String], repo: &Path) -> Result<Vec<Commit>, String> {
    let mut args = vec!["show", "-s", LOG_FORMAT, "--end-of-options"];
    args.extend(ids.iter().map(String::as_str));
    Ok(run(&args, repo)?.split('\u{1e}').filter_map(parse_commit).collect())
}

fn parse_commit(record: &str) -> Option<Commit> {
    let mut f = record.trim_matches('\n').split('\u{1f}');
    let (id, parents, author, date, refs, subject) = (f.next()?, f.next()?, f.next()?, f.next()?, f.next()?, f.next()?);
    (!id.is_empty()).then(|| Commit {
        id: id.into(),
        parents: parents.split_whitespace().map(String::from).collect(),
        author: author.into(),
        date: date.parse().unwrap_or_default(),
        refs: decorations(refs),
        subject: subject.into(),
        stash: None,
        stat: None,
    })
}

/// "%D" ("HEAD -> main, origin/main, tag: v1") -> one entry per ref. ", " never appears inside a ref name.
pub fn decorations(text: &str) -> Vec<String> {
    text.split(", ").map(str::trim).filter(|r| !r.is_empty()).map(String::from).collect()
}

// MARK: Working tree

/// One `git status`: the checked-out branch (None when detached), HEAD's commit, and every changed file, untracked
/// ones listed one by one (so the uncommitted row's count matches the file tree).
pub struct Status {
    pub branch: Option<String>,
    pub head: Option<String>,
    pub files: BTreeMap<String, char>, // path -> status letter (A, M, D, R, U…); untracked = A
    pub staged: Vec<String>,           // files with changes in the index
    pub untracked: Vec<String>,        // new files git doesn't know yet
}

pub fn status(repo: &Path) -> Result<Status, String> {
    let out = run(&["status", "--porcelain=v2", "-z", "--branch", "--untracked-files=all"], repo)?;
    let mut s = Status { branch: None, head: None, files: BTreeMap::new(), staged: vec![], untracked: vec![] };
    let mut records = out.split('\0');
    while let Some(record) = records.next() {
        let header = |key: &str| record.strip_prefix(key).filter(|v| !v.starts_with('(')).map(String::from);
        if record.starts_with("# ") {
            s.branch = s.branch.take().or_else(|| header("# branch.head "));
            s.head = s.head.take().or_else(|| header("# branch.oid "));
            continue;
        }
        // "1 XY …8 fields… path", "2 XY …9 fields… path\0orig", "u XY …10 fields… path", "? path"
        let (letter, staged, path) = match record.as_bytes().first() {
            // A repo inside this one is listed as its folder, "inner/": shown as one entry, "inner".
            Some(b'?') => ('A', false, record[2..].trim_end_matches('/')),
            Some(kind @ (b'1' | b'2' | b'u')) => {
                let fields = match kind { b'1' => 9, b'2' => 10, _ => 11 };
                let mut parts = record.splitn(fields, ' ');
                let xy = parts.nth(1).unwrap_or("..").as_bytes();
                let Some(path) = parts.last() else { continue };
                if *kind == b'2' {
                    records.next(); // the rename's old path
                }
                let (x, y) = (xy[0] as char, xy.get(1).copied().unwrap_or(b'.') as char);
                let letter = if *kind == b'u' { 'U' } else if x != '.' { x } else { y };
                (letter, *kind != b'u' && x != '.', path)
            }
            _ => continue,
        };
        if staged {
            s.staged.push(path.to_string());
        }
        if record.starts_with('?') {
            s.untracked.push(path.to_string());
        }
        s.files.insert(path.to_string(), letter);
    }
    Ok(s)
}

/// The "Uncommitted changes" row on top of `head`, or None when the working tree is clean.
pub fn worktree_commit(head: Option<String>, stat: Stat) -> Option<Commit> {
    (stat.files > 0).then(|| Commit {
        id: WORKTREE_ID.into(),
        parents: head.into_iter().collect(),
        author: String::new(),
        date: SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0),
        refs: vec![],
        subject: String::new(),
        stash: None,
        stat: Some(stat),
    })
}

/// A working-tree file read as text (new files aren't in git yet): None if it's too big or not text.
pub fn file_text(repo: &Path, path: &str, max_bytes: u64) -> Option<String> {
    let full = repo_file(repo, path).ok()?;
    if std::fs::metadata(&full).ok()?.len() > max_bytes {
        return None;
    }
    String::from_utf8(std::fs::read(full).ok()?).ok()
}

/// Lines of a text: "a\nb" and "a\nb\n" both have 2.
pub fn line_count(text: &str) -> u32 {
    text.lines().count() as u32
}

// MARK: Diffs

/// What a selection shows: the uncommitted changes (against HEAD), or what a commit changed against its real first
/// parent, as git has it, never the graph's (a simplified graph links a commit to an older one). For a merge that's
/// what it brought into the branch it was made on; a root commit, against nothing.
#[derive(Clone, Copy)]
pub enum Span<'a> {
    Worktree,
    Commit(&'a str),
}

impl<'a> Span<'a> {
    /// None = the uncommitted-changes row.
    pub fn new(commit: Option<&'a str>) -> Span<'a> {
        commit.map_or(Span::Worktree, Span::Commit)
    }

    /// The commit whose files these are; None = the working tree.
    pub fn target(&self) -> Option<&'a str> {
        match self {
            Span::Worktree => None,
            Span::Commit(id) => Some(id),
        }
    }

    /// `git diff` (diff-tree for a commit) with `flags`, limited to `paths`.
    fn diff(&self, flags: &[&str], paths: &[&str], repo: &Path) -> Result<String, String> {
        let mut args: Vec<&str> = match self {
            Span::Worktree => vec!["diff", "--no-ext-diff", "HEAD"],
            Span::Commit(id) => vec!["diff-tree", "--no-ext-diff", "-r", "--root", "--diff-merges=first-parent", "--no-commit-id", id],
        };
        args.splice(1..1, flags.iter().copied());
        args.push("--");
        args.extend(paths);
        run(&args, repo)
    }
}

/// Files changed in a commit span, path -> status letter (A, M, D, R…).
pub fn changed_files(span: Span, repo: &Path) -> Result<BTreeMap<String, char>, String> {
    let out = span.diff(&["--name-status", "-z", "-M"], &[], repo)?;
    // -z: "M\0path\0", renames "R100\0old\0new\0".
    let mut tokens = out.split('\0');
    let mut result = BTreeMap::new();
    while let Some(status) = tokens.next() {
        let Some(letter) = status.chars().next() else { continue };
        let mut path = tokens.next().unwrap_or_default();
        if matches!(letter, 'R' | 'C') {
            path = tokens.next().unwrap_or_default();
        }
        result.insert(path.to_string(), letter);
    }
    Ok(result)
}

/// Lines added/removed per file. Binary files are left out.
pub fn line_counts(span: Span, repo: &Path) -> HashMap<String, (u32, u32)> {
    let out = span.diff(&["--numstat", "-z", "-M"], &[], repo).unwrap_or_default();
    // -z records: "added\tremoved\tpath\0", renames: "added\tremoved\t\0old\0new\0".
    let mut tokens = out.split('\0');
    let mut result = HashMap::new();
    while let Some(record) = tokens.next() {
        let mut f = record.splitn(3, '\t');
        let (Some(a), Some(r), Some(path)) = (f.next(), f.next(), f.next()) else { continue };
        let path = if path.is_empty() {
            tokens.next();
            tokens.next().unwrap_or_default()
        } else {
            path
        };
        if let (Ok(a), Ok(r)) = (a.parse(), r.parse()) {
            result.insert(path.to_string(), (a, r));
        }
    }
    result
}

/// Unified diff of `paths` (all files when empty) with `context` lines around each change.
pub fn patch(span: Span, paths: &[&str], context: u32, repo: &Path) -> Result<String, String> {
    let unified = format!("-U{context}");
    span.diff(&["--patch", &unified, "--src-prefix=a/", "--dst-prefix=b/"], paths, repo)
}

/// Every tracked file at `rev`; None = working tree (tracked + untracked, minus ignored).
pub fn all_files(rev: Option<&str>, repo: &Path) -> Vec<String> {
    let args: Vec<&str> = match rev {
        Some(rev) => vec!["ls-tree", "-r", "-z", "--name-only", rev],
        None => vec!["ls-files", "-z", "--cached", "--others", "--exclude-standard"],
    };
    read_opt(&args, repo).unwrap_or_default().split('\0').filter(|p| !p.is_empty()).map(String::from).collect()
}

/// True when merging `theirs` into `ours` would change nothing: its content is already there, e.g. after a
/// squash merge (new commit, same changes). Needs git 2.38+ (`merge-tree --write-tree`): older git says no.
pub fn already_contains(ours: &str, theirs: &str, repo: &Path) -> bool {
    let Ok(merged) = run(&["merge-tree", "--write-tree", ours, theirs], repo) else { return false };
    let merged = merged.lines().next().unwrap_or_default().trim().to_string();
    read_opt(&["rev-parse", &format!("{ours}^{{tree}}")], repo).is_some_and(|tree| tree.trim() == merged)
}

// MARK: The base's main line

/// First-parent history of the branch pinned to the left (`tip`: its commit), newest first, up to 20k commits.
/// Cached per repo and tip: no walking it on every load.
pub fn main_line(repo: &Path, tip: &str) -> Arc<MainLine> {
    static CACHE: LazyLock<Mutex<HashMap<(String, String), Arc<MainLine>>>> = LazyLock::new(Mutex::default);
    let key = (repo.to_string_lossy().into_owned(), tip.to_string());
    if let Some(line) = CACHE.lock().unwrap().get(&key) {
        return line.clone();
    }
    let order: Vec<String> = read_opt(&["rev-list", "--first-parent", "--max-count=20000", tip], repo).unwrap_or_default().lines().map(String::from).collect();
    let line = Arc::new(MainLine { set: order.iter().cloned().collect(), order });
    let mut cache = CACHE.lock().unwrap();
    if cache.len() >= 16 {
        cache.clear(); // tips move on: old lines are rarely asked again
    }
    cache.insert(key, line.clone());
    line
}

#[derive(Default)]
pub struct MainLine {
    pub order: Vec<String>, // newest first
    pub set: HashSet<String>,
}

#[cfg(test)]
mod tests {
    use crate::testutil::Repo;

    #[test]
    fn status_lists_every_change_once() {
        let repo = Repo::new("status");
        repo.commit("a.txt", "1", "init");
        let tip = repo.git(&["rev-parse", "HEAD"]).trim().to_string();
        let s = super::status(repo.path()).unwrap();
        assert_eq!((s.branch.as_deref(), s.head.as_deref(), s.files.len()), (Some("main"), Some(tip.as_str()), 0));
        std::fs::write(repo.path().join("a.txt"), "2").unwrap();
        std::fs::create_dir(repo.path().join("dir")).unwrap();
        std::fs::write(repo.path().join("dir/new one.txt"), "x").unwrap();
        std::fs::write(repo.path().join("dir/two.txt"), "x").unwrap();
        repo.git(&["add", "dir/two.txt"]);
        let s = super::status(repo.path()).unwrap();
        assert_eq!(s.files.get("a.txt"), Some(&'M'));
        assert_eq!(s.files.get("dir/new one.txt"), Some(&'A')); // untracked, one by one
        assert_eq!(s.staged, ["dir/two.txt"]);
        repo.git(&["checkout", "-q", "--detach"]);
        assert_eq!(super::status(repo.path()).unwrap().branch, None);
    }

    #[test]
    fn containment() {
        let repo = Repo::new("conflicts");
        repo.commit("a.txt", "base\n", "init");
        repo.git(&["checkout", "-q", "-b", "mine"]);
        repo.commit("a.txt", "mine\n", "mine");
        repo.git(&["checkout", "-q", "-b", "theirs", "main"]);
        repo.commit("a.txt", "theirs\n", "theirs");
        repo.commit("b.txt", "new\n", "other file");
        assert!(super::already_contains("theirs", "main", repo.path()));
        assert!(!super::already_contains("main", "theirs", repo.path()));
    }

    #[test]
    fn log_of_a_file_keeps_its_line_connected() {
        let repo = Repo::new("file-log");
        repo.commit("a.txt", "1", "a 1");
        repo.commit("b.txt", "1", "b only");
        repo.commit("a.txt", "2", "a 2");
        let q = super::LogQuery { with_head: true, path: Some("a.txt"), limit: 50, ..Default::default() };
        let commits = super::log(repo.path(), &q).unwrap();
        assert_eq!(commits.iter().map(|c| c.subject.as_str()).collect::<Vec<_>>(), ["a 2", "a 1"]);
        assert_eq!(commits[0].parents, [commits[1].id.clone()]); // rewritten past "b only"
    }

    #[test]
    fn a_merge_shows_what_it_brought_into_its_branch() {
        let repo = Repo::new("merge-span");
        repo.commit("a.txt", "1\n", "init");
        repo.git(&["checkout", "-q", "-b", "feat"]);
        repo.commit("feature.txt", "1\n", "feature");
        repo.git(&["checkout", "-q", "main"]);
        repo.commit("main.txt", "1\n", "main only");
        repo.git(&["merge", "-q", "--no-ff", "--no-edit", "feat"]);
        let merge = repo.git(&["rev-parse", "HEAD"]).trim().to_string();
        let files = super::changed_files(super::Span::Commit(&merge), repo.path()).unwrap();
        assert_eq!(files.keys().collect::<Vec<_>>(), ["feature.txt"]);
        let root = repo.git(&["rev-list", "--max-parents=0", "HEAD"]).trim().to_string();
        assert_eq!(super::changed_files(super::Span::Commit(&root), repo.path()).unwrap().keys().collect::<Vec<_>>(), ["a.txt"]);
    }

    #[test]
    fn changed_files_and_counts_handle_odd_names() {
        let repo = Repo::new("odd-names");
        repo.commit("a.txt", "1\n", "init");
        repo.commit("we:ird \"name\".txt", "1\n2\n", "odd");
        let tip = repo.git(&["rev-parse", "HEAD"]).trim().to_string();
        let span = super::Span::Commit(&tip);
        let files = super::changed_files(span, repo.path()).unwrap();
        assert_eq!(files.get("we:ird \"name\".txt"), Some(&'A'));
        assert_eq!(super::line_counts(span, repo.path()).get("we:ird \"name\".txt"), Some(&(2, 0)));
        assert!(super::repo_file(repo.path(), "../x").is_err() && super::repo_file(repo.path(), "/etc/hosts").is_err());
    }
}
