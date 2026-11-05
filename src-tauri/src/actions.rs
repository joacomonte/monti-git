//! Actions that change the repo, ported from Git.swift. Same safety rules: fast-forward only, never a half-done
//! merge/rebase, uncommitted work always stashed or refused, never lost. One action at a time per repo: each takes
//! the repo's lock, so a background sync can't interleave with a checkout's stash, switch and pop.
use crate::git::{self, read_opt, run, write, write_input};
use crate::refs::{self, current_branch, local_name, DEFAULT_REMOTE};
use crate::{branches, patch, wip};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};

/// Runs `action` while holding `repo`'s lock.
fn exclusive<T>(repo: &Path, action: impl FnOnce() -> T) -> T {
    static LOCKS: LazyLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = LazyLock::new(Mutex::default);
    let lock = LOCKS.lock().unwrap().entry(repo.to_path_buf()).or_default().clone();
    let _held = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    action()
}

/// Fast-forwards `branch` to `upstream` (a ref), never merging: the checked-out one with `merge --ff-only`, which
/// refuses over local changes in the way; any other with a "+"-less local fetch, which git refuses for a branch
/// checked out in another worktree.
fn fast_forward(branch: &str, upstream: &str, is_current: bool, repo: &Path) -> Result<String, String> {
    if is_current {
        write(&["merge", "--ff-only", upstream], repo)
    } else {
        write(&["fetch", ".", &format!("{upstream}:refs/heads/{branch}")], repo)
    }
}

fn fetch(repo: &Path) -> Result<String, String> {
    write(&["fetch", "--all", "--prune"], repo)
}

fn changed_files(repo: &Path) -> usize {
    git::status(repo).map_or(0, |s| s.files.len())
}

// MARK: Sync and pull

/// Why sync left a branch as it was.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SkipReason {
    Diverged,            // commits here and on the server: not a fast-forward
    LocalChanges,        // the checked-out branch, and uncommitted changes are in the way
    CheckedOutElsewhere, // checked out in another worktree
}

#[derive(Serialize)]
pub struct Skipped {
    branch: String,
    reason: SkipReason,
}

#[derive(Serialize, Default)]
pub struct SyncResult {
    updated: Vec<String>,
    skipped: Vec<Skipped>,
    log: Vec<String>, // raw git errors of the skipped branches, for debugging
    changed: bool,    // any ref moved (fetch, prune or fast-forward): the graph needs a reload
}

/// Fetches every remote with --prune, then fast-forwards local branches that are strictly behind their upstream.
/// Never rewrites history, deletes local branches or touches uncommitted work: anything that is not a clean
/// fast-forward is skipped and reported. `--prune` only removes remote-tracking refs whose branch was deleted.
pub fn sync(repo: &Path) -> Result<SyncResult, String> {
    exclusive(repo, || {
        let refs = || read_opt(&["for-each-ref", "--format=%(refname) %(objectname)"], repo);
        let before = refs();
        fetch(repo)?;
        let current = current_branch(repo);
        let branches = run(&["for-each-ref", "--format=%(refname:short)%09%(upstream)%09%(upstream:track)", "refs/heads"], repo)?;
        let mut result = SyncResult::default();
        for line in branches.lines() {
            let fields: Vec<&str> = line.split('\t').collect();
            let [branch, upstream, track] = fields[..] else { continue };
            if upstream.is_empty() || !track.contains("behind") {
                continue;
            }
            if track.contains("ahead") {
                result.skipped.push(Skipped { branch: branch.into(), reason: SkipReason::Diverged });
                result.log.push(diverged_report(branch, upstream, track, repo));
                continue;
            }
            let is_current = current.as_deref() == Some(branch);
            match fast_forward(branch, upstream, is_current, repo) {
                Ok(_) => result.updated.push(branch.to_string()),
                Err(error) => {
                    result.log.push(format!("[{branch}] {}", error.trim()));
                    let reason = if is_current { SkipReason::LocalChanges } else { SkipReason::CheckedOutElsewhere };
                    result.skipped.push(Skipped { branch: branch.into(), reason });
                }
            }
        }
        result.changed = before.is_none() || refs() != before;
        Ok(result)
    })
}

