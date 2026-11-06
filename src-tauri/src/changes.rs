//! What a selection changed: its files (the file tree) and one file's diff (the diff pane).
use crate::diff::{self, DiffRow};
use crate::git::{self, Span};
use crate::patch;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

const UNTRACKED_MAX: u64 = 5 * 1024 * 1024; // bigger new files aren't read for line counts or display
const MAX_ROWS: usize = 5000; // diff pane rows; past that the pane says it's cut

#[derive(Serialize)]
pub struct Changes {
    changes: BTreeMap<String, char>,     // path -> status letter
    counts: HashMap<String, (u32, u32)>, // path -> lines added, removed
    paths: Vec<String>,                  // what the tree shows: changed files, or every file with `all`
    staged: Vec<String>,                 // uncommitted-changes row only: files in the index
    untracked: Vec<String>,              // uncommitted-changes row only: new files git doesn't follow yet (deleting them can't be undone)
    nested: Vec<String>,                 // the untracked ones that are a whole git repo inside this one
}

pub fn load(repo: &Path, span: Span, all: bool) -> Result<Changes, String> {
    let mut counts = git::line_counts(span, repo);
    let (changes, staged, untracked) = match span {
        Span::Worktree => {
            let status = git::status(repo)?;
            // Untracked files are not in `git diff`: count their lines.
            for path in &status.untracked {
                if let Some(text) = git::file_text(repo, path, UNTRACKED_MAX) {
                    counts.insert(path.clone(), (git::line_count(&text), 0));
                }
            }
            (status.files, status.staged, status.untracked)
        }
        _ => (git::changed_files(span, repo)?, vec![], vec![]),
    };
    let mut paths: Vec<String> = changes.keys().cloned().collect();
    if all {
        paths.extend(git::all_files(span.target(), repo));
        paths.sort();
        paths.dedup();
    }
    let nested = untracked.iter().filter(|p| git::repo_file(repo, p).is_ok_and(|f| f.join(".git").exists())).cloned().collect();
    Ok(Changes { changes, counts, paths, staged, untracked, nested })
}

#[derive(Serialize)]
pub struct FileDiff {
    rows: Vec<DiffRow>,
    truncated: bool,       // more than MAX_ROWS rows: only the first ones are here
}

/// `whole`: the file's full content, one column, no diff (it didn't change, or a searched line is outside the
/// changed blocks).
pub fn file_diff(repo: &Path, path: &str, span: Span, whole: bool) -> Result<FileDiff, String> {
    let mut rows = if whole {
        let text = match span.target() {
            Some(target) => git::run(&["show", &format!("{target}:{path}")], repo)?,
            None => git::file_text(repo, path, UNTRACKED_MAX).unwrap_or_default(),
        };
        diff::whole(&text)
    } else {
        let text = git::patch(span, &[path], 3, repo)?;
        match patch::parse(&text).into_iter().next() {
            Some(file) => diff::rows(&file.hunks),
            None if matches!(span, Span::Worktree) => git::file_text(repo, path, UNTRACKED_MAX).map(|t| diff::added(&t)).unwrap_or_default(),
            None => vec![], // binary, or a mode change only
        }
    };
    let truncated = rows.len() > MAX_ROWS;
    rows.truncate(MAX_ROWS);
    Ok(FileDiff { rows, truncated })
}

#[cfg(test)]
mod tests {
    use crate::git::Span;
    use crate::testutil::Repo;

    #[test]
    fn worktree_files_counts_and_diffs() {
        let repo = Repo::new("changes");
        repo.commit("a.txt", "1\n", "init");
        std::fs::write(repo.path().join("a.txt"), "1\n2\n").unwrap();
        std::fs::write(repo.path().join("new.txt"), "x\ny").unwrap();
        let changes = super::load(repo.path(), Span::Worktree, false).unwrap();
        assert_eq!(changes.paths, ["a.txt", "new.txt"]);
        assert_eq!((changes.counts["a.txt"], changes.counts["new.txt"]), ((1, 0), (2, 0)));
        let diff = super::file_diff(repo.path(), "new.txt", Span::Worktree, false).unwrap();
        assert_eq!(diff.rows.len(), 3); // the block header and two added lines
        let diff = super::file_diff(repo.path(), "a.txt", Span::Worktree, false).unwrap();
        assert_eq!(diff.rows.iter().filter(|r| r.is_change).count(), 1);
    }
}
