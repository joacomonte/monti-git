//! Search: a text (e.g. "MOA-854") in commit messages across repos, a line of code across repos and branches, and
//! lines inside the files of one selection (the file tree's search). Case-insensitive, the text taken literally.
use crate::git::{self, Span};
use crate::{par, patch, refs};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

const COMMITS_PER_REPO: usize = 30;
const CODE_PER_REPO: usize = 40;

/// Results, and whether some repo had more than its share (only the first ones are listed).
#[derive(Serialize)]
pub struct Found<T> {
    hits: Vec<T>,
    truncated: bool,
}

impl<T> Found<T> {
    fn merge(parts: Vec<(Vec<T>, bool)>) -> Found<T> {
        let truncated = parts.iter().any(|(_, t)| *t);
        Found { hits: parts.into_iter().flat_map(|(hits, _)| hits).collect(), truncated }
    }
}

#[derive(Serialize)]
pub struct CommitHit {
    pub repo: String,
    pub id: String,
    pub author: String,
    pub date: i64, // unix seconds
    pub subject: String,
    pub snippet: Option<String>, // the description line that matched, when the title didn't
}

/// Newest matches first per repo, every branch (local and remote).
pub fn commits(repos: &[String], query: &str) -> Found<CommitHit> {
    Found::merge(par::map(repos, |repo| commits_in_repo(repo, query)))
}

fn commits_in_repo(repo: &str, query: &str) -> (Vec<CommitHit>, bool) {
    let grep = format!("--grep={query}");
    let limit = format!("-n{}", COMMITS_PER_REPO + 1); // one more than shown: tells whether there are more
    let args = ["log", "--all", "-i", "-F", &grep, &limit, "--format=%H%x1f%an%x1f%at%x1f%s%x1f%b%x1e"];
    let Ok(out) = git::run(&args, Path::new(repo)) else { return (vec![], false) };
    let needle = query.to_lowercase();
    let mut hits: Vec<CommitHit> = out
        .split('\x1e')
        .filter_map(|record| {
            let mut f = record.trim_start_matches('\n').split('\x1f');
            let (id, author, date, subject, body) = (f.next()?, f.next()?, f.next()?, f.next()?, f.next().unwrap_or(""));
            let snippet = (!subject.to_lowercase().contains(&needle))
                .then(|| body.lines().find(|l| l.to_lowercase().contains(&needle)).map(|l| l.trim().to_string()))
                .flatten();
            (!id.is_empty()).then(|| CommitHit { repo: repo.into(), id: id.into(), author: author.into(), date: date.parse().unwrap_or(0), subject: subject.into(), snippet })
        })
        .collect();
    let truncated = hits.len() > COMMITS_PER_REPO;
    hits.truncate(COMMITS_PER_REPO);
    (hits, truncated)
}

/// One `git grep` match: "rev:path\0line\0text" with `-z` (no "rev:" for the working tree), so any path parses.
struct GrepLine<'a> {
    rev: Option<&'a str>,
    path: &'a str,
    line: u32,
    text: &'a str,
}

/// `git grep` for `query` in `revs` (none = the working tree, untracked files included), limited to `paths`.
/// Exits 1 when nothing matches: no results, not an error.
fn grep(repo: &Path, query: &str, revs: &[&str], paths: &[&str], each: &mut dyn FnMut(GrepLine)) {
    let mut args = vec!["grep", "-z", "-n", "-I", "-i", "-F", "-e", query];
    if revs.is_empty() {
        args.push("--untracked");
    }
    args.extend(revs);
    args.push("--");
    args.extend(paths);
    let out = git::read_opt(&args, repo).unwrap_or_default();
    for record in out.lines() {
        let mut f = record.splitn(3, '\0');
        let (Some(name), Some(line), Some(text)) = (f.next(), f.next(), f.next()) else { continue };
        let Ok(line) = line.parse() else { continue };
        // "rev:path": revs are hashes or branch names, which never hold a ':' the way paths can.
        let (rev, path) = match revs.iter().find(|r| name.starts_with(&format!("{r}:"))) {
            Some(rev) => (Some(*rev), &name[rev.len() + 1..]),
            None => (None, name),
        };
        each(GrepLine { rev, path, line, text });
    }
}

/// A line of code that contains the text, in one or more branches of a repo.
#[derive(Serialize)]
pub struct CodeHit {
    pub repo: String,
    pub id: String,            // commit the line was found at (the first branch's tip)
    pub branches: Vec<String>, // where it is: the current branch, the base, or every branch that has this line
    pub targets: Vec<String>,  // those of `branches` where work ends up (refs::is_target)
    pub path: String,
    pub line: u32,
    pub text: String,
}