/// Raw details of a diverged branch for the sync log: what each side has, and commands that fix it.
fn diverged_report(branch: &str, upstream: &str, track: &str, repo: &Path) -> String {
    let remote = refs::short_name(upstream);
    let oneline = |range: String| read_opt(&["log", "--format=  %h %an, %ar: %s", "-5", &range], repo).unwrap_or_default().trim_matches('\n').to_string();
    let path = repo.display();
    let mut lines = vec![
        format!("[{branch}] not updated: diverged from {remote} {track}. Sync only fast-forwards, so it left it as is."),
        "Only here (newest 5):".into(),
        oneline(format!("{remote}..{branch}")),
        "Only on the server (newest 5):".into(),
        oneline(format!("{branch}..{remote}")),
    ];
    if remote.split_once('/').map(|(_, name)| name) != Some(branch) {
        lines.push(format!("Note: {branch} follows {remote}, a branch with a different name. Usually a mistake: stop tracking it."));
    }
    lines.extend([
        "Fix, pick one:".into(),
        "  # Stop tracking (deletes nothing; sync stops reporting it):".into(),
        format!("  git -C \"{path}\" branch --unset-upstream {branch}"),
        "  # Bring the server's commits in (may ask you to fix conflicts):".into(),
        format!("  git -C \"{path}\" switch {branch} && git -C \"{path}\" pull --rebase"),
    ]);
    lines.join("\n")
}

/// Fetch, then fast-forward one branch to its upstream without checking it out. Returns a one-line status.
pub fn pull(name: &str, repo: &Path) -> Result<String, String> {
    exclusive(repo, || {
        fetch(repo)?;
        let branch = local_name(name);
        let info = read_opt(&["for-each-ref", "--format=%(upstream)%09%(upstream:track)", &format!("refs/heads/{branch}")], repo).unwrap_or_default();
        let info = info.trim_end_matches('\n');
        if info.is_empty() {
            return Ok(format!("Fetched {name}")); // remote-only branch: fetching was the pull
        }
        let (upstream, track) = info.split_once('\t').unwrap_or((info, ""));
        Ok(match () {
            _ if upstream.is_empty() => format!("{branch} has no upstream"),
            _ if track.contains("gone") => format!("{branch}: upstream branch was deleted"),
            _ if !track.contains("behind") => format!("{branch} up to date"),
            _ if track.contains("ahead") => format!("{branch} diverged from upstream, not pulled"),
            _ => {
                let is_current = current_branch(repo).as_deref() == Some(branch);
                match fast_forward(branch, upstream, is_current, repo) {
                    Ok(_) => format!("Pulled {branch}"),
                    Err(_) if is_current => format!("{branch} not pulled: local changes in the way"),
                    Err(_) => format!("{branch} not pulled: checked out elsewhere"),
                }
            }
        })
    })
}

// MARK: Checkout

/// What to do with uncommitted changes when checking out another branch (asked every time).
#[derive(Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum WipMode {
    Shelve, // stash them for the branch I'm leaving; restored when I come back to it
    Carry,  // bring them along to the target branch
    Commit, // WIP commit on the branch I'm leaving; undone when I come back to it
}

/// Applies a stash onto a clean tree; on conflict undoes the half-applied result (the tree was clean before, so
/// reset + clean only removes what the apply wrote) and leaves the stash where it is.
fn apply_cleanly(reference: &str, repo: &Path) -> bool {
    if write(&["stash", "apply", reference], repo).is_ok() {
        return true;
    }
    let _ = write(&["reset", "--hard", "--quiet"], repo);
    let _ = write(&["clean", "-fd", "--quiet"], repo);
    false
}

/// Fetch, switch to the branch, then fast-forward it to its upstream. Never merges or overwrites: a diverged branch
/// is switched to but left as is. Uncommitted changes are put aside first (`wip`), so the switch never fails because
/// of them, and only re-applied when that applies cleanly: otherwise they stay in the stash or go back where they were.
pub fn checkout(name: &str, wip: Option<WipMode>, repo: &Path) -> Result<String, String> {
    exclusive(repo, || switch_to(name, wip, true, repo))
}

