//! The commands the web view calls: one line each into the module that owns the work. `async` in the attribute
//! runs them on a worker thread, so git never blocks the UI. Errors come back as text (git's stderr, mostly).
mod actions;
mod blame;
mod branches;
mod changes;
mod diff;
mod editors;
mod git;
mod graph;
mod history;
mod library;
mod par;
mod patch;
mod refs;
mod search;
#[cfg(test)]
mod testutil;
mod wip;

use git::Span;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

type Result<T> = std::result::Result<T, String>;

#[tauri::command(async)]
fn list_repos(folders: Vec<String>, repos: Vec<String>) -> Vec<library::RepoEntry> {
    library::discover(&folders, &repos)
}

#[tauri::command(async)]
fn is_repo(path: String) -> bool {
    library::is_repo(Path::new(&path))
}

/// How to open a repo file: shown selected in Finder / Explorer, with the system's default app, or in a code editor.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum Open {
    Reveal,
    Default,
    Editor(String),
}

/// Our own command, so it isn't limited by the opener plugin's path scope.
#[tauri::command(async)]
fn open_file(repo: String, path: String, with: Open) -> Result<()> {
    let full = git::repo_file(Path::new(&repo), &path)?;
    match with {
        Open::Reveal => tauri_plugin_opener::reveal_item_in_dir(full).map_err(|e| e.to_string()),
        Open::Default => tauri_plugin_opener::open_path(full, None::<&str>).map_err(|e| e.to_string()),
        Open::Editor(id) => editors::open(&id, Path::new(&repo), &full),
    }
}

#[tauri::command(async)]
fn list_editors() -> Vec<editors::Editor> {
    editors::installed()
}

// MARK: Reading

#[tauri::command(async)]
fn load_history(repo: String, query: history::Query) -> Result<history::History> {
    history::load(Path::new(&repo), &query)
}

/// Files of a selection. `commit` None = the uncommitted-changes row.
#[tauri::command(async)]
fn load_changes(repo: String, commit: Option<String>, all: bool) -> Result<changes::Changes> {
    changes::load(Path::new(&repo), Span::new(commit.as_deref()), all)
}

