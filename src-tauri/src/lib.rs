mod actions;
mod branches;
mod diff;
mod git;
mod graph;
mod library;
mod search;
#[cfg(test)]
mod testutil;

use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

// `async` in the attribute runs these on a worker thread, so git never blocks the UI.

#[tauri::command(async)]
fn list_repos(folders: Vec<String>, repos: Vec<String>) -> Vec<library::RepoEntry> {
    library::discover(&folders, &repos)
}

/// Opens a repo file with the system's default app, or with `reveal` shows it selected in Finder / Explorer.
/// Our own command, so it isn't limited by the opener plugin's path scope.
#[tauri::command(async)]
fn open_file(repo: String, path: String, reveal: bool) -> Result<(), String> {
    let full = Path::new(&repo).join(path);
    let result = if reveal { tauri_plugin_opener::reveal_item_in_dir(full) } else { tauri_plugin_opener::open_path(full, None::<&str>) };
    result.map_err(|e| e.to_string())
}

#[derive(Serialize)]
struct CommitInfo {
    id: String,
    author: String,
    #[serde(rename = "authorEmail")]
    author_email: String,
    #[serde(rename = "authorDate")]
    author_date: i64,
    committer: String,
    #[serde(rename = "committerEmail")]
    committer_email: String,
    #[serde(rename = "committerDate")]
    committer_date: i64,
    parents: Vec<String>,
    refs: Vec<String>, // branches and tags pointing at it
    subject: String,
    body: String,
}

/// Everything about one commit, for the details above the file tree.
#[tauri::command(async)]
fn commit_info(repo: String, id: String) -> Result<CommitInfo, String> {
    let format = "--format=%H%x1f%an%x1f%ae%x1f%at%x1f%cn%x1f%ce%x1f%ct%x1f%P%x1f%D%x1f%s%x1f%b";
    let out = git::run(&["-c", "log.showSignature=false", "show", "-s", format, &id], Path::new(&repo))?;
    let f: Vec<&str> = out.splitn(11, '\x1f').collect();
    if f.len() < 11 {
        return Err(format!("Can't read commit {id}"));
    }
    let list = |s: &str, sep: &str| s.split(sep).map(str::trim).filter(|x| !x.is_empty()).map(String::from).collect();
    Ok(CommitInfo {
        id: f[0].into(),
        author: f[1].into(),
        author_email: f[2].into(),
        author_date: f[3].parse().unwrap_or(0),
        committer: f[4].into(),
        committer_email: f[5].into(),
        committer_date: f[6].parse().unwrap_or(0),
        parents: list(f[7], " "),
        refs: list(f[8], ","),
        subject: f[9].into(),
        body: f[10].trim().into(),
    })
}

/// Files changed (stat) and the full patch of one commit, for "Copy for AI (full)".
#[tauri::command(async)]
fn commit_patch(repo: String, id: String) -> Result<String, String> {
    git::run(&["-c", "log.showSignature=false", "show", "--format=", "--stat", "--patch", "--no-color", &id], Path::new(&repo))
}

#[derive(Serialize)]
struct Blame {
    lines: Vec<usize>, // per line of the file (1-based line n at n-1): index into `commits`
    commits: Vec<BlameCommit>,
}

#[derive(Serialize)]
struct BlameCommit {
    id: String,
    author: String,
    date: i64,
    subject: String,
}

/// Who last touched each line of `path` as of `rev` (None = the working tree, uncommitted lines included).
#[tauri::command(async)]
fn blame(repo: String, path: String, rev: Option<String>) -> Result<Blame, String> {
    let mut args = vec!["blame", "--line-porcelain"];
    args.extend(rev.as_deref());
    args.extend(["--", &path]);
    let out = git::run(&args, Path::new(&repo))?;
    let mut blame = Blame { lines: vec![], commits: vec![] };
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut current = 0;
    for line in out.lines() {
        if line.starts_with('\t') {
            blame.lines.push(current); // the line's text ends its record
        } else if let Some(value) = line.strip_prefix("author ") {
            blame.commits[current].author = value.into();
        } else if let Some(value) = line.strip_prefix("author-time ") {
            blame.commits[current].date = value.parse().unwrap_or(0);
        } else if let Some(value) = line.strip_prefix("summary ") {
            blame.commits[current].subject = value.into();
        } else if line.len() >= 40 && line[..40].chars().all(|c| c.is_ascii_hexdigit()) && line.as_bytes().get(40) == Some(&b' ') {
            let id = &line[..40];
            current = *index.entry(id.to_string()).or_insert_with(|| {
                blame.commits.push(BlameCommit { id: id.into(), author: String::new(), date: 0, subject: String::new() });
                blame.commits.len() - 1
            });
        }
    }
    Ok(blame)
}