fn switch_to(name: &str, wip: Option<WipMode>, fetch_first: bool, repo: &Path) -> Result<String, String> {
    let fetched = !fetch_first || fetch(repo).is_ok();
    let branch = local_name(name);
    let has_local = refs::tip(&format!("refs/heads/{branch}"), repo).is_some();
    // Where we were, to come back to: the branch, or HEAD's commit when detached.
    let original = current_branch(repo);
    let back_to: Vec<String> = match &original {
        Some(b) => vec![b.clone()],
        None => vec!["--detach".into(), refs::tip("HEAD", repo).unwrap_or_default()],
    };
    let left = original.clone().unwrap_or_else(|| "HEAD".into());
    let dirty = changed_files(repo) > 0;

    let (mut stashed, mut committed) = (false, false);
    match wip.filter(|_| dirty) {
        Some(WipMode::Commit) => {
            // Staged, unstaged and untracked alike; --no-verify: a WIP snapshot shouldn't run lint hooks.
            write(&["add", "--all"], repo)?;
            write(&["commit", "--no-verify", "--quiet", "-m", wip::COMMIT_SUBJECT], repo)?;
            committed = true;
        }
        Some(mode) => {
            let message = if mode == WipMode::Shelve { wip::shelve_message(&left) } else { wip::carry_message(&left) };
            write(&["stash", "push", "--include-untracked", "-m", &message], repo)?;
            stashed = true;
        }
        None => {}
    }
    let switched = if has_local { write(&["switch", branch], repo) } else { write(&["switch", "--track", name], repo) };
    if let Err(error) = switched {
        if stashed {
            let _ = write(&["stash", "pop"], repo); // still on the original branch: clean pop
        }
        if committed {
            let _ = write(&["reset", "--quiet", "HEAD~1"], repo); // back to uncommitted
        }
        return Err(error);
    }

    let mut status = format!("On {branch}");
    if read_opt(&["rev-parse", "--abbrev-ref", "@{u}"], repo).is_some() {
        let before = refs::tip("HEAD", repo);
        if write(&["merge", "--ff-only", "@{u}"], repo).is_err() {
            status += " · not pulled (diverged or local changes)";
        } else if refs::tip("HEAD", repo) != before {
            status += " · pulled";
        }
    }

    if stashed && wip == Some(WipMode::Carry) {
        if apply_cleanly("stash@{0}", repo) {
            let _ = write(&["stash", "drop", "stash@{0}"], repo);
            status += " · changes brought along";
        } else {
            // Would conflict here: go back and put the changes where they were (same base, always clean).
            let back: Vec<&str> = std::iter::once("switch").chain(back_to.iter().map(String::as_str)).collect();
            write(&back, repo).map_err(|e| format!("Couldn't go back to {left} ({e}): your changes are safe in the stash (stash@{{0}})"))?;
            write(&["stash", "pop"], repo).map_err(|e| format!("Back on {left}, but your changes stayed in the stash (stash@{{0}}): {e}"))?;
            return Ok(format!("Couldn't bring the changes to {branch} without conflicts: back on {left}, changes untouched"));
        }
    } else {
        if stashed {
            status += &format!(" · changes of {left} stashed");
        }
        if committed {
            status += &format!(" · changes of {left} committed as WIP");
        }
        // Coming back to a branch left with a WIP commit: undo it (changes return as uncommitted), unless it was
        // pushed meanwhile: then it's real history and stays.
        let tip_subject = read_opt(&["log", "-1", "--format=%s"], repo).map(|s| s.trim_end().to_string());
        let pushed = run(&["merge-base", "--is-ancestor", "HEAD", "@{u}"], repo).is_ok();
        if tip_subject.as_deref() == Some(wip::COMMIT_SUBJECT) && !pushed && changed_files(repo) == 0 {
            write(&["reset", "--quiet", "HEAD~1"], repo)?;
            status += " · restored its WIP commit as uncommitted changes";
        }
        if let Some(reference) = wip::shelved_stash(branch, repo).filter(|_| changed_files(repo) == 0) {
            if apply_cleanly(&reference, repo) {
                let _ = write(&["stash", "drop", &reference], repo);
                status += " · restored its saved changes";
            } else {
                status += " · its saved changes conflict now, kept in the stash";
            }
        }
    }
    Ok(if fetched { status } else { status + " · fetch failed, used local refs" })
}

// MARK: Branches

/// Where a branch is and whether deleting it can lose work: the delete dialog's options.
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Deletion {
    local: bool,
    remote: bool,      // on origin
    current: bool,     // checked out: deleting switches to the base first
    dirty: bool,       // checked out with uncommitted changes: that switch is refused
    missing: Vec<String>, // targets (base, environments) its work isn't in yet: then nothing can be deleted
    target: bool,      // it's the base or an environment itself: never deleted from here
    fetched: bool,     // false: offline, judged on what we last fetched
}

