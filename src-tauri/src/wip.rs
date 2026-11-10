//! How Monti Git marks the work it puts aside when leaving a branch: a stash named after the branch (Shelve),
//! a stash it pops right away (Carry), or a commit with a fixed subject (Commit).
use crate::stash;
use std::path::Path;

pub const COMMIT_SUBJECT: &str = "WIP [monti-git]";
const SHELVE_PREFIX: &str = "monti-git: ";

pub fn shelve_message(branch: &str) -> String {
    format!("{SHELVE_PREFIX}{branch}")
}

pub fn carry_message(branch: &str) -> String {
    format!("monti-git-carry: {branch}")
}

/// Branch of a stash message ("On <branch>: monti-git: <branch>"), if it is one of ours.
pub fn shelved_branch(message: &str) -> Option<&str> {
    message.split_once(&format!(": {SHELVE_PREFIX}")).map(|(_, branch)| branch)
}

/// The stash saved for `branch` when leaving it with Shelve, e.g. "stash@{2}".
pub fn shelved_stash(branch: &str, repo: &Path) -> Option<String> {
    stash::list(repo).into_iter().find(|s| shelved_branch(&s.message) == Some(branch)).map(|s| s.reference)
}
