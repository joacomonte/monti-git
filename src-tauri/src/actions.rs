//! Actions that change the repo, ported 1:1 from Git.swift. Same safety rules: fast-forward only, never
//! a half-done merge/rebase, uncommitted work always stashed or refused, never lost.
use crate::git::{run, run_input};
use serde::{Deserialize, Serialize};
use std::path::Path;

fn ok(args: &[&str], repo: &Path) -> Option<String> {
    run(args, repo).ok()
}

fn current_branch(repo: &Path) -> Option<String> {
    ok(&["symbolic-ref", "--quiet", "--short", "HEAD"], repo).map(|s| s.trim().to_string()) // None when detached
}

// shortcut: same origin/upstream assumption as RefBadge.
fn strip_remote(name: &str) -> &str {
    if name.starts_with("origin/") || name.starts_with("upstream/") { &name[name.find('/').unwrap() + 1..] } else { name }
}

#[derive(Serialize, Default)]
pub struct SyncResult {
    updated: Vec<String>,
    skipped: Vec<String>,
    log: Vec<String>, // raw git errors of the skipped branches, for debugging
}

/// Fetches every remote with --prune, then fast-forwards local branches that are strictly behind
/// their upstream. Never rewrites history, deletes local branches or touches uncommitted work:
/// anything that is not a clean fast-forward is skipped and reported. `--prune` only removes
/// remote-tracking refs (origin/x) whose branch was deleted on the remote.
pub fn sync(repo: &Path) -> Result<SyncResult, String> {
    run(&["fetch", "--all", "--prune"], repo)?;
    let current = current_branch(repo);
    let branches = run(&["for-each-ref", "--format=%(refname:short)%09%(upstream)%09%(upstream:track)", "refs/heads"], repo)?;
    let mut result = SyncResult::default();
    for line in branches.lines() {
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != 3 || fields[1].is_empty() || !fields[2].contains("behind") {
            continue;
        }
        let (branch, upstream) = (fields[0], fields[1]);
        if fields[2].contains("ahead") {
            result.skipped.push(format!("{branch} (diverged)"));
            result.log.push(diverged_report(branch, upstream, fields[2], repo));
            continue;
        }
        let is_current = current.as_deref() == Some(branch);
        let outcome = if is_current {
            run(&["merge", "--ff-only", "@{u}"], repo) // refuses on its own if it would overwrite local changes
        } else {
            // Local fetch without "+": fast-forward only, and git refuses branches checked out in any other worktree.
            run(&["fetch", ".", &format!("{upstream}:refs/heads/{branch}")], repo)
        };
        match outcome {
            Ok(_) => result.updated.push(branch.to_string()),
            Err(error) => {
                result.log.push(format!("[{branch}] {}", error.trim()));
                result.skipped.push(format!("{branch} ({})", if is_current { "local changes" } else { "checked out elsewhere" }));
            }
        }
    }
    Ok(result)
}

/// Deletes a shipped branch: `git branch -d` (git itself refuses if it isn't merged), then, if asked, the same
/// branch on its remote. Never the checked-out one.
pub fn delete_branch(branch: &str, remote: bool, repo: &Path) -> Result<String, String> {
    if current_branch(repo).as_deref() == Some(branch) {
        return Err(format!("{branch} is checked out: switch to another branch first"));
    }
    let origin = ok(&["config", &format!("branch.{branch}.remote")], repo).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    run(&["branch", "-d", branch], repo)?;
    if !remote {
        return Ok(format!("Deleted {branch}"));
    }
    let Some(origin) = origin else { return Ok(format!("Deleted {branch} (it had no remote)")) };
    Ok(match run(&["push", &origin, "--delete", branch], repo) {
        Ok(_) => format!("Deleted {branch} here and on {origin}"),
        Err(error) => format!("Deleted {branch} here, but not on {origin}: {}", error.lines().next().unwrap_or_default()),
    })
}