/// Fetches, then checks `branch` against the base and every target (merged, fast-forwarded, cherry-picked or squashed
/// all count: `branches::status` decides, same as the "Safe to delete" label).
pub fn deletion(branch: &str, base: &str, targets: &[String], repo: &Path) -> Deletion {
    exclusive(repo, || {
        let fetched = fetch(repo).is_ok();
        Deletion { fetched, ..check_deletion(branch, base, targets, repo) }
    })
}

fn check_deletion(branch: &str, base: &str, targets: &[String], repo: &Path) -> Deletion {
    let exists = |full: &str| run(&["rev-parse", "--verify", "-q", full], repo).is_ok();
    let local = exists(&format!("refs/heads/{branch}"));
    let remote = exists(&format!("refs/remotes/{DEFAULT_REMOTE}/{branch}"));
    let current = current_branch(repo).as_deref() == Some(branch);
    let reference = if local { branch.to_string() } else { format!("{DEFAULT_REMOTE}/{branch}") };
    let missing = branches::status(repo, std::slice::from_ref(&reference), base, targets)
        .into_iter()
        .flat_map(|s| s.promotion)
        .filter(|(_, count, _)| *count > 0)
        .map(|(target, _, _)| local_name(&target).to_string())
        .collect();
    let target = std::iter::once(base).chain(targets.iter().map(String::as_str)).any(|t| local_name(t) == branch);
    Deletion { local, remote, current, dirty: current && changed_files(repo) > 0, missing, target, fetched: true }
}

/// Deletes a shipped branch here and, with `remote`, on origin. Checked again right before (the dialog may be stale):
/// refused while its work is missing from a target. Squashed or cherry-picked work isn't merged as far as git knows,
/// so `-D` is what deletes it, safe because the check above already found its contents in every target.
/// The checked-out branch switches to `base` first, never with uncommitted changes.
pub fn delete_branch(branch: &str, base: &str, targets: &[String], remote: bool, repo: &Path) -> Result<String, String> {
    exclusive(repo, || {
        let check = check_deletion(branch, base, targets, repo);
        if check.target {
            return Err(format!("{branch} is the base or an environment: it isn't deleted from here"));
        }
        if !check.missing.is_empty() {
            return Err(format!("{branch} isn't in {} yet: deleting it would lose work", check.missing.join(", ")));
        }
        if check.current {
            if check.dirty {
                return Err(format!("{branch} is checked out with uncommitted changes: commit or discard them first"));
            }
            write(&["checkout", "-q", local_name(base)], repo)?;
        }
        let mut done = vec![];
        if check.local {
            write(&["branch", "-D", branch], repo)?;
            done.push("here");
        }
        if remote && check.remote {
            match write(&["push", DEFAULT_REMOTE, "--delete", branch], repo) {
                Ok(_) => done.push("on the server"),
                Err(e) if done.is_empty() => return Err(e),
                Err(e) => return Ok(format!("Deleted {branch} here, but not on the server: {}", e.lines().next().unwrap_or_default())),
            }
        }
        let switched = if check.current { format!(" (switched to {})", local_name(base)) } else { String::new() };
        Ok(format!("Deleted {branch} {}{switched}", done.join(" and ")))
    })
}

/// Brings `parent`'s new commits into `branch` by merge (or rebase). Never leaves a half-done state: on a conflict
/// it aborts and returns the conflicting files. A branch that isn't checked out is updated in a temporary worktree,
/// so the user's checkout is never touched. The checked-out branch must have no uncommitted tracked changes:
/// `merge --abort` can't always restore those, so we refuse up front.
pub fn update(branch: &str, parent: &str, rebase: bool, repo: &Path) -> Result<(), String> {
    exclusive(repo, || {
        let integrate = |dir: &Path| -> Result<(), String> {
            let outcome = if rebase { write(&["rebase", parent], dir) } else { write(&["merge", "--no-edit", parent], dir) };
            outcome.map(|_| ()).map_err(|error| {
                let conflicts = read_opt(&["diff", "--name-only", "--diff-filter=U"], dir).unwrap_or_default().trim().to_string();
                let _ = write(&[if rebase { "rebase" } else { "merge" }, "--abort"], dir);
                if conflicts.is_empty() { error.trim().to_string() } else { format!("Conflicts in:\n{conflicts}") }
            })
        };
        if current_branch(repo).as_deref() == Some(branch) {
            let status = git::status(repo)?;
            if status.files.len() > status.untracked.len() {
                return Err("You have uncommitted changes on this branch. Commit or stash them first, so nothing can get lost if the update hits a conflict.".into());
            }
            return integrate(repo);
        }
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("monti-git-update-{}-{nanos}", std::process::id()));
        let dir_str = dir.to_string_lossy().into_owned();
        write(&["worktree", "add", "--quiet", &dir_str, branch], repo)?;
        let result = integrate(&dir);
        let _ = write(&["worktree", "remove", "--force", &dir_str], repo);
        result
    })
}