#[tauri::command(async)]
fn search_commits(repos: Vec<String>, query: String) -> Vec<search::Hit> {
    search::commits(&repos, &query)
}

#[tauri::command(async)]
fn commit_places(repo: String, id: String) -> Vec<String> {
    search::places(&repo, &id)
}

#[tauri::command(async)]
fn is_repo(path: String) -> bool {
    library::is_repo(Path::new(&path))
}

#[derive(Serialize)]
struct History {
    commits: Vec<git::Commit>,
    rows: Vec<graph::WireRow>,
    names: Vec<Option<String>>, // branch each commit belongs to, by index, for the line tooltip
    #[serde(rename = "hasMore")]
    has_more: bool,
    current: Option<String>, // checked-out branch, None when detached
    base: Option<String>, // the branch pinned to the left (main / production / the user's pick)
    environments: Vec<String>, // qa, develop…: kept by "Only mine" and compared against on the branch cards
    #[serde(rename = "allBranches")]
    all_branches: Vec<String>, // every branch name, local or on origin: the environments picker
    local: Vec<String>, // every local branch, for "Change base"
    #[serde(rename = "shelvedBranches")]
    shelved_branches: Vec<String>, // branches with WIP the app put aside (stash): the "With WIP" filter
    #[serde(rename = "fastForwarded")]
    fast_forwarded: bool, // "Only this branch": it reached the base by a fast-forward, so there's no merge to draw
}