/// A prompt to paste into an AI coding agent when a branch can't be updated from its upstream on its own
/// (diverged, conflicts, or local changes in the way): the situation, both sides' commits, and what to do.
pub fn update_prompt(branch: &str, repo: &Path) -> Result<String, String> {
    let branch = strip_remote(branch);
    let upstream = run(&["rev-parse", "--abbrev-ref", &format!("{branch}@{{upstream}}")], repo)?.trim().to_string();
    let count = |range: String| ok(&["rev-list", "--count", &range], repo).and_then(|s| s.trim().parse::<u32>().ok()).unwrap_or(0);
    let (mine, theirs) = (count(format!("{upstream}..{branch}")), count(format!("{branch}..{upstream}")));
    let oneline = |range: String| ok(&["log", "--format=  %h %s (%an, %ar)", "-5", &range], repo).unwrap_or_default().trim_end().to_string();
    let conflicts = crate::git::conflicts(branch, &upstream, repo);
    let is_current = current_branch(repo).as_deref() == Some(branch);
    let changed = if is_current { ok(&["-c", "core.quotePath=false", "status", "--porcelain", "-uall"], repo).unwrap_or_default() } else { String::new() };

    let mut lines = vec![
        "I need help updating a git branch that couldn't be updated automatically. Don't push or force-push without asking me.".to_string(),
        String::new(),
        format!("Repo: {}", repo.display()),
        format!("Branch: {branch}{}", if is_current { " (checked out)" } else { " (not checked out)" }),
        format!("{upstream} has {theirs} commit(s) I don't have, and I have {mine} commit(s) it doesn't have."),
    ];
    if mine > 0 {
        lines.extend(["".into(), "My commits (newest 5):".into(), oneline(format!("{upstream}..{branch}"))]);
    }
    if theirs > 0 {
        lines.extend(["".into(), "Their commits (newest 5):".into(), oneline(format!("{branch}..{upstream}"))]);
    }
    if !conflicts.is_empty() {
        lines.extend(["".into(), "Files that conflict (detected with git merge-tree, nothing applied yet):".into()]);
        lines.extend(conflicts.iter().map(|f| format!("  {f}")));
    }
    if !changed.trim().is_empty() {
        lines.extend(["".into(), "Uncommitted changes in the working tree (git status --porcelain):".into(), changed.trim_end().to_string()]);
    }
    let pull = if mine > 0 { format!("git pull --rebase {}", upstream.replacen('/', " ", 1)) } else { "git pull --ff-only".to_string() };
    lines.extend([
        String::new(),
        "Please:".into(),
        if is_current { format!("1. Bring in the new commits ({pull}).") } else { format!("1. Check out {branch} first (keep my uncommitted work safe), then bring in the new commits ({pull}).") },
        "2. If there are conflicts, resolve each one keeping the intent of both sides, and explain what each side changed.".into(),
        "3. Run the tests for the touched files.".into(),
        "4. Show me the result and wait for my OK before pushing.".into(),
    ]);
    Ok(lines.join("\n"))
}

