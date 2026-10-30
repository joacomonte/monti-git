//! Lane layout of the commit graph, ported 1:1 from GraphLayout.swift.
use crate::git::Commit;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// A lane is a vertical track waiting for a specific commit to appear further down.
#[derive(Clone, PartialEq, Debug)]
pub struct Lane {
    pub hash: String,
    pub color: usize,
    pub behind: bool,  // main lane above its tip: other branches are ahead of main
    pub current: bool, // checked-out branch: drawn thicker
}

/// Lane state entering (`before`) and leaving (`after`) one commit row. Lanes never shift sideways.
#[derive(Clone, Debug)]
pub struct GraphRow {
    pub column: usize,
    pub color: usize,
    pub before: Vec<Option<Lane>>,
    pub after: Vec<Option<Lane>>,
    pub is_current: bool, // commit on the checked-out branch: its link to the first parent is thicker
}

/// What the web view draws: lanes as `[target, color, flags]` (flags: 1 behind, 2 current), so the IPC payload carries
/// small numbers instead of a 40-char hash per lane per row. `target` is the commit's index, or a negative id for a
/// commit past the loaded page (same id for the same hash, so parents still match their lanes).
#[derive(Serialize)]
pub struct WireRow {
    pub column: usize,
    pub color: usize,
    pub before: Vec<Option<(i64, usize, u8)>>,
    pub after: Vec<Option<(i64, usize, u8)>>,
    #[serde(rename = "isCurrent")]
    pub is_current: bool,
    pub parents: Vec<i64>, // same ids as `target`
}

pub fn to_wire<'a>(rows: &'a [GraphRow], commits: &'a [Commit]) -> Vec<WireRow> {
    let mut index: HashMap<&'a str, i64> = commits.iter().enumerate().map(|(i, c)| (c.id.as_str(), i as i64)).collect();
    let mut id = |hash: &'a str| -> i64 {
        let next = -(index.len() as i64) - 1; // unique and negative: never collides with a real index
        *index.entry(hash).or_insert(next)
    };
    let lanes = |lanes: &'a [Option<Lane>], id: &mut dyn FnMut(&'a str) -> i64| -> Vec<Option<(i64, usize, u8)>> {
        lanes.iter().map(|l| l.as_ref().map(|l| (id(&l.hash), l.color, l.behind as u8 | (l.current as u8) << 1))).collect()
    };
    rows.iter()
        .zip(commits)
        .map(|(r, c)| WireRow {
            column: r.column,
            color: r.color,
            before: lanes(&r.before, &mut id),
            after: lanes(&r.after, &mut id),
            is_current: r.is_current,
            parents: c.parents.iter().map(|p| id(p)).collect(),
        })
        .collect()
}