#[tauri::command(async)]
/// `focus` ([branch] or [branch, base]): only that branch's own line (first parents, minus what's already in
/// `base`), plus, once merged into `base`, that merge and the fork point; nothing else, not even HEAD; wins over `only_mine`. `path`: only the commits that touched that file.
fn load_history(
    repo: String, limit: usize, only_mine: bool, branches_only: bool, preferred: Option<String>, focus: Vec<String>,
    path: Option<String>, environments: Option<Vec<String>>, only: Option<Vec<String>>,
) -> Result<History, String> {
    let repo = Path::new(&repo);
    let (main, main_tip) = git::main_ref(repo, preferred.as_deref()).unzip();
    let (local, environments, all_branches) = branches::branches(repo, main.as_deref(), environments.as_deref());
    // Only mine: my branches plus the base branch and the environments (qa, develop…), so the graph keeps its spine.
    // A name from the graph may be a branch since deleted, or a stripped origin/ name: keep only refs git knows.
    // Focused on a branch that isn't checked out, or a file's history: no "Uncommitted changes" row.
    // A file's history skips the status (no uncommitted row there): only the branch name.
    let (current, head, changed) = if path.is_none() {
        git::head_status(repo)
    } else {
        (git::run(&["symbolic-ref", "--quiet", "--short", "HEAD"], repo).ok().map(|s| s.trim().to_string()), None, 0)
    };
    let focus: Vec<String> = focus.into_iter().filter(|r| git::run(&["rev-parse", "--verify", "--quiet", r], repo).is_ok()).collect();
    let focused = !focus.is_empty();
    let mut fast_forwarded = false;
    let mut commits = if focused {
        let tip = &focus[0];
        // Merged into the base by a real merge: the oldest base commit (first parents) that contains the tip is that
        // merge, and its first parent is the base right before it. A fast-forward has no merge to show.
        // The first base commit after the tip on the base's main line: a merge of it, or (fast-forward: its first
        // parent is the tip itself) simply where the base went on from this branch.
        // (--first-parent with --ancestry-path misses a merge that brought it in through another branch, like qa
        // merged into production: so the base's main line and the tip's descendants, crossed.)
        let next = focus.get(1).filter(|_| path.is_none()).and_then(|base| {
            let tip_id = git::run(&["rev-parse", tip], repo).ok()?.trim().to_string();
            let main_line = git::run(&["rev-list", "--first-parent", base], repo).ok()?;
            let main_line: Vec<&str> = main_line.lines().collect();
            if let Some(at) = main_line.iter().position(|c| *c == tip_id) {
                return Some((main_line.get(at.checked_sub(1)?)?.to_string(), true));
            }
            let after = git::run(&["rev-list", "--ancestry-path", &format!("{tip}..{base}")], repo).ok()?;
            let after: std::collections::HashSet<&str> = after.lines().collect();
            main_line.iter().rev().find(|c| after.contains(**c)).map(|c| (c.to_string(), false))
        });
        let merge = next.as_ref().filter(|(_, ff)| !ff).map(|(next, _)| (next.clone(), format!("{next}^1")));
        fast_forwarded = next.as_ref().is_some_and(|(_, ff)| *ff);
        let own = |exclude: bool| {
            let mut args = vec!["--first-parent".to_string(), tip.clone()];
            let stop = merge.as_ref().map(|(_, before)| before.clone()).or_else(|| focus.get(1).cloned());
            args.extend(stop.filter(|_| exclude).map(|stop| format!("^{stop}")));
            git::log(repo, Some(&args), false, path.as_deref(), limit)
        };
        // Already fully in its base (merged): its own line from the tip down instead of nothing.
        let mut commits = own(true)?;
        let fork = commits.last().and_then(|oldest| oldest.parents.first().cloned());
        if commits.is_empty() {
            commits = own(false)?;
        }
        // Merged: the merge commit on top and the fork point below, the base's line drawn straight between them,
        // so the branch is seen leaving the base and coming back.
        if let (Some((merge, _)), Some(fork), Some(newest)) = (&merge, fork, commits.first().map(|c| c.id.clone())) {
            let one = |id: &String| git::log(repo, Some(std::slice::from_ref(id)), false, None, 1).ok().and_then(|mut c| c.pop());
            if let (Some(mut merged), Some(mut forked)) = (one(merge), one(&fork)) {
                merged.parents = vec![forked.id.clone(), newest];
                forked.parents.clear();
                commits.insert(0, merged);
                commits.push(forked);
            }
        }
        // Fast-forwarded: no merge to show, but the base's next commit on top, on the same line, shows it went on from here.
        if let (Some((next, true)), Some(newest)) = (&next, commits.first().map(|c| c.id.clone())) {
            if let Some(mut after) = git::log(repo, Some(std::slice::from_ref(next)), false, None, 1)?.pop() {
                after.parents = vec![newest];
                commits.insert(0, after);
            }
        }
        // Merges keep only the parent shown: the merged-in branches are hidden, so no lines to nowhere.
        let ids: std::collections::HashSet<String> = commits.iter().map(|c| c.id.clone()).collect();
        for commit in &mut commits {
            commit.parents.retain(|p| ids.contains(p));
        }
        commits
    } else {
        let full = |r: &str| if r.starts_with("refs/") { r.to_string() } else if r.starts_with("origin/") { format!("refs/remotes/{r}") } else { format!("refs/heads/{r}") };
        // Only mine and/or a filter (`only`: the branches it matched): those, both when both are on.
        let picked: Option<Vec<String>> = match (only_mine, &only) {
            (false, None) => None,
            (true, None) => Some(git::my_branches(repo)),
            (false, Some(only)) => Some(only.iter().map(|r| full(r)).collect()),
            (true, Some(only)) => {
                let only: std::collections::HashSet<String> = only.iter().map(|r| full(r)).collect();
                Some(git::my_branches(repo).into_iter().filter(|r| only.contains(r)).collect())
            }
        };
        let mut refs = picked.map(|picked| {
            let shown: Vec<String> = picked.into_iter().chain(main.clone()).chain(environments.clone()).collect();
            // Walking from the base also reaches other branches merged into it: label only these refs (plus the
            // checked-out branch, HEAD and, unless filtering, tags), so their names and collapsed rows don't show up.
            let tags = only.is_none().then(|| "refs/tags/".to_string());
            let labels = shown.iter().map(|r| full(r)).chain(current.as_deref().map(full)).chain(["HEAD".into()]).chain(tags);
            labels.map(|r| format!("--decorate-refs={r}")).chain(shown.iter().cloned()).collect::<Vec<_>>()
        });
        // Branches only: just the commits a branch or tag points at, each linked to the next one down (rewritten parents).
        if branches_only && path.is_none() {
            let all = ["--branches", "--remotes", "--tags"].map(String::from).to_vec();
            refs = Some(["--simplify-by-decoration", "--parents"].map(String::from).into_iter().chain(refs.unwrap_or(all)).collect());
        }
        git::log(repo, refs.as_deref(), true, path.as_deref(), limit)?
    };
    let has_more = commits.len() >= limit;
    let worktree = git::worktree_commit(head, changed).filter(|w| !focused || commits.iter().any(|c| Some(&c.id) == w.parents.first()));
    if let Some(worktree) = worktree {
        // Right above the checked-out commit, not at the very top of the graph.
        let head = commits.iter().position(|c| Some(&c.id) == worktree.parents.first()).unwrap_or(0);
        commits.insert(head, worktree);
    }
    // Saved WIP of other branches: right above the commit it was saved on, when that commit is shown.
    let shelved = if path.is_none() { git::shelved_commits(repo) } else { vec![] };
    let shelved_branches: Vec<String> = shelved.iter().filter_map(|s| s.shelved.clone()).collect();
    for shelved in shelved {
        if let Some(at) = commits.iter().position(|c| Some(&c.id) == shelved.parents.first()) {
            commits.insert(at, shelved);
        }
    }
    let chain = git::main_chain(repo, main_tip.as_deref());
    let rows = graph::compute(&commits, &chain);
    let names = graph::branch_names(&commits, main.as_deref(), &chain);
    Ok(History {
        rows: graph::to_wire(&rows, &commits),
        names: commits.iter().map(|c| names.get(&c.id).cloned()).collect(),
        commits,
        has_more,
        current,
        base: main,
        environments,
        all_branches,
        local,
        fast_forwarded,
        shelved_branches,
    })
}