/// Raw details of a diverged branch for the sync log: what each side has, and commands that fix it.
fn diverged_report(branch: &str, upstream: &str, track: &str, repo: &Path) -> String {
    let remote = upstream.strip_prefix("refs/remotes/").unwrap_or(upstream);
    let oneline = |range: String| ok(&["log", "--format=  %h %an, %ar: %s", "-5", &range], repo).unwrap_or_default().trim_matches('\n').to_string();
    let path = repo.display();
    let mut lines = vec![
        format!("[{branch}] not updated: diverged from {remote} {track}. Sync only fast-forwards, so it left it as is."),
        "Only here (newest 5):".into(),
        oneline(format!("{remote}..{branch}")),
        "Only on the server (newest 5):".into(),
        oneline(format!("{branch}..{remote}")),
    ];
    if remote.split('/').skip(1).collect::<Vec<_>>().join("/") != branch {
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

/// What to do with uncommitted changes when checking out another branch (asked every time).
#[derive(Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum WipMode {
    Shelve, // stash them for the branch I'm leaving; restored when I come back to it
    Carry, // bring them along to the target branch
    Commit, // "WIP [monti-git]" commit on the branch I'm leaving; undone when I come back to it
}

pub const WIP_SUBJECT: &str = "WIP [monti-git]";

fn shelve_message(branch: &str) -> String {
    format!("monti-git: {branch}")
}

/// Staged, unstaged and untracked files.
pub fn changed_file_count(repo: &Path) -> usize {
    ok(&["status", "--porcelain"], repo).unwrap_or_default().lines().count()
}

fn is_dirty(repo: &Path) -> bool {
    changed_file_count(repo) > 0
}

/// The stash saved for `branch` when leaving it with Shelve, e.g. "stash@{2}".
fn shelved_stash(branch: &str, repo: &Path) -> Option<String> {
    // Subject of `stash push -m X` is "On <branch>: X".
    let suffix = format!(": {}", shelve_message(branch));
    let list = ok(&["stash", "list", "--format=%gd%x09%gs"], repo).unwrap_or_default();
    list.lines().find(|l| l.ends_with(&suffix)).map(|l| l.split('\t').next().unwrap().to_string())
}

/// Applies a stash onto a clean tree; on conflict undoes the half-applied result (the tree was clean
/// before, so reset + clean only removes what the apply wrote) and leaves the stash where it is.
fn apply_cleanly(reference: &str, repo: &Path) -> bool {
    if run(&["stash", "apply", reference], repo).is_ok() {
        return true;
    }
    let _ = run(&["reset", "--hard", "--quiet"], repo);
    let _ = run(&["clean", "-fd", "--quiet"], repo);
    false
}

/// Fetch, switch to the branch, then fast-forward it to its upstream. Never merges or overwrites: the pull
/// is --ff-only, so a diverged branch is switched to but left as is. Uncommitted changes are stashed first
/// (`wip`), so the switch never fails because of them, and are only re-applied when that applies cleanly:
/// otherwise they stay in the stash (or go back where they came from).
pub fn checkout(name: &str, wip: Option<WipMode>, repo: &Path) -> Result<String, String> {
    let fetched = run(&["fetch", "--all", "--prune"], repo).is_ok();
    let branch = strip_remote(name);
    let has_local = run(&["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")], repo).is_ok();
    let original = current_branch(repo).unwrap_or_else(|| "HEAD".into());

    let mut stashed = false;
    let mut committed = false;
    if wip == Some(WipMode::Commit) && is_dirty(repo) {
        // Staged, unstaged and untracked alike; --no-verify: a WIP snapshot shouldn't run lint hooks.
        run(&["add", "--all"], repo)?;
        run(&["commit", "--no-verify", "--quiet", "-m", WIP_SUBJECT], repo)?;
        committed = true;
    } else if let Some(mode) = wip.filter(|_| is_dirty(repo)) {
        let message = if mode == WipMode::Shelve { shelve_message(&original) } else { format!("monti-git-carry: {original}") };
        run(&["stash", "push", "--include-untracked", "-m", &message], repo)?;
        stashed = true;
    }
    let switched = if has_local { run(&["switch", branch], repo) } else { run(&["switch", "--track", name], repo) };
    if let Err(error) = switched {
        if stashed {
            let _ = run(&["stash", "pop"], repo); // still on the original branch: clean pop
        }
        if committed {
            let _ = run(&["reset", "--quiet", "HEAD~1"], repo); // back to uncommitted
        }
        return Err(error);
    }

    let mut status = format!("On {branch}");
    if run(&["rev-parse", "--abbrev-ref", "@{u}"], repo).is_ok() {
        let before = run(&["rev-parse", "HEAD"], repo)?;
        if run(&["merge", "--ff-only", "@{u}"], repo).is_err() {
            status += " · not pulled (diverged or local changes)";
        } else if run(&["rev-parse", "HEAD"], repo)? != before {
            status += " · pulled";
        }
    }

    if stashed && wip == Some(WipMode::Carry) {
        if apply_cleanly("stash@{0}", repo) {
            let _ = run(&["stash", "drop", "stash@{0}"], repo);
            status += " · changes brought along";
        } else {
            // Would conflict here: go back and put the changes where they were (same base, always clean).
            run(&["switch", if original == "HEAD" { "-" } else { &original }], repo)?;
            run(&["stash", "pop"], repo)?;
            return Ok(format!("Couldn't bring the changes to {branch} without conflicts: back on {original}, changes untouched"));
        }
    } else {
        if stashed {
            status += &format!(" · changes of {original} stashed");
        }
        if committed {
            status += &format!(" · changes of {original} committed as WIP");
        }
        // Coming back to a branch left with a WIP commit: undo it (changes return as uncommitted),
        // unless it was pushed meanwhile, then it's real history and stays.
        let tip = ok(&["log", "-1", "--format=%s"], repo).map(|s| s.trim_end_matches('\n').to_string());
        let pushed = run(&["merge-base", "--is-ancestor", "HEAD", "@{u}"], repo).is_ok();
        if tip.as_deref() == Some(WIP_SUBJECT) && !pushed && !is_dirty(repo) {
            run(&["reset", "--quiet", "HEAD~1"], repo)?;
            status += " · restored its WIP commit as uncommitted changes";
        }
        if let Some(reference) = shelved_stash(branch, repo).filter(|_| !is_dirty(repo)) {
            if apply_cleanly(&reference, repo) {
                let _ = run(&["stash", "drop", &reference], repo);
                status += " · restored its saved changes";
            } else {
                status += " · its saved changes conflict now, kept in the stash";
            }
        }
    }
    Ok(if fetched { status } else { status + " · fetch failed, used local refs" })
}