/// How a local branch compares with its copy on origin. `remote` false: there's none, use the local one as is.
#[derive(Serialize)]
pub struct BaseStatus {
    pub ahead: usize,
    pub behind: usize,
    pub remote: bool,
}

/// Fetches, then compares `base` with origin/`base`: is it safe to start a branch from it?
pub fn base_status(base: &str, repo: &Path) -> BaseStatus {
    exclusive(repo, || {
        let _ = fetch(repo);
        compare_with_remote(base, repo)
    })
}

fn compare_with_remote(base: &str, repo: &Path) -> BaseStatus {
    let remote = format!("refs/remotes/{DEFAULT_REMOTE}/{base}");
    if refs::tip(&remote, repo).is_none() {
        return BaseStatus { ahead: 0, behind: 0, remote: false };
    }
    let counts = read_opt(&["rev-list", "--left-right", "--count", &format!("refs/heads/{base}...{remote}")], repo).unwrap_or_default();
    let mut n = counts.split_whitespace().map(|c| c.parse().unwrap_or(0));
    BaseStatus { ahead: n.next().unwrap_or(0), behind: n.next().unwrap_or(0), remote: true } // no local base: 0/0, starts from origin
}

/// "Start feature": brings `base` up to date with origin (fast-forward only, refuses if it diverged), creates `name`
/// from it without tracking it, and checks it out like `checkout` (`wip`: what to do with uncommitted changes).
pub fn start_feature(name: &str, base: &str, wip: Option<WipMode>, repo: &Path) -> Result<String, String> {
    exclusive(repo, || {
        run(&["check-ref-format", "--branch", name], repo).map_err(|_| format!("\"{name}\" isn't a valid branch name"))?;
        let _ = fetch(repo);
        let status = compare_with_remote(base, repo);
        if status.ahead > 0 && status.behind > 0 {
            return Err(format!("{base} diverged from origin ({} only here, {} only there): sort that out first", status.ahead, status.behind));
        }
        let remote = format!("refs/remotes/{DEFAULT_REMOTE}/{base}");
        if status.behind > 0 {
            // Up to origin's copy even if it doesn't track it; never a merge.
            fast_forward(base, &remote, current_branch(repo).as_deref() == Some(base), repo)
                .map_err(|_| format!("Couldn't bring {base} up to date (local changes in the way, or checked out elsewhere)"))?;
        }
        let start = if refs::tip(&format!("refs/heads/{base}"), repo).is_some() { format!("refs/heads/{base}") } else { remote };
        write(&["branch", "--no-track", name, &start], repo)?;
        match switch_to(name, wip, false, repo) {
            Ok(status) => Ok(format!("Created {name} from {base} · {status}")),
            Err(error) => {
                let _ = write(&["branch", "-D", name], repo); // brand new and never checked out: nothing to lose
                Err(error)
            }
        }
    })
}

/// Undo for the activity panel: puts `branch` back at `target`. `expected` None recreates a deleted branch;
/// otherwise the branch must still be at `expected` (nothing happened since) and that state must not be on any
/// remote, so undoing never rewrites shared history. The checked-out branch moves with `reset --soft`: the
/// working tree stays and an undone commit's changes come back staged.
pub fn move_branch(branch: &str, expected: Option<&str>, target: &str, repo: &Path) -> Result<(), String> {
    exclusive(repo, || {
        let Some(expected) = expected else {
            write(&["branch", branch, target], repo)?;
            return Ok(());
        };
        if refs::tip(&format!("refs/heads/{branch}"), repo).as_deref() != Some(expected) {
            return Err(format!("{branch} changed since then: nothing was undone"));
        }
        if !read_opt(&["branch", "-r", "--contains", expected], repo).unwrap_or_default().trim().is_empty() {
            return Err("Already pushed: undoing it would rewrite shared history".into());
        }
        if current_branch(repo).as_deref() == Some(branch) {
            write(&["reset", "--soft", target], repo)?;
        } else {
            write(&["update-ref", "-m", "monti-git: undo", &format!("refs/heads/{branch}"), target, expected], repo)?;
        }
        Ok(())
    })
}