#[derive(Serialize)]
struct Changes {
    changes: HashMap<String, String>, // path -> status letter
    counts: HashMap<String, (u32, u32)>, // path -> lines added, removed
    paths: Vec<String>, // what the tree shows: changed files, or every file with `all`
    staged: Vec<String>, // uncommitted-changes row only: files in the index
}

/// Files of a selection. `target` None = the uncommitted-changes row; `base` None = root commit.
#[tauri::command(async)]
fn load_changes(repo: String, base: Option<String>, target: Option<String>, all: bool) -> Changes {
    let repo = Path::new(&repo);
    let changes = match &target {
        Some(t) => git::changed_files(base.as_deref(), t, repo),
        None => git::worktree_files(repo),
    };
    let mut counts = git::line_counts(base.as_deref(), target.as_deref(), repo);
    if target.is_none() {
        // Untracked files are not in `git diff`: count their lines.
        for (path, status) in &changes {
            if status == "A" && !counts.contains_key(path) {
                if let Ok(text) = std::fs::read_to_string(repo.join(path)) {
                    counts.insert(path.clone(), (text.matches('\n').count() as u32, 0));
                }
            }
        }
    }
    let mut paths: Vec<String> = changes.keys().cloned().collect();
    if all {
        paths.extend(git::all_files(target.as_deref(), repo));
        paths.sort();
        paths.dedup();
    }
    let staged = if target.is_none() { git::staged_files(repo) } else { vec![] };
    Changes { changes, counts, paths, staged }
}