/// Fetch, then fast-forward one branch to its upstream without checking it out. Same rules as `sync`:
/// --ff-only for the checked-out branch, a "+"-less local fetch for the others, so nothing is ever
/// merged, rewritten or overwritten. Returns a one-line status.
pub fn pull(name: &str, repo: &Path) -> Result<String, String> {
    run(&["fetch", "--all", "--prune"], repo)?;
    let branch = strip_remote(name);
    let info = ok(&["for-each-ref", "--format=%(upstream)%09%(upstream:track)", &format!("refs/heads/{branch}")], repo).unwrap_or_default();
    let info = info.trim_end_matches('\n');
    if info.is_empty() {
        return Ok(format!("Fetched {name}")); // remote-only branch: fetching was the pull
    }
    let fields: Vec<&str> = info.split('\t').collect();
    let (upstream, track) = (fields[0], fields.get(1).copied().unwrap_or(""));
    if upstream.is_empty() {
        return Ok(format!("{branch} has no upstream"));
    }
    if track.contains("gone") {
        return Ok(format!("{branch}: upstream branch was deleted"));
    }
    if !track.contains("behind") {
        return Ok(format!("{branch} up to date"));
    }
    if track.contains("ahead") {
        return Ok(format!("{branch} diverged from upstream, not pulled"));
    }
    let is_current = current_branch(repo).as_deref() == Some(branch);
    let outcome = if is_current {
        run(&["merge", "--ff-only", "@{u}"], repo) // refuses if it would overwrite local changes
    } else {
        run(&["fetch", ".", &format!("{upstream}:refs/heads/{branch}")], repo)
    };
    Ok(match outcome {
        Ok(_) => format!("Pulled {branch}"),
        Err(_) if is_current => format!("{branch} not pulled: local changes in the way"),
        Err(_) => format!("{branch} not pulled: checked out elsewhere"),
    })
}

/// Replaces every commit of `branch` after `base` with one commit holding the same final files.
/// No checkout: the tree is identical, so the working tree, index and uncommitted changes stay as they are.
/// `expected_tip` makes it refuse if the branch moved since the dashboard loaded; the old tip stays in the reflog.
pub fn squash(branch: &str, base: &str, expected_tip: &str, message: &str, repo: &Path) -> Result<(), String> {
    let tree = run(&["rev-parse", &format!("{expected_tip}^{{tree}}")], repo)?.trim().to_string();
    let commit = run_input(&["commit-tree", &tree, "-p", base, "-F", "-"], repo, Some(message))?.trim().to_string();
    run(&["update-ref", "-m", "monti-git: squash", &format!("refs/heads/{branch}"), &commit, expected_tip], repo)?;
    Ok(())
}

/// Brings `parent`'s new commits into `branch` by merge (or rebase). Never leaves a half-done state: on a
/// conflict it aborts and returns the conflicting files. A branch that isn't checked out is updated in a
/// temporary worktree, so the user's checkout is never touched. The checked-out branch must have no
/// uncommitted tracked changes: `merge --abort` can't always restore those, so we refuse up front.
pub fn update(branch: &str, parent: &str, rebase: bool, is_current: bool, repo: &Path) -> Result<(), String> {
    let integrate = |dir: &Path| -> Result<(), String> {
        let outcome = if rebase { run(&["rebase", parent], dir) } else { run(&["merge", "--no-edit", parent], dir) };
        outcome.map(|_| ()).map_err(|error| {
            let conflicts = ok(&["diff", "--name-only", "--diff-filter=U"], dir).unwrap_or_default().trim().to_string();
            let _ = run(&[if rebase { "rebase" } else { "merge" }, "--abort"], dir);
            if conflicts.is_empty() { error.trim().to_string() } else { format!("Conflicts in:\n{conflicts}") }
        })
    };
    if is_current {
        if !ok(&["status", "--porcelain", "--untracked-files=no"], repo).unwrap_or_default().trim().is_empty() {
            return Err("You have uncommitted changes on this branch. Commit or stash them first, so nothing can get lost if the update hits a conflict.".into());
        }
        return integrate(repo);
    }
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("monti-git-update-{}-{nanos}", std::process::id()));
    let dir_str = dir.to_string_lossy().into_owned();
    run(&["worktree", "add", "--quiet", &dir_str, branch], repo)?;
    let result = integrate(&dir);
    let _ = run(&["worktree", "remove", "--force", &dir_str], repo);
    result
}