/// What a push did: `Behind` = refused, the server has commits this branch doesn't (pull first, or force).
#[derive(Serialize, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub enum Pushed {
    Done,
    Behind,
}

pub fn push(branch: &str, force: bool, repo: &Path) -> Result<Pushed, String> {
    exclusive(repo, || push_branch(branch, force, repo))
}

/// Pushes `branch` to the branch of the same name on its remote (origin the first time), and makes that its upstream.
/// Never to a differently named upstream: a feature created from origin/main tracks main, and must not land there.
/// `force` uses --force-with-lease: git still refuses if the server has commits we haven't fetched, so nobody's work is lost.
fn push_branch(branch: &str, force: bool, repo: &Path) -> Result<Pushed, String> {
    let config = |key: &str| read_opt(&["config", &format!("branch.{branch}.{key}")], repo).map(|s| s.trim().to_string());
    let target = format!("refs/heads/{branch}");
    let remote = config("remote").filter(|_| config("merge").as_deref() == Some(target.as_str()));
    let lease = if force { "--force-with-lease" } else { "--no-force-with-lease" };
    let pushed = write(&["push", "-u", lease, remote.as_deref().unwrap_or(DEFAULT_REMOTE), &format!("{target}:{target}")], repo);
    match pushed {
        Ok(_) => Ok(Pushed::Done),
        Err(e) if e.contains("[rejected]") || e.contains("non-fast-forward") || e.contains("stale info") => Ok(Pushed::Behind),
        Err(e) => Err(e),
    }
}

// MARK: Working tree

/// Throws away uncommitted changes of one file: back to HEAD, or deleted if HEAD doesn't have it.
/// shortcut: a staged rename only reverts its new path, upgrade if renames need discarding.
pub fn discard(path: &str, repo: &Path) -> Result<(), String> {
    exclusive(repo, || discard_file(path, repo))
}

fn discard_file(path: &str, repo: &Path) -> Result<(), String> {
    if run(&["cat-file", "-e", &format!("HEAD:{path}")], repo).is_ok() {
        write(&["checkout", "HEAD", "--", path], repo)?;
    } else if run(&["ls-files", "--error-unmatch", "--", path], repo).is_ok() {
        write(&["rm", "-f", "--", path], repo)?; // staged new file
    } else {
        std::fs::remove_file(git::repo_file(repo, path)?).map_err(|e| e.to_string())?; // untracked
    }
    Ok(())
}

/// Working tree and index back to HEAD, untracked files deleted (ignored ones stay).
pub fn discard_all(repo: &Path) -> Result<(), String> {
    exclusive(repo, || {
        write(&["reset", "--hard", "HEAD"], repo)?;
        write(&["clean", "-fdq"], repo)?;
        Ok(())
    })
}

/// Reverts one block of `path`'s uncommitted diff, the one the diff pane showed as block `index` with `header`
/// ("@@ -3,2 +3,4 @@"). If the file changed since and that block isn't there anymore, nothing is touched.
/// A file git doesn't know yet is a single block: the whole file goes.
pub fn discard_hunk(path: &str, index: usize, header: &str, repo: &Path) -> Result<(), String> {
    exclusive(repo, || {
        let text = git::patch(git::Span::Worktree, &[path], 3, repo)?;
        let Some(file) = patch::parse(&text).into_iter().next() else {
            return if index == 0 { discard_file(path, repo) } else { Err(changed_since(path)) };
        };
        let hunk = file.hunks.get(index).filter(|h| h.header == header).ok_or_else(|| changed_since(path))?;
        let patch: String = file.header.iter().map(|l| format!("{l}\n")).chain([hunk.text()]).collect();
        // Index and working tree agree on this file: revert it in both; otherwise only in the working tree.
        let in_sync = run(&["diff", "--quiet", "--", path], repo).is_ok();
        let args: &[&str] = if in_sync { &["apply", "-R", "--index"] } else { &["apply", "-R"] };
        write_input(args, repo, &patch).map(|_| ())
    })
}

fn changed_since(path: &str) -> String {
    format!("{path} changed since it was shown: nothing was discarded. Look at it again and retry.")
}

/// Adds files to the index (new, modified or deleted alike).
pub fn stage(paths: &[String], repo: &Path) -> Result<(), String> {
    let args: Vec<&str> = ["add", "-A", "--"].into_iter().chain(paths.iter().map(String::as_str)).collect();
    exclusive(repo, || write(&args, repo).map(|_| ()))
}