/// `main_chain`: first-parent history of the main branch. Those commits always sit in lane 0 (leftmost)
/// and no other branch may use that lane.
pub fn compute(commits: &[Commit], main_chain: &HashSet<String>) -> Vec<GraphRow> {
    let reserved = if main_chain.is_empty() { 0 } else { 1 };
    let mut lanes: Vec<Option<Lane>> = vec![None; reserved];
    // Main lane runs from the very top down to main's tip, so it reads as "behind".
    if let Some(tip) = commits.iter().position(|c| main_chain.contains(&c.id)) {
        if tip > 0 {
            lanes[0] = Some(Lane { hash: commits[tip].id.clone(), color: 0, behind: true, current: false });
        }
    }
    // Checked-out branch: first-parent walk from HEAD until it reaches main.
    let by_id: HashMap<&str, &Commit> = commits.iter().rev().map(|c| (c.id.as_str(), c)).collect();
    let mut head_chain: HashSet<String> = HashSet::new();
    let mut cursor = commits.iter().find(|c| c.refs.iter().any(|r| r == "HEAD" || r.starts_with("HEAD -> ")));
    while let Some(commit) = cursor {
        if main_chain.contains(&commit.id) || !head_chain.insert(commit.id.clone()) {
            break;
        }
        cursor = commit.parents.first().and_then(|p| by_id.get(p.as_str()).copied());
    }

    let mut next_color = reserved; // color 0 belongs to the main branch
    let mut rows = Vec::with_capacity(commits.len());
    let free_slot = |lanes: &mut Vec<Option<Lane>>| -> usize {
        if let Some(i) = (reserved..lanes.len()).find(|&i| lanes[i].is_none()) {
            return i;
        }
        lanes.push(None);
        lanes.len() - 1
    };
    let tracks = |lanes: &[Option<Lane>], hash: &str| lanes.iter().any(|l| l.as_ref().is_some_and(|l| l.hash == hash));

    for commit in commits {
        let before = lanes.clone();
        let is_main = main_chain.contains(&commit.id);
        let column;
        if is_main {
            column = 0;
            if lanes[0].as_ref().map(|l| &l.hash) != Some(&commit.id) {
                lanes[0] = Some(Lane { hash: commit.id.clone(), color: 0, behind: false, current: false }); // main tip
            }
        } else if let Some(i) = lanes.iter().position(|l| l.as_ref().is_some_and(|l| l.hash == commit.id)) {
            column = i;
        } else {
            // Branch tip: nothing above points to it.
            column = free_slot(&mut lanes);
            lanes[column] = Some(Lane { hash: commit.id.clone(), color: next_color, behind: false, current: false });
            next_color += 1;
        }
        let color = lanes[column].as_ref().unwrap().color;

        // Other lanes that were also waiting for this commit end here (branches merging in).
        for i in 0..lanes.len() {
            if i != column && lanes[i].as_ref().is_some_and(|l| l.hash == commit.id) {
                lanes[i] = None;
            }
        }

        // First parent continues this lane, unless another lane already waits for it. The main lane always continues.
        match commit.parents.first() {
            Some(first) if is_main || !tracks(&lanes, first) => {
                lanes[column] = Some(Lane { hash: first.clone(), color, behind: false, current: head_chain.contains(&commit.id) });
            }
            _ => lanes[column] = None,
        }

        // Extra parents (merge commits) get a new lane if nobody tracks them yet.
        for parent in commit.parents.iter().skip(1) {
            if !tracks(&lanes, parent) {
                let i = free_slot(&mut lanes);
                lanes[i] = Some(Lane { hash: parent.clone(), color: next_color, behind: false, current: false });
                next_color += 1;
            }
        }

        while lanes.len() > reserved && lanes.last() == Some(&None) {
            lanes.pop();
        }
        rows.push(GraphRow { column, color, before, after: lanes.clone(), is_current: head_chain.contains(&commit.id) });
    }
    rows
}

/// "origin/x" / "upstream/x" -> "x".
fn strip_remote(name: &str) -> &str {
    name.split_once('/').filter(|(r, _)| *r == "origin" || *r == "upstream").map_or(name, |(_, rest)| rest)
}

/// Branch named in a merge subject: "Merge branch 'x'", "Merge remote-tracking branch 'x'", "Merged in x",
/// "Merge pull request #N from user/x".
fn merged_branch(subject: &str) -> Option<&str> {
    for prefix in ["Merge branch '", "Merge remote-tracking branch '"] {
        if let Some(rest) = subject.find(prefix).map(|i| &subject[i + prefix.len()..]) {
            return rest.split_once('\'').map(|(name, _)| name);
        }
    }
    if let Some(rest) = subject.find("Merged in ").map(|i| &subject[i + 10..]) {
        return rest.split_whitespace().next();
    }
    let rest = &subject[subject.find("Merge pull request #")? + 20..];
    let rest = rest.trim_start_matches(|c: char| c.is_ascii_digit()).strip_prefix(" from ")?;
    rest.split_whitespace().next()?.split_once('/').map(|(_, branch)| branch)
}