/// Pushes `branch` over its upstream with --force-with-lease: git refuses if the remote has commits
/// we haven't fetched, so someone else's work is never overwritten.
pub fn force_push(branch: &str, repo: &Path) -> Result<(), String> {
    let config = |key: &str| run(&["config", &format!("branch.{branch}.{key}")], repo).map(|s| s.trim().to_string());
    let (remote, merge) = (config("remote")?, config("merge")?);
    run(&["push", "--force-with-lease", &remote, &format!("refs/heads/{branch}:{merge}")], repo)?;
    Ok(())
}

/// Throws away uncommitted changes of one file: back to HEAD, or deleted if HEAD doesn't have it.
/// shortcut: a staged rename only reverts its new path, upgrade if renames need discarding.
pub fn discard(path: &str, repo: &Path) -> Result<(), String> {
    if run(&["cat-file", "-e", &format!("HEAD:{path}")], repo).is_ok() {
        run(&["checkout", "HEAD", "--", path], repo)?;
    } else if run(&["ls-files", "--error-unmatch", "--", path], repo).is_ok() {
        run(&["rm", "-f", "--", path], repo)?; // staged new file
    } else {
        std::fs::remove_file(repo.join(path)).map_err(|e| e.to_string())?; // untracked
    }
    Ok(())
}

/// Working tree and index back to HEAD, untracked files deleted (ignored ones stay).
pub fn discard_all(repo: &Path) -> Result<(), String> {
    run(&["reset", "--hard", "HEAD"], repo)?;
    run(&["clean", "-fdq"], repo)?;
    Ok(())
}

/// Reverts one hunk of `diff` (the `git diff HEAD` of a single file) in the working tree, and in the
/// index too when the change was staged.
pub fn discard_hunk(index: usize, diff: &str, repo: &Path) -> Result<(), String> {
    let lines: Vec<&str> = diff.split('\n').collect();
    let starts: Vec<usize> = (0..lines.len()).filter(|&i| lines[i].starts_with("@@")).collect();
    let (Some(&first), Some(&start)) = (starts.first(), starts.get(index)) else { return Ok(()) };
    let end = starts.get(index + 1).copied().unwrap_or(lines.len());
    let mut patch = [&lines[..first], &lines[start..end]].concat().join("\n");
    if !patch.ends_with('\n') {
        patch.push('\n');
    }
    if run_input(&["apply", "-R", "--index"], repo, Some(&patch)).is_err() {
        run_input(&["apply", "-R"], repo, Some(&patch))?;
    }
    Ok(())
}

/// Adds files to the index (new, modified or deleted alike).
pub fn stage(paths: &[String], repo: &Path) -> Result<(), String> {
    let args: Vec<&str> = ["add", "-A", "--"].into_iter().chain(paths.iter().map(String::as_str)).collect();
    run(&args, repo).map(|_| ())
}

/// Takes files out of the index; their changes stay in the working tree.
pub fn unstage(paths: &[String], repo: &Path) -> Result<(), String> {
    let args: Vec<&str> = ["reset", "-q", "--"].into_iter().chain(paths.iter().map(String::as_str)).collect();
    run(&args, repo).map(|_| ())
}

/// Commits what's staged. With `push`, pushes the current branch too, setting its upstream the first time.
pub fn commit(message: &str, push: bool, repo: &Path) -> Result<String, String> {
    run_input(&["commit", "-q", "-F", "-"], repo, Some(message))?;
    if !push {
        return Ok("Committed".into());
    }
    // The commit is done either way: a failed push is reported as a status, not as an error that keeps the form filled.
    let branch = run(&["symbolic-ref", "--short", "HEAD"], repo)?.trim().to_string();
    let pushed = if run(&["rev-parse", "--abbrev-ref", "@{u}"], repo).is_ok() {
        run(&["push"], repo)
    } else {
        run(&["push", "-u", "origin", &branch], repo)
    };
    Ok(match pushed {
        Ok(_) => format!("Committed and pushed {branch}"),
        Err(e) => format!("Committed, but the push failed: {}", e.lines().next().unwrap_or("unknown error")),
    })
}