/// Where the code search looks in each repo.
#[derive(serde::Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Head, // the checked-out commit
    Base, // main / production / the base picked for the repo (`bases`)
    All,  // every branch tip
}

pub fn code(repos: &[String], bases: &HashMap<String, String>, query: &str, scope: Scope) -> Found<CodeHit> {
    Found::merge(par::map(repos, |repo| code_in_repo(repo, bases.get(repo).map(String::as_str), query, scope)))
}

fn code_in_repo(repo: &str, base: Option<&str>, query: &str, scope: Scope) -> (Vec<CodeHit>, bool) {
    let dir = Path::new(repo);
    // (name, commit) of every tree to search; the same commit under two names is searched once.
    let tips: Vec<(String, String)> = match scope {
        Scope::Base => refs::main_ref(dir, base).into_iter().collect(),
        Scope::All => git::read_opt(&["for-each-ref", "--format=%(refname:short)%09%(objectname)", "refs/heads", "refs/remotes"], dir)
            .unwrap_or_default()
            .lines()
            .filter_map(|l| l.split_once('\t').map(|(n, o)| (n.to_string(), o.to_string())))
            .filter(|(n, _)| !refs::is_alias(n))
            .collect(),
        Scope::Head => refs::tip("HEAD", dir).map(|id| (refs::current_branch(dir).unwrap_or_else(|| "HEAD".into()), id)).into_iter().collect(),
    };
    let mut names: HashMap<&str, Vec<String>> = HashMap::new();
    for (name, oid) in &tips {
        names.entry(oid.as_str()).or_default().push(name.clone());
    }
    if names.is_empty() {
        return (vec![], false);
    }
    let revs: Vec<&str> = names.keys().copied().collect();
    let mut hits: Vec<CodeHit> = vec![];
    let mut truncated = false;
    let mut seen: HashMap<(String, u32, String), usize> = HashMap::new();
    grep(dir, query, &revs, &[], &mut |m| {
        let branches = m.rev.and_then(|r| names.get(r)).cloned().unwrap_or_default();
        let text: String = m.text.trim().chars().take(240).collect();
        let key = (m.path.to_string(), m.line, text);
        // Same line in several branches: one hit listing all of them.
        if let Some(&at) = seen.get(&key) {
            hits[at].branches.extend(branches);
            return;
        }
        if hits.len() >= CODE_PER_REPO {
            truncated = true;
            return;
        }
        seen.insert(key.clone(), hits.len());
        hits.push(CodeHit { repo: repo.into(), id: m.rev.unwrap_or_default().into(), branches, targets: vec![], path: key.0, line: key.1, text: key.2 });
    });
    for hit in &mut hits {
        hit.targets = hit.branches.iter().filter(|b| refs::is_target(b)).cloned().collect();
    }
    (hits, truncated)
}

/// Matches of the tree search in one file: how many lines, and the first one (`old`: on the "before" side of the diff).
#[derive(Serialize)]
pub struct LineHits {
    count: u32,
    line: u32,
    old: bool,
}

/// The tree search by content in a selection: path -> its matching lines. `diff_only`: only the added and removed
/// lines (untracked files count as all added). Otherwise whole files at the selection's commit, limited to `paths`.
pub fn lines(repo: &Path, span: Span, query: &str, paths: &[&str], diff_only: bool) -> HashMap<String, LineHits> {
    let mut hits: HashMap<String, LineHits> = HashMap::new();
    let mut add = |path: &str, line: u32, old: bool| {
        hits.entry(path.to_string()).or_insert(LineHits { count: 0, line, old }).count += 1;
    };
    if !diff_only {
        let revs: Vec<&str> = span.target().into_iter().collect();
        grep(repo, query, &revs, paths, &mut |m| add(m.path, m.line, false));
        return hits;
    }
    let needle = query.to_lowercase();
    let diff = git::patch(span, paths, 0, repo).unwrap_or_default();
    for file in patch::parse(&diff) {
        for (old, number, text) in file.hunks.iter().flat_map(|h| h.changes()) {
            if text.to_lowercase().contains(&needle) {
                add(&file.path, number, old);
            }
        }
    }
    if let Span::Worktree = span {
        // Untracked files aren't in `git diff`: every line of theirs is added.
        let untracked = git::read_opt(&["ls-files", "-z", "--others", "--exclude-standard", "--"].iter().copied().chain(paths.iter().copied()).collect::<Vec<_>>(), repo);
        for path in untracked.unwrap_or_default().split('\0').filter(|p| !p.is_empty()) {
            let Some(text) = git::file_text(repo, path, 2 * 1024 * 1024) else { continue };
            for (line, number) in text.lines().zip(1..) {
                if line.to_lowercase().contains(&needle) {
                    add(path, number, false);
                }
            }
        }
    }
    hits
}