/// Which branch each commit belongs to, for the tooltip on a graph line. Main claims its first-parent history
/// first; then every branch tip claims its own first-parent commits down to an already named one; last, a merge
/// names the side it brought in from its message, for deleted branches.
pub fn branch_names(commits: &[Commit], main: Option<&str>, main_chain: &HashSet<String>) -> HashMap<String, String> {
    let by_id: HashMap<&str, &Commit> = commits.iter().rev().map(|c| (c.id.as_str(), c)).collect();
    let mut names: HashMap<String, String> = HashMap::new();
    let claim = |names: &mut HashMap<String, String>, from: &str, name: &str| {
        let mut id = Some(from.to_string());
        while let Some(current) = id {
            let Some(commit) = by_id.get(current.as_str()) else { break };
            if names.contains_key(&current) {
                break;
            }
            names.insert(current, name.to_string());
            id = commit.parents.first().cloned();
        }
    };
    if let Some(main) = main.map(strip_remote) {
        for id in main_chain.iter().filter(|id| by_id.contains_key(id.as_str())) {
            names.insert(id.clone(), main.to_string());
        }
    }
    for commit in commits {
        if commit.is_worktree() {
            names.insert(commit.id.clone(), "uncommitted changes".into());
            continue;
        }
        if let Some(branch) = &commit.shelved {
            names.insert(commit.id.clone(), format!("WIP saved from {branch}"));
            continue;
        }
        let branches: Vec<&str> =
            commit.refs.iter().filter(|r| !r.starts_with("tag: ") && *r != "HEAD").map(|r| r.trim_start_matches("HEAD -> ")).collect();
        // Prefer the local name; a remote-only branch shows without "origin/".
        let local = branches.iter().find(|b| strip_remote(b) == **b);
        if let Some(name) = local.copied().or_else(|| branches.first().map(|b| b.split_once('/').map_or(*b, |(_, r)| r))) {
            claim(&mut names, &commit.id, name);
        }
    }
    for commit in commits.iter().filter(|c| c.parents.len() > 1) {
        if names.contains_key(&commit.parents[1]) {
            continue;
        }
        if let Some(name) = merged_branch(&commit.subject) {
            claim(&mut names, &commit.parents[1], &format!("{} (merged)", strip_remote(name)));
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(id: &str, parents: &[&str], refs: &[&str], subject: &str) -> Commit {
        Commit {
            id: id.into(),
            parents: parents.iter().map(|s| s.to_string()).collect(),
            author: "a".into(),
            date: 0,
            refs: refs.iter().map(|s| s.to_string()).collect(),
            subject: subject.into(),
            shelved: None,
        }
    }

    #[test]
    fn layout_and_names() {
        // main: m2 <- m1 ; feature f1 off m1 merged into m2? No: m2 merges deleted branch d1.
        let commits = vec![
            c("f1", &["m2"], &["HEAD -> feature"], "work"),
            c("m2", &["m1", "d1"], &["main", "origin/main"], "Merge branch 'old-fix'"),
            c("d1", &["m1"], &[], "fix"),
            c("m1", &[], &[], "init"),
        ];
        let chain: HashSet<String> = ["m2", "m1"].iter().map(|s| s.to_string()).collect();
        let rows = compute(&commits, &chain);
        assert_eq!(rows[0].column, 1); // feature beside main
        assert_eq!(rows[1].column, 0);
        assert_eq!(rows[2].column, 1); // merged side gets its own lane
        assert!(rows[0].is_current);
        let names = branch_names(&commits, Some("origin/main"), &chain);
        assert_eq!(names["f1"], "feature");
        assert_eq!(names["m2"], "main");
        assert_eq!(names["d1"], "old-fix (merged)");
        assert_eq!(merged_branch("Merge pull request #12 from joaco/feat/x"), Some("feat/x"));
        assert_eq!(merged_branch("Merged in hotfix (pull request #3)"), Some("hotfix"));
    }
}