/// File tree search by content: path -> lines containing `query` (case-insensitive, literal).
/// `diff_only`: only the added/removed lines of the selection. Otherwise whole files at `target`, limited to `paths` if given.
#[tauri::command(async)]
fn search_lines(repo: String, base: Option<String>, target: Option<String>, query: String, paths: Option<Vec<String>>, diff_only: bool) -> HashMap<String, u32> {
    let repo = Path::new(&repo);
    let mut hits = HashMap::new();
    if diff_only {
        let needle = query.to_lowercase();
        // shortcut: untracked files aren't in `git diff`, so the uncommitted row misses them here.
        let args: Vec<&str> = match (&base, &target) {
            (_, None) => vec!["diff", "-U0", "--no-color", "HEAD"],
            (Some(b), Some(t)) => vec!["diff", "-U0", "--no-color", b, t],
            (None, Some(t)) => vec!["show", "-U0", "--no-color", "--format=", t],
        };
        let out = git::run(&[&["-c", "core.quotePath=false"][..], &args].concat(), repo).unwrap_or_default();
        let mut path = String::new();
        for line in out.lines() {
            if let Some(p) = line.strip_prefix("+++ b/") { path = p.to_string(); continue; }
            if line.starts_with("+++ ") || line.starts_with("--- ") { continue; }
            if let Some(p) = line.strip_prefix("diff --git a/") { path = p.split(" b/").next().unwrap_or_default().to_string(); continue; }
            if (line.starts_with('+') || line.starts_with('-')) && line[1..].to_lowercase().contains(&needle) {
                *hits.entry(path.clone()).or_insert(0) += 1;
            }
        }
        return hits;
    }
    let mut args = vec!["-c", "core.quotePath=false", "grep", "-c", "-I", "-i", "-F", "-e", &query];
    match &target {
        Some(t) => args.push(t),
        None => args.push("--untracked"),
    }
    args.push("--");
    if let Some(paths) = &paths { args.extend(paths.iter().map(String::as_str)); }
    let prefix = target.as_ref().map(|t| format!("{t}:")).unwrap_or_default();
    // "rev:path:count" (no "rev:" for the working tree); git grep exits 1 when nothing matches.
    for line in git::run(&args, repo).unwrap_or_default().lines() {
        let line = line.strip_prefix(&prefix).unwrap_or(line);
        if let Some((path, count)) = line.rsplit_once(':') {
            if let Ok(count) = count.parse() { hits.insert(path.to_string(), count); }
        }
    }
    hits
}

#[derive(Serialize)]
struct FileDiff {
    rows: Vec<diff::DiffRow>,
    touching: Vec<String>, // commits of a collapsed group that changed this file, newest first
}

#[tauri::command(async)]
/// `whole`: the file didn't change in this selection (whole-project tree): its full content, one column, no diff.
fn load_diff(repo: String, path: String, base: Option<String>, target: Option<String>, group: Vec<String>, whole: bool) -> FileDiff {
    let repo = Path::new(&repo);
    let mut rows = if whole {
        let text = match &target {
            Some(target) => git::run(&["show", &format!("{target}:{path}")], repo).unwrap_or_default(),
            None => std::fs::read_to_string(repo.join(&path)).unwrap_or_default(),
        };
        let text = text.strip_suffix('\n').unwrap_or(&text);
        let lines = if text.is_empty() { vec![] } else { text.split('\n').collect() };
        lines.into_iter().enumerate()
            .map(|(i, line)| diff::DiffRow { right: Some(diff::Side { number: i + 1, text: line.to_string(), changed: None }), ..Default::default() })
            .collect()
    } else {
        diff::rows(&git::file_diff(&path, base.as_deref(), target.as_deref(), repo))
    };
    rows.truncate(5000); // shortcut: caps huge diffs at 5000 rows, add paging when needed
    let touching = if whole { vec![] } else if group.len() > 1 { git::commits_touching(&path, &group, repo) } else { group };
    FileDiff { rows, touching }
}

/// Behind/merged facts of the branch cards in "Commits collapsed", only for the ones visible in the graph.
#[tauri::command(async)]
fn branch_status(repo: String, refs: Vec<String>, base: String, targets: Vec<String>) -> Vec<branches::Status> {
    branches::status(Path::new(&repo), &refs, &base, &targets)
}

