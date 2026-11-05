//! Who last touched each line of a file: `git blame --porcelain` (each commit's details once, then only its hash).
use crate::git::run;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Serialize)]
pub struct Blame {
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

/// As of `rev` (None = the working tree, uncommitted lines included: their hash is all zeros).
pub fn blame(path: &str, rev: Option<&str>, repo: &Path) -> Result<Blame, String> {
    let mut args = vec!["blame", "--porcelain"];
    args.extend(rev);
    args.extend(["--", path]);
    let out = run(&args, repo)?;
    let mut blame = Blame { lines: vec![], commits: vec![] };
    let mut index: HashMap<&str, usize> = HashMap::new();
    let mut current = 0;
    for line in out.lines() {
        if line.starts_with('\t') {
            blame.lines.push(current); // the line's text ends its record
            continue;
        }
        let (key, value) = line.split_once(' ').unwrap_or((line, ""));
        match key {
            "author" => blame.commits[current].author = value.into(),
            "author-time" => blame.commits[current].date = value.parse().unwrap_or_default(),
            "summary" => blame.commits[current].subject = value.into(),
            // "<40 hex> <orig line> <final line> [<group size>]": the record of a line starts here.
            id if id.len() == 40 && id.bytes().all(|b| b.is_ascii_hexdigit()) => {
                current = *index.entry(id).or_insert_with(|| {
                    blame.commits.push(BlameCommit { id: id.into(), author: String::new(), date: 0, subject: String::new() });
                    blame.commits.len() - 1
                });
            }
            _ => {}
        }
    }
    Ok(blame)
}

#[cfg(test)]
mod tests {
    use crate::testutil::Repo;

    #[test]
    fn blames_each_line_and_survives_non_ascii() {
        let repo = Repo::new("blame");
        repo.commit("a.txt", "uno\n", "init");
        repo.commit("a.txt", "uno\ndós ñ\n", "año");
        let blame = super::blame("a.txt", None, repo.path()).unwrap();
        assert_eq!(blame.lines.len(), 2);
        assert_eq!(blame.commits[blame.lines[1]].subject, "año");
        assert_ne!(blame.lines[0], blame.lines[1]);
    }
}