/// Takes files out of the index; their changes stay in the working tree.
pub fn unstage(paths: &[String], repo: &Path) -> Result<(), String> {
    let args: Vec<&str> = ["reset", "-q", "--"].into_iter().chain(paths.iter().map(String::as_str)).collect();
    exclusive(repo, || write(&args, repo).map(|_| ()))
}

/// Commits what's staged. With `push`, pushes the current branch too, setting its upstream the first time.
pub fn commit(message: &str, push: bool, repo: &Path) -> Result<String, String> {
    exclusive(repo, || {
        let branch = current_branch(repo);
        if push && branch.is_none() {
            return Err("No branch checked out (detached HEAD): nothing to push to. Commit without pushing.".into());
        }
        write_input(&["commit", "-q", "-F", "-"], repo, message)?;
        let Some(branch) = branch.filter(|_| push) else { return Ok("Committed".into()) };
        // The commit is done either way: a failed push is reported as a status, not as an error that keeps the form filled.
        Ok(match push_branch(&branch, false, repo) {
            Ok(Pushed::Done) => format!("Committed and pushed {branch}"),
            Ok(Pushed::Behind) => format!("Committed, but not pushed: the server has commits {branch} doesn't. Pull first."),
            Err(e) => format!("Committed, but the push failed: {}", e.lines().next().unwrap_or("unknown error")),
        })
    })
}

#[cfg(test)]
mod tests {
    use crate::testutil::Repo;

    #[test]
    fn undo_moves_back_only_unpushed_unchanged_branches() {
        let repo = Repo::new("undo");
        repo.commit("a.txt", "1", "one");
        let one = repo.git(&["rev-parse", "HEAD"]).trim().to_string();
        repo.commit("a.txt", "2", "two");
        let two = repo.git(&["rev-parse", "HEAD"]).trim().to_string();
        assert!(super::move_branch("main", Some(&one), &one, repo.path()).is_err()); // main isn't at `one` anymore
        super::move_branch("main", Some(&two), &one, repo.path()).unwrap();
        assert_eq!(repo.git(&["rev-parse", "HEAD"]).trim(), one);
        assert_eq!(repo.git(&["diff", "--cached", "--name-only"]).trim(), "a.txt"); // the undone commit's change, staged
        super::move_branch("gone", None, &two, repo.path()).unwrap();
        assert_eq!(repo.git(&["rev-parse", "gone"]).trim(), two);
    }

    #[test]
    fn delete_checks_every_target_and_takes_squashed_work() {
        let repo = Repo::new("delete");
        repo.commit("a.txt", "1\n", "init");
        let targets = ["main".to_string()];
        repo.git(&["checkout", "-q", "-b", "feat/squashed"]);
        repo.commit("b.txt", "1\n", "b");
        repo.git(&["checkout", "-q", "main"]);
        repo.git(&["merge", "-q", "--squash", "feat/squashed"]);
        repo.git(&["commit", "-q", "-m", "squashed b"]);
        repo.git(&["checkout", "-q", "-b", "feat/open"]);
        repo.commit("c.txt", "1\n", "c, not in main");
        // Checked out and not merged: refused, nothing moves.
        assert!(super::delete_branch("feat/open", "main", &targets, false, repo.path()).unwrap_err().contains("isn't in main"));
        assert_eq!(repo.git(&["branch", "--show-current"]).trim(), "feat/open");
        assert!(super::delete_branch("main", "main", &targets, false, repo.path()).is_err());
        // Squashed: git's -d would refuse; the contents are in main, so it goes.
        let status = super::delete_branch("feat/squashed", "main", &targets, false, repo.path()).unwrap();
        assert_eq!(status, "Deleted feat/squashed here");
        assert!(repo.git(&["branch", "--list", "feat/squashed"]).trim().is_empty());
        // Checked out and shipped: switches to the base first.
        repo.git(&["checkout", "-q", "main"]);
        repo.git(&["merge", "-q", "feat/open"]);
        repo.git(&["checkout", "-q", "feat/open"]);
        let status = super::delete_branch("feat/open", "main", &targets, false, repo.path()).unwrap();
        assert_eq!(status, "Deleted feat/open here (switched to main)");
        assert_eq!(repo.git(&["branch", "--show-current"]).trim(), "main");
    }

