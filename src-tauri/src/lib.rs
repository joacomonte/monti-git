mod actions;
mod dashboard;
mod diff;
mod git;
mod graph;
mod library;

use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// `async` in the attribute runs these on a worker thread, so git never blocks the UI.

#[tauri::command(async)]
fn list_repos(folder: Option<String>, extras: Vec<String>) -> Vec<library::RepoEntry> {
    let folder = folder.map(PathBuf::from).unwrap_or_else(library::default_folder);
    library::discover(&folder, &extras)
}

#[derive(Serialize)]
struct History {
    commits: Vec<git::Commit>,
    rows: Vec<graph::WireRow>,
    runs: Vec<(usize, usize)>, // collapsible groups: first commit index -> last
    names: Vec<Option<String>>, // branch each commit belongs to, by index, for the line tooltip
    #[serde(rename = "hasMore")]
    has_more: bool,
}

#[tauri::command(async)]
fn load_history(repo: String, limit: usize, only_mine: bool, preferred: Option<String>) -> Result<History, String> {
    let repo = Path::new(&repo);
    let main = git::main_ref(repo, preferred.as_deref());
    // Only mine: my branches plus the base branch, so the graph keeps its spine.
    let refs = only_mine.then(|| git::my_branches(repo).into_iter().chain(main.clone()).collect::<Vec<_>>());
    let mut commits = git::log(repo, refs.as_deref(), limit)?;
    let has_more = commits.len() >= limit;
    if let Some(worktree) = git::worktree_commit(repo) {
        // Right above the checked-out commit, not at the very top of the graph.
        let head = commits.iter().position(|c| Some(&c.id) == worktree.parents.first()).unwrap_or(0);
        commits.insert(head, worktree);
    }
    let chain = git::main_chain(repo, main.as_deref());
    let rows = graph::compute(&commits, &chain);
    let names = graph::branch_names(&commits, main.as_deref(), &chain);
    Ok(History {
        runs: graph::linear_runs(&commits, &rows, 3),
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
    Changes { changes, counts, paths }
}

#[derive(Serialize)]
struct FileDiff {
    rows: Vec<diff::DiffRow>,
    touching: Vec<String>, // commits of a collapsed group that changed this file, newest first
}

#[tauri::command(async)]
fn load_diff(repo: String, path: String, base: Option<String>, target: Option<String>, group: Vec<String>) -> FileDiff {
    let repo = Path::new(&repo);
    let text = git::file_diff(&path, base.as_deref(), target.as_deref(), repo);
    let mut rows = diff::rows(&text);
    rows.truncate(5000); // shortcut: caps huge diffs at 5000 rows, add paging when needed
    let touching = if group.len() > 1 { git::commits_touching(&path, &group, repo) } else { group };
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
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![list_repos, load_history, load_changes, load_diff, load_dashboard, branch_chain,
            sync, changed_file_count, checkout, pull, squash, update_branch, force_push, discard_file, discard_all, discard_hunk])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