/// A branch that contains a commit, and whether work ends up there (refs::is_target).
#[derive(Serialize)]
pub struct Place {
    name: String,
    target: bool,
}

/// Branches that contain a commit (local names; a remote-only branch keeps its origin/ prefix), for "where is it".
pub fn places(repo: &str, id: &str) -> Vec<Place> {
    let Ok(out) = git::run(&["branch", "-a", "--contains", id, "--format=%(refname:short)"], Path::new(repo)) else { return vec![] };
    let names: Vec<&str> = out.lines().filter(|n| !refs::is_alias(n) && !n.starts_with('(')).collect();
    // origin/qa next to a local qa: one entry.
    names
        .iter()
        .filter(|n| refs::local_name(n) == **n || !names.contains(&refs::local_name(n)))
        .map(|n| Place { name: n.to_string(), target: refs::is_target(n) })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::git::Span;
    use crate::testutil::Repo;

    #[test]
    fn finds_tickets_in_titles_and_descriptions() {
        let repo = Repo::new("search");
        repo.commit("a.txt", "1", "feat(MOA-854): filters");
        repo.git(&["checkout", "-q", "-b", "qa"]);
        repo.git(&["commit", "-q", "--allow-empty", "-m", "fix: dates", "-m", "Part of moa-854"]);
        repo.commit("b.txt", "1", "unrelated");
        let found = super::commits(&[repo.path().display().to_string()], "MOA-854");
        assert_eq!((found.hits.len(), found.truncated), (2, false));
        // Same second, so no order between them: look each one up.
        let hit = |subject: &str| found.hits.iter().find(|h| h.subject == subject).unwrap();
        assert_eq!(hit("fix: dates").snippet.as_deref(), Some("Part of moa-854")); // matched in the description
        assert_eq!(hit("feat(MOA-854): filters").snippet, None);
        let mut places: Vec<(String, bool)> = super::places(&repo.path().display().to_string(), &hit("feat(MOA-854): filters").id).into_iter().map(|p| (p.name, p.target)).collect();
        places.sort();
        assert_eq!(places, [("main".to_string(), true), ("qa".to_string(), true)]);
    }

    #[test]
    fn finds_code_lines_per_branch() {
        let repo = Repo::new("search-code");
        repo.commit("v8:migration.sql", "-- V8 Migration\nselect 1;\n", "base");
        repo.git(&["checkout", "-q", "-b", "qa"]);
        repo.commit("app.ts", "run('v8 migration')\n", "qa");
        let path = repo.path().display().to_string();
        let none = std::collections::HashMap::new();
        let head = super::code(&[path.clone()], &none, "v8 migration", super::Scope::Head);
        assert_eq!(head.hits.len(), 2);
        let base = super::code(&[path.clone()], &none, "v8 migration", super::Scope::Base);
        assert_eq!(base.hits.len(), 1);
        let hit = &base.hits[0];
        assert_eq!((hit.path.as_str(), hit.line, hit.branches.clone()), ("v8:migration.sql", 1, vec!["main".to_string()]));
        let all = super::code(&[path], &none, "V8 MIGRATION", super::Scope::All);
        let sql = all.hits.iter().find(|h| h.path == "v8:migration.sql").unwrap();
        let mut branches = sql.branches.clone();
        branches.sort();
        assert_eq!(branches, ["main", "qa"]); // one hit, both branches
        let mut targets = sql.targets.clone();
        targets.sort();
        assert_eq!(targets, ["main", "qa"]);
        assert_eq!(all.hits.len(), 2);
    }

    #[test]
    fn tree_search_reads_diff_lines_and_untracked_files() {
        let repo = Repo::new("search-lines");
        repo.commit("q.sql", "-- keep\nselect 1;\n", "init");
        std::fs::write(repo.path().join("q.sql"), "-- gone\nselect 1;\nselect v8;\n").unwrap();
        std::fs::write(repo.path().join("new.txt"), "a\nV8 here\n").unwrap();
        let hits = super::lines(repo.path(), Span::Worktree, "v8", &[], true);
        assert_eq!((hits["q.sql"].line, hits["q.sql"].old), (3, false));
        assert_eq!(hits["new.txt"].line, 2);
        let removed = super::lines(repo.path(), Span::Worktree, "keep", &[], true);
        assert_eq!((removed["q.sql"].line, removed["q.sql"].old), (1, true));
        let whole = super::lines(repo.path(), Span::Worktree, "select", &["q.sql"], false);
        assert_eq!((whole["q.sql"].count, whole["q.sql"].line), (2, 2));
    }
}
