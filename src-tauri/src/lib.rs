mod actions;
mod dashboard;
mod diff;
mod git;
mod graph;
mod library;
mod search;

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
}

#[tauri::command(async)]
/// `focus` ([branch] or [branch, base]): only that branch's own line (first parents, minus what's already in
/// `base`), nothing else, not even HEAD; wins over `only_mine`.
fn load_history(repo: String, limit: usize, only_mine: bool, preferred: Option<String>, environments: Vec<String>, focus: Vec<String>) -> Result<History, String> {
    let repo = Path::new(&repo);
    let main = git::main_ref(repo, preferred.as_deref());
    // Only mine: my branches plus the base branch and the environments (qa, develop…), so the graph keeps its spine.
    // A name from the graph may be a branch since deleted, or a stripped origin/ name: keep only refs git knows.
    let focus: Vec<String> = focus.into_iter().filter(|r| git::run(&["rev-parse", "--verify", "--quiet", r], repo).is_ok()).collect();
    let focused = !focus.is_empty();
    let mut commits = if focused {
        let own = |exclude: bool| {
            let mut args = vec!["--first-parent".to_string(), focus[0].clone()];
            args.extend(focus.get(1).filter(|_| exclude).map(|base| format!("^{base}")));
            git::log(repo, Some(&args), false, limit)
        };
        // Already fully in its base (merged): its own line from the tip down instead of nothing.
        let mut commits = own(true)?;
        if commits.is_empty() {
            commits = own(false)?;
        }
        // Merges keep only the parent shown: the merged-in branches are hidden, so no lines to nowhere.
        let ids: std::collections::HashSet<String> = commits.iter().map(|c| c.id.clone()).collect();
        for commit in &mut commits {
            commit.parents.retain(|p| ids.contains(p));
        }
        commits
    } else {
        let refs = only_mine.then(|| git::my_branches(repo).into_iter().chain(main.clone()).chain(environments).collect::<Vec<_>>());
        git::log(repo, refs.as_deref(), true, limit)?
    };
    let has_more = commits.len() >= limit;
    // Focused on a branch that isn't checked out: no "Uncommitted changes" row.
    let worktree = git::worktree_commit(repo).filter(|w| !focused || commits.iter().any(|c| Some(&c.id) == w.parents.first()));
    if let Some(worktree) = worktree {
        // Right above the checked-out commit, not at the very top of the graph.
        let head = commits.iter().position(|c| Some(&c.id) == worktree.parents.first()).unwrap_or(0);
        commits.insert(head, worktree);
    }
    let chain = git::main_chain(repo, main.as_deref());
    let rows = graph::compute(&commits, &chain);
    let names = graph::branch_names(&commits, main.as_deref(), &chain);
    Ok(History {
        rows: graph::to_wire(&rows, &commits),
        names: commits.iter().map(|c| names.get(&c.id).cloned()).collect(),
        commits,
        has_more,
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

/// Per-repo prefs from the frontend: pins, the environment list (None = detect by name), "active"/"old" per feature.
#[tauri::command(async)]
fn load_dashboard(
    repo: String,
    preferred: Option<String>,
    pinned: Vec<String>,
    environments: Option<Vec<String>>,
    states: HashMap<String, String>,
) -> dashboard::Dashboard {
    dashboard::load(Path::new(&repo), preferred.as_deref(), &pinned, environments.as_deref(), &states)
}

/// First-parent commits of a branch from its tip down to where it was cut (`base`), for the graph highlight.
#[tauri::command(async)]
fn branch_chain(repo: String, base: String, reference: String) -> Vec<String> {
    let log = git::run(&["rev-list", "--first-parent", &format!("{base}..{reference}")], Path::new(&repo)).unwrap_or_default();
    log.lines().map(String::from).chain([base]).collect()
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
fn update_prompt(repo: String, branch: String) -> Result<String, String> {
    actions::update_prompt(&branch, Path::new(&repo))
}

#[tauri::command(async)]
fn squash(repo: String, branch: String, base: String, expected_tip: String, message: String) -> Result<(), String> {
    actions::squash(&branch, &base, &expected_tip, &message, Path::new(&repo))
}

#[tauri::command(async)]
fn update_branch(repo: String, branch: String, parent: String, rebase: bool, is_current: bool) -> Result<(), String> {
    actions::update(&branch, &parent, rebase, is_current, Path::new(&repo))
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
        .invoke_handler(tauri::generate_handler![list_repos, load_history, load_changes, load_diff, load_dashboard, branch_chain,
            sync, changed_file_count, checkout, pull, update_prompt, delete_branch, squash, update_branch, force_push, discard_file, discard_all, discard_hunk,
            stage, unstage, last_commit_message, commit, is_repo, open_file, search_commits, commit_places, commit_info, commit_patch])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