    #[test]
    fn push_sets_upstream_then_refuses_to_overwrite_the_server() {
        use super::Pushed;
        let server = Repo::new("push-server");
        server.commit("a.txt", "1", "one");
        let repo = Repo::new("push-local");
        repo.git(&["remote", "add", "origin", &server.path().display().to_string()]);
        repo.git(&["fetch", "-q", "origin"]);
        repo.git(&["checkout", "-q", "-b", "feat/x", "origin/main"]);
        repo.commit("b.txt", "1", "mine");
        assert_eq!(super::push("feat/x", false, repo.path()).unwrap(), Pushed::Done); // first time: upstream set
        assert_eq!(repo.git(&["rev-parse", "--abbrev-ref", "feat/x@{u}"]).trim(), "origin/feat/x");
        server.git(&["checkout", "-q", "feat/x"]);
        server.commit("c.txt", "1", "theirs");
        server.git(&["checkout", "-q", "main"]);
        repo.commit("b.txt", "2", "mine again");
        assert_eq!(super::push("feat/x", false, repo.path()).unwrap(), Pushed::Behind);
        assert_eq!(super::push("feat/x", true, repo.path()).unwrap(), Pushed::Behind); // "theirs" not fetched: the lease holds
        repo.git(&["fetch", "-q", "origin"]);
        assert_eq!(super::push("feat/x", true, repo.path()).unwrap(), Pushed::Done);
        assert_eq!(server.git(&["rev-parse", "feat/x"]), repo.git(&["rev-parse", "feat/x"]));
    }

    #[test]
    fn start_feature_updates_the_base_first() {
        let server = Repo::new("start-server");
        server.commit("a.txt", "1", "one");
        let repo = Repo::new("start-local");
        repo.git(&["remote", "add", "origin", &server.path().display().to_string()]);
        repo.git(&["fetch", "-q", "origin"]);
        repo.git(&["reset", "-q", "--hard", "origin/main"]);
        server.commit("a.txt", "2", "two"); // local main is now 1 behind
        let status = super::start_feature("feat/x", "main", None, repo.path()).unwrap();
        assert!(status.starts_with("Created feat/x from main"), "{status}");
        let tip = server.git(&["rev-parse", "HEAD"]);
        assert_eq!(repo.git(&["rev-parse", "main"]), tip);
        assert_eq!(repo.git(&["rev-parse", "HEAD"]), tip);
        assert_eq!(repo.git(&["branch", "--show-current"]).trim(), "feat/x");
        assert!(super::start_feature("bad name", "main", None, repo.path()).is_err());
    }

    #[test]
    fn discard_hunk_reverts_only_the_block_shown() {
        let repo = Repo::new("discard-hunk");
        let lines: String = (1..=20).map(|n| format!("{n}\n")).collect();
        repo.commit("a.txt", &lines, "init");
        std::fs::write(repo.path().join("a.txt"), lines.replace("2\n", "two\n").replace("19\n", "nineteen\n")).unwrap();
        assert!(super::discard_hunk("a.txt", 1, "@@ -1,5 +1,5 @@", repo.path()).is_err(), "wrong header: untouched");
        let text = crate::git::patch(crate::git::Span::Worktree, &["a.txt"], 3, repo.path()).unwrap();
        let header = crate::patch::parse(&text)[0].hunks[1].header.to_string();
        super::discard_hunk("a.txt", 1, &header, repo.path()).unwrap();
        let now = std::fs::read_to_string(repo.path().join("a.txt")).unwrap();
        assert!(now.contains("two\n") && now.contains("\n19\n"));
    }

    #[test]
    fn carry_brings_changes_and_shelve_restores_them() {
        let repo = Repo::new("wip-modes");
        repo.commit("a.txt", "1", "init");
        repo.git(&["branch", "other"]);
        std::fs::write(repo.path().join("a.txt"), "2").unwrap();
        super::checkout("other", Some(super::WipMode::Shelve), repo.path()).unwrap();
        assert_eq!(std::fs::read_to_string(repo.path().join("a.txt")).unwrap(), "1");
        let back = super::checkout("main", None, repo.path()).unwrap();
        assert!(back.contains("restored its saved changes"), "{back}");
        assert_eq!(std::fs::read_to_string(repo.path().join("a.txt")).unwrap(), "2");
        super::checkout("other", Some(super::WipMode::Carry), repo.path()).unwrap();
        assert_eq!(repo.git(&["branch", "--show-current"]).trim(), "other");
        assert_eq!(std::fs::read_to_string(repo.path().join("a.txt")).unwrap(), "2");
    }
}