/// Filters, fast pass: every branch (yours only with `only_mine`) and, per target, the ones already in it by history.
#[tauri::command(async)]
fn merged_into(repo: String, targets: Vec<String>, only_mine: bool) -> branches::Merged {
    branches::merged_into(Path::new(&repo), &targets, only_mine)
}

/// The Logs panel: git commands run since `since` (an id), with their output.
#[tauri::command(async)]
fn git_logs(since: u64) -> Vec<git::LogEntry> {
    git::logs(since)
}

// Actions that change the repo (see actions.rs). Errors come back as git's stderr.

#[tauri::command(async)]
fn sync(repo: String) -> Result<actions::SyncResult, String> {
    actions::sync(Path::new(&repo))
}

#[tauri::command(async)]
fn changed_file_count(repo: String) -> usize {
    actions::changed_file_count(Path::new(&repo))
}

#[tauri::command(async)]
fn checkout(repo: String, name: String, wip: Option<actions::WipMode>) -> Result<String, String> {
    actions::checkout(&name, wip, Path::new(&repo))
}

#[tauri::command(async)]
fn pull(repo: String, name: String) -> Result<String, String> {
    actions::pull(&name, Path::new(&repo))
}

#[tauri::command(async)]
fn delete_branch(repo: String, branch: String, remote: bool) -> Result<String, String> {
    actions::delete_branch(&branch, remote, Path::new(&repo))
}

#[tauri::command(async)]
fn update_branch(repo: String, branch: String, parent: String, rebase: bool, is_current: bool) -> Result<(), String> {
    actions::update(&branch, &parent, rebase, is_current, Path::new(&repo))
}

#[tauri::command(async)]
fn base_status(repo: String, base: String) -> actions::BaseStatus {
    actions::base_status(&base, Path::new(&repo))
}

#[tauri::command(async)]
fn start_feature(repo: String, name: String, base: String, wip: Option<actions::WipMode>) -> Result<String, String> {
    actions::start_feature(&name, &base, wip, Path::new(&repo))
}

#[tauri::command(async)]
fn move_branch(repo: String, branch: String, expected: Option<String>, target: String) -> Result<(), String> {
    actions::move_branch(&branch, expected.as_deref(), &target, Path::new(&repo))
}

/// Commit a local branch points at, None if there's no such branch.
#[tauri::command(async)]
fn branch_tip(repo: String, branch: String) -> Option<String> {
    git::run(&["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")], Path::new(&repo)).ok().map(|s| s.trim().to_string())
}

#[tauri::command(async)]
fn force_push(repo: String, branch: String) -> Result<(), String> {
    actions::force_push(&branch, Path::new(&repo))
}

#[tauri::command(async)]
fn discard_file(repo: String, path: String) -> Result<(), String> {
    actions::discard(&path, Path::new(&repo))
}

#[tauri::command(async)]
fn stage(repo: String, paths: Vec<String>) -> Result<(), String> {
    actions::stage(&paths, Path::new(&repo))
}

#[tauri::command(async)]
fn unstage(repo: String, paths: Vec<String>) -> Result<(), String> {
    actions::unstage(&paths, Path::new(&repo))
}

/// Placeholder for the commit form: the previous commit's message.
#[tauri::command(async)]
fn last_commit_message(repo: String) -> String {
    git::run(&["log", "-1", "--format=%B"], Path::new(&repo)).unwrap_or_default().trim().to_string()
}

#[tauri::command(async)]
fn commit(repo: String, message: String, push: bool) -> Result<String, String> {
    actions::commit(&message, push, Path::new(&repo))
}

#[tauri::command(async)]
fn discard_all(repo: String) -> Result<(), String> {
    actions::discard_all(Path::new(&repo))
}