/// One file of a selection. `whole`: its full content instead of its diff.
#[tauri::command(async)]
fn load_diff(repo: String, path: String, commit: Option<String>, whole: bool) -> Result<changes::FileDiff> {
    changes::file_diff(Path::new(&repo), &path, Span::new(commit.as_deref()), whole)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CommitInfo {
    id: String,
    author: String,
    author_email: String,
    author_date: i64,
    committer: String,
    committer_email: String,
    committer_date: i64,
    parents: Vec<String>,
    refs: Vec<String>, // branches and tags pointing at it
    subject: String,
    body: String,
}

/// Everything about one commit, for the details above the file tree.
#[tauri::command(async)]
fn commit_info(repo: String, id: String) -> Result<CommitInfo> {
    let format = "--format=%H%x1f%an%x1f%ae%x1f%at%x1f%cn%x1f%ce%x1f%ct%x1f%P%x1f%D%x1f%s%x1f%b";
    let out = git::run(&["show", "-s", format, "--end-of-options", &id], Path::new(&repo))?;
    let f: Vec<&str> = out.splitn(11, '\x1f').collect();
    let [id, author, author_email, author_date, committer, committer_email, committer_date, parents, refs, subject, body] = f[..] else {
        return Err(format!("Can't read commit {id}"));
    };
    Ok(CommitInfo {
        id: id.into(),
        author: author.into(),
        author_email: author_email.into(),
        author_date: author_date.parse().unwrap_or_default(),
        committer: committer.into(),
        committer_email: committer_email.into(),
        committer_date: committer_date.parse().unwrap_or_default(),
        parents: parents.split_whitespace().map(String::from).collect(),
        refs: git::decorations(refs),
        subject: subject.into(),
        body: body.trim().into(),
    })
}

#[tauri::command(async)]
fn blame(repo: String, path: String, rev: Option<String>) -> Result<blame::Blame> {
    blame::blame(&path, rev.as_deref(), Path::new(&repo))
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

/// Commit a local branch points at, None if there's no such branch.
#[tauri::command(async)]
fn branch_tip(repo: String, branch: String) -> Option<String> {
    refs::tip(&format!("refs/heads/{branch}"), Path::new(&repo))
}

/// Placeholder for the commit form: the previous commit's message.
#[tauri::command(async)]
fn last_commit_message(repo: String) -> String {
    git::read_opt(&["log", "-1", "--format=%B"], Path::new(&repo)).unwrap_or_default().trim().to_string()
}

/// Staged, unstaged and untracked files.
#[tauri::command(async)]
fn changed_file_count(repo: String) -> Result<usize> {
    Ok(git::status(Path::new(&repo))?.files.len())
}

/// The Logs panel: git commands run since `since` (an id), with their output.
#[tauri::command(async)]
fn git_logs(since: u64) -> Vec<git::LogEntry> {
    git::logs(since)
}

// MARK: Search

#[tauri::command(async)]
fn search_commits(repos: Vec<String>, query: String) -> search::Found<search::CommitHit> {
    search::commits(&repos, &query)
}

#[tauri::command(async)]
fn search_code(repos: Vec<String>, bases: HashMap<String, String>, query: String, scope: search::Scope) -> search::Found<search::CodeHit> {
    search::code(&repos, &bases, &query, scope)
}

/// File tree search by content in a selection: path -> its matching lines.
#[tauri::command(async)]
fn search_lines(repo: String, commit: Option<String>, query: String, paths: Vec<String>, diff_only: bool) -> HashMap<String, search::LineHits> {
    let paths: Vec<&str> = paths.iter().map(String::as_str).collect();
    search::lines(Path::new(&repo), Span::new(commit.as_deref()), &query, &paths, diff_only)
}

#[tauri::command(async)]
fn commit_places(repo: String, id: String) -> Vec<search::Place> {
    search::places(&repo, &id)
}

// MARK: Actions (actions.rs)

#[tauri::command(async)]
fn sync(repo: String) -> Result<actions::SyncResult> {
    actions::sync(Path::new(&repo))
}

#[tauri::command(async)]
fn checkout(repo: String, name: String, wip: Option<actions::WipMode>) -> Result<String> {
    actions::checkout(&name, wip, Path::new(&repo))
}

#[tauri::command(async)]
fn pull(repo: String, name: String) -> Result<String> {
    actions::pull(&name, Path::new(&repo))
}

#[tauri::command(async)]
fn delete_branch(repo: String, branch: String, base: String, targets: Vec<String>, remote: bool) -> Result<String> {
    actions::delete_branch(&branch, &base, &targets, remote, Path::new(&repo))
}

/// Fetches, then whether `branch` can be deleted and where it is: the delete dialog's options.
#[tauri::command(async)]
fn deletion(repo: String, branch: String, base: String, targets: Vec<String>) -> actions::Deletion {
    actions::deletion(&branch, &base, &targets, Path::new(&repo))
}

#[tauri::command(async)]
fn update_branch(repo: String, branch: String, parent: String, rebase: bool) -> Result<()> {
    actions::update(&branch, &parent, rebase, Path::new(&repo))
}

#[tauri::command(async)]
fn base_status(repo: String, base: String) -> actions::BaseStatus {
    actions::base_status(&base, Path::new(&repo))
}

#[tauri::command(async)]
fn start_feature(repo: String, name: String, base: String, wip: Option<actions::WipMode>) -> Result<String> {
    actions::start_feature(&name, &base, wip, Path::new(&repo))
}

#[tauri::command(async)]
fn move_branch(repo: String, branch: String, expected: Option<String>, target: String) -> Result<()> {
    actions::move_branch(&branch, expected.as_deref(), &target, Path::new(&repo))
}

/// Pushes `branch` (setting its upstream the first time); `force` with --force-with-lease.
#[tauri::command(async)]
fn push(repo: String, branch: String, force: bool) -> Result<actions::Pushed> {
    actions::push(&branch, force, Path::new(&repo))
}

#[tauri::command(async)]
fn discard_file(repo: String, path: String) -> Result<()> {
    actions::discard(&path, Path::new(&repo))
}

#[tauri::command(async)]
fn discard_all(repo: String) -> Result<()> {
    actions::discard_all(Path::new(&repo))
}

#[tauri::command(async)]
fn discard_hunk(repo: String, path: String, index: usize, header: String) -> Result<()> {
    actions::discard_hunk(&path, index, &header, Path::new(&repo))
}

#[tauri::command(async)]
fn stage(repo: String, paths: Vec<String>) -> Result<()> {
    actions::stage(&paths, Path::new(&repo))
}

#[tauri::command(async)]
fn unstage(repo: String, paths: Vec<String>) -> Result<()> {
    actions::unstage(&paths, Path::new(&repo))
}

#[tauri::command(async)]
fn commit(repo: String, message: String, push: bool) -> Result<String> {
    actions::commit(&message, push, Path::new(&repo))
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            list_repos, is_repo, open_file, list_editors,
            load_history, load_changes, load_diff, commit_info, blame, branch_status, merged_into, branch_tip, last_commit_message,
            changed_file_count, git_logs,
            search_commits, search_code, search_lines, commit_places,
            sync, checkout, pull, delete_branch, deletion, update_branch, base_status, start_feature, move_branch, push,
            discard_file, discard_all, discard_hunk, stage, unstage, commit,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