/// One block of the working-tree diff of `path`, numbered like `DiffRow.hunk`.
#[tauri::command(async)]
fn discard_hunk(repo: String, path: String, index: usize) -> Result<(), String> {
    let repo = Path::new(&repo);
    let text = git::file_diff(&path, None, None, repo);
    // An untracked file's diff is synthesized (no `diff --git` header): its only block is the whole file.
    if text.contains("diff --git") { actions::discard_hunk(index, &text, repo) } else { actions::discard(&path, repo) }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![list_repos, load_history, load_changes, load_diff, search_lines, branch_status, merged_into, git_logs,
            sync, changed_file_count, checkout, pull, delete_branch, update_branch, force_push, discard_file, discard_all, discard_hunk,
            stage, unstage, last_commit_message, commit, is_repo, open_file, search_commits, commit_places, commit_info, commit_patch, blame, move_branch, branch_tip, base_status, start_feature])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use crate::testutil::Repo;

    /// "Only this branch" on a merged branch: the merge on top, its own commits, the fork point below, linked up.
    #[test]
    fn focus_shows_where_a_merged_branch_left_and_came_back() {
        let repo = Repo::new("focus-merge");
        repo.commit("a.txt", "1", "init");
        repo.git(&["checkout", "-q", "-b", "feat"]);
        repo.commit("f.txt", "1", "feat 1");
        repo.commit("f.txt", "2", "feat 2");
        repo.git(&["checkout", "-q", "main"]);
        repo.commit("a.txt", "2", "main moves");
        repo.git(&["merge", "-q", "--no-ff", "-m", "merge feat", "feat"]);
        repo.commit("a.txt", "3", "after");
        let path = repo.path().to_string_lossy().to_string();
        let history = super::load_history(path, 300, false, false, Some("main".into()), vec!["feat".into(), "main".into()], None, None, None).unwrap();
        let subjects: Vec<&str> = history.commits.iter().map(|c| c.subject.as_str()).collect();
        assert_eq!(subjects, ["merge feat", "feat 2", "feat 1", "init"]);
        let id = |i: usize| history.commits[i].id.clone();
        assert_eq!(history.commits[0].parents, [id(3), id(1)], "merge links to the fork point and the tip");
        assert_eq!(history.commits[2].parents, [id(3)], "the branch starts at the fork point");
        assert!(!history.fast_forwarded);

        // Merged through another branch (feature → qa, then qa → main): that qa merge is where it came back.
        repo.git(&["checkout", "-q", "-b", "qa2"]);
        repo.git(&["checkout", "-q", "-b", "via"]);
        repo.commit("v.txt", "1", "via 1");
        repo.git(&["checkout", "-q", "qa2"]);
        repo.git(&["merge", "-q", "--no-ff", "-m", "via into qa2", "via"]);
        repo.git(&["checkout", "-q", "main"]);
        repo.commit("a.txt", "3b", "main moves again");
        repo.git(&["merge", "-q", "--no-ff", "-m", "qa2 into main", "qa2"]);
        let path = repo.path().to_string_lossy().to_string();
        let history = super::load_history(path, 300, false, false, Some("main".into()), vec!["via".into(), "main".into()], None, None, None).unwrap();
        assert_eq!(history.commits[0].subject, "qa2 into main");
        assert_eq!(history.commits[1].subject, "via 1");

        // Fast-forwarded: no merge, the base's next commit on top of the tip, same line; its card says so too.
        repo.git(&["checkout", "-q", "-b", "ff"]);
        repo.commit("g.txt", "1", "ff 1");
        repo.git(&["checkout", "-q", "main"]);
        repo.git(&["merge", "-q", "--ff-only", "ff"]);
        repo.commit("a.txt", "4", "main after ff");
        let path = repo.path().to_string_lossy().to_string();
        let history = super::load_history(path, 300, false, false, Some("main".into()), vec!["ff".into(), "main".into()], None, None, None).unwrap();
        assert!(history.fast_forwarded);
        assert_eq!(history.commits[0].subject, "main after ff");
        assert_eq!(history.commits[0].parents, [history.commits[1].id.clone()]);
        let card = crate::branches::status(repo.path(), &["ff".into()], "main", &["main".into()]);
        assert_eq!(card[0].promotion, [("main".to_string(), 0, "fast-forwarded")]);
    }
}
