//! The graph's commit list: every branch (or only some), one branch focused on its base, or one file's history;
//! then the uncommitted row and the saved WIP rows placed above their commits, and the lanes laid out.
use crate::git::{self, Commit, LogQuery, Stat};
use crate::graph;
use crate::refs::{self, full_name};
use crate::wip;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

/// What the graph shows, as the UI asks for it.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Query {
    pub limit: usize,
    pub only_mine: bool,     // my branches plus the base and the environments
    pub branches_only: bool, // "Commits collapsed": only commits a branch or tag points at
    pub preferred: Option<String>, // the base the user picked for this repo
    pub focus: Vec<String>, // [branch] or [branch, base]: that branch's own line; wins over the rest
    pub path: Option<String>, // only the commits that touched this file
    pub environments: Option<Vec<String>>, // the user's environments; None = detected by name
    pub only: Option<Vec<String>>, // a filter's branches
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct History {
    commits: Vec<Commit>,
    rows: Vec<graph::WireRow>,
    names: Vec<Option<String>>, // branch each commit belongs to, by index, for the line tooltip
    has_more: bool,
    current: Option<String>, // checked-out branch, None when detached
    base: Option<String>,    // the branch pinned to the left (main / production / the user's pick)
    environments: Vec<String>, // qa, develop…: kept by "Only mine" and compared against on the branch cards
    all_branches: Vec<String>, // every branch name, local or on origin: the environments picker
    local: Vec<String>,       // every local branch, for "Change base"
    shelved_branches: Vec<String>, // branches with WIP the app put aside (stash): the "With WIP" filter
    fast_forwarded: bool, // focused branch reached the base by a fast-forward, so there's no merge to draw
}

pub fn load(repo: &Path, q: &Query) -> Result<History, String> {
    let (base, base_tip) = refs::main_ref(repo, q.preferred.as_deref()).unzip();
    let branches = refs::branches(repo, base.as_deref(), q.environments.as_deref());
    // A file's history has no uncommitted row: only the branch name, no status.
    let status = if q.path.is_none() { Some(git::status(repo)?) } else { None };
    let current = match &status {
        Some(s) => s.branch.clone(),
        None => refs::current_branch(repo),
    };
    let focus: Vec<String> = q.focus.iter().filter(|r| refs::tip(r, repo).is_some()).cloned().collect();
    let (mut commits, fast_forwarded) = match focus.as_slice() {
        [] => (browse(repo, q, base.as_deref(), &branches.environments, current.as_deref())?, false),
        [tip, rest @ ..] => focused(repo, tip, rest.first(), q)?,
    };
    let has_more = commits.len() >= q.limit;

    if let Some(status) = status {
        let stat = Stat { files: status.files.len() as u32, ..Default::default() }; // lines aren't shown: no extra diff
        // Right above the checked-out commit, not at the very top of the graph; focused, only if that commit is shown.
        if let Some(row) = git::worktree_commit(status.head, stat) {
            let at = commits.iter().position(|c| Some(&c.id) == row.parents.first());
            if focus.is_empty() || at.is_some() {
                commits.insert(at.unwrap_or(0), row);
            }
        }
    }
    // Saved WIP of other branches: right above the commit it was saved on, when that commit is shown.
    let shelved = if q.path.is_none() { wip::shelved_commits(repo) } else { vec![] };
    let shelved_branches = shelved.iter().filter_map(|s| s.shelved.clone()).collect();
    for row in shelved {
        if let Some(at) = commits.iter().position(|c| Some(&c.id) == row.parents.first()) {
            commits.insert(at, row);
        }
    }

    let main_line = base_tip.map(|tip| git::main_line(repo, &tip)).unwrap_or_default();
    let rows = graph::compute(&commits, &main_line.set);
    let names = graph::branch_names(&commits, base.as_deref(), &main_line.set);
    Ok(History {
        rows: graph::to_wire(&rows, &commits),
        names: commits.iter().map(|c| names.get(&c.id).cloned()).collect(),
        commits,
        has_more,
        current,
        base,
        environments: branches.environments,
        all_branches: branches.names,
        local: branches.local,
        fast_forwarded,
        shelved_branches,
    })
}

/// Every branch, or only some: mine (plus the base and the environments, so the graph keeps its spine) and/or a
/// filter's. "Commits collapsed": only the commits a branch or tag points at, each linked to the next one down.
fn browse(repo: &Path, q: &Query, base: Option<&str>, environments: &[String], current: Option<&str>) -> Result<Vec<Commit>, String> {
    let picked: Option<Vec<String>> = match (q.only_mine, &q.only) {
        (false, None) => None,
        (true, None) => Some(refs::my_branches(repo)),
        (false, Some(only)) => Some(only.iter().map(|r| full_name(r)).collect()),
        (true, Some(only)) => {
            let only: HashSet<String> = only.iter().map(|r| full_name(r)).collect();
            Some(refs::my_branches(repo).into_iter().filter(|r| only.contains(r)).collect())
        }
    };
    let mut query = LogQuery { with_head: true, simplify: q.branches_only && q.path.is_none(), path: q.path.as_deref(), limit: q.limit, ..Default::default() };
    if let Some(picked) = picked {
        let shown: Vec<String> = picked.into_iter().chain(base.map(String::from)).chain(environments.iter().cloned()).collect();
        // Walking from the base also reaches other branches merged into it: name only these refs (plus the
        // checked-out branch, HEAD and, unless filtering, tags), so their names and collapsed rows don't show up.
        let tags = q.only.is_none().then(|| "refs/tags/".to_string());
        query.decorate = Some(shown.iter().map(|r| full_name(r)).chain(current.map(full_name)).chain(["HEAD".into()]).chain(tags).collect());
        query.include = shown;
    }
    git::log(repo, &query)
}

/// "Only this branch": its own line (first parents, minus what's already in `base`), plus, once merged into `base`,
/// that merge on top and the fork point below, linked so the branch is seen leaving the base and coming back.
/// Fast-forwarded into the base instead: the base's next commit on top, on the same line. Also says which one.
fn focused(repo: &Path, tip: &str, base: Option<&String>, q: &Query) -> Result<(Vec<Commit>, bool), String> {
    let next = base.filter(|_| q.path.is_none()).and_then(|base| next_on_base(repo, tip, base));
    let fast_forwarded = next.as_ref().is_some_and(|(_, ff)| *ff);
    let merge = next.as_ref().filter(|(_, ff)| !ff).map(|(id, _)| id.clone());
    let own = |exclude: bool| {
        // Stops where the branch left: before the merge (so the merge's other side isn't walked), else the base.
        let stop = merge.as_ref().map(|m| format!("{m}^1")).or_else(|| base.cloned());
        let exclude = stop.filter(|_| exclude).into_iter().collect();
        git::log(repo, &LogQuery { include: vec![tip.into()], exclude, first_parent: true, path: q.path.as_deref(), limit: q.limit, ..Default::default() })
    };
    // Already fully in its base: its own line from the tip down instead of nothing.
    let mut commits = own(true)?;
    let fork = commits.last().and_then(|oldest| oldest.parents.first().cloned());
    if commits.is_empty() {
        commits = own(false)?;
    }
    let newest = commits.first().map(|c| c.id.clone());
    let wanted: Vec<String> = merge.iter().chain(fork.iter().filter(|_| merge.is_some())).chain(next.iter().filter(|(_, ff)| *ff).map(|(id, _)| id)).cloned().collect();
    let mut extra = if wanted.is_empty() || newest.is_none() {
        vec![]
    } else {
        git::commits(&wanted, repo)?
    };
    let mut take = |id: &String| extra.iter().position(|c| &c.id == id).map(|i| extra.remove(i));
    if let (Some(merge), Some(fork), Some(newest)) = (&merge, &fork, &newest) {
        if let (Some(mut merged), Some(mut forked)) = (take(merge), take(fork)) {
            merged.parents = vec![forked.id.clone(), newest.clone()];
            forked.parents.clear();
            commits.insert(0, merged);
            commits.push(forked);
        }
    }
    if let (Some((next, true)), Some(newest)) = (&next, &newest) {
        if let Some(mut after) = take(next) {
            after.parents = vec![newest.clone()];
            commits.insert(0, after);
        }
    }
    // Merges keep only the parent shown: the merged-in branches are hidden, so no lines to nowhere.
    let ids: HashSet<String> = commits.iter().map(|c| c.id.clone()).collect();
    for commit in &mut commits {
        commit.parents.retain(|p| ids.contains(p));
    }
    Ok((commits, fast_forwarded))
}

/// The first commit on `base`'s main line after `tip`: a merge that brought it in (false), or, when `tip` itself sits
/// on that line, the commit the base went on with after a fast-forward (true). None while not in `base`.
/// (--first-parent with --ancestry-path misses a merge that brought it in through another branch, like qa merged
/// into production: so the base's main line and the tip's descendants, crossed.)
fn next_on_base(repo: &Path, tip: &str, base: &str) -> Option<(String, bool)> {
    let tip_id = refs::tip(tip, repo)?;
    let line = git::main_line(repo, &refs::tip(base, repo)?);
    if let Some(at) = line.order.iter().position(|c| *c == tip_id) {
        return Some((line.order.get(at.checked_sub(1)?)?.clone(), true));
    }
    let after = git::run(&["rev-list", "--ancestry-path", "--max-count=20000", &format!("{tip_id}..{base}")], repo).ok()?;
    let after: HashSet<&str> = after.lines().collect();
    line.order.iter().rev().find(|c| after.contains(c.as_str())).map(|c| (c.clone(), false))
}

#[cfg(test)]
mod tests {
    use crate::testutil::Repo;

    fn query(focus: &[&str]) -> super::Query {
        super::Query {
            limit: 300, only_mine: false, branches_only: false, preferred: Some("main".into()),
            focus: focus.iter().map(|s| s.to_string()).collect(), path: None, environments: None, only: None,
        }
    }

    /// "Only this branch" on a merged branch: the merge on top, its own commits, the fork point below, linked up.
    #[test]
    fn focus_shows_where_a_merged_branch_left_and_came_back() {
        let repo = Repo::new("focus-merge");
        repo.commit("a.txt", "1", "init");
        repo.git(&["checkout", "-q", "-b", "feat"]);
        repo.commit("f.txt", "1", "feat 1");
        repo.commit("f.txt", "2", "feat 2");
        repo.git(&["checkout", "-q", "main"]);
        repo.commit("a.txt", "2", "main moves");
        repo.git(&["merge", "-q", "--no-ff", "-m", "merge feat", "feat"]);
        repo.commit("a.txt", "3", "after");
        let history = super::load(repo.path(), &query(&["feat", "main"])).unwrap();
        let subjects: Vec<&str> = history.commits.iter().map(|c| c.subject.as_str()).collect();
        assert_eq!(subjects, ["merge feat", "feat 2", "feat 1", "init"]);
        let id = |i: usize| history.commits[i].id.clone();
        assert_eq!(history.commits[0].parents, [id(3), id(1)], "merge links to the fork point and the tip");
        assert_eq!(history.commits[2].parents, [id(3)], "the branch starts at the fork point");
        assert!(!history.fast_forwarded);

        // Merged through another branch (feature → qa, then qa → main): that qa merge is where it came back.
        repo.git(&["checkout", "-q", "-b", "qa2"]);
        repo.git(&["checkout", "-q", "-b", "via"]);
        repo.commit("v.txt", "1", "via 1");
        repo.git(&["checkout", "-q", "qa2"]);
        repo.git(&["merge", "-q", "--no-ff", "-m", "via into qa2", "via"]);
        repo.git(&["checkout", "-q", "main"]);
        repo.commit("a.txt", "3b", "main moves again");
        repo.git(&["merge", "-q", "--no-ff", "-m", "qa2 into main", "qa2"]);
        let history = super::load(repo.path(), &query(&["via", "main"])).unwrap();
        assert_eq!(history.commits[0].subject, "qa2 into main");
        assert_eq!(history.commits[1].subject, "via 1");

        // Fast-forwarded: no merge, the base's next commit on top of the tip, same line; its card says so too.
        repo.git(&["checkout", "-q", "-b", "ff"]);
        repo.commit("g.txt", "1", "ff 1");
        repo.git(&["checkout", "-q", "main"]);
        repo.git(&["merge", "-q", "--ff-only", "ff"]);
        repo.commit("a.txt", "4", "main after ff");
        let history = super::load(repo.path(), &query(&["ff", "main"])).unwrap();
        assert!(history.fast_forwarded);
        assert_eq!(history.commits[0].subject, "main after ff");
        assert_eq!(history.commits[0].parents, [history.commits[1].id.clone()]);
        let card = crate::branches::status(repo.path(), &["ff".into()], "main", &["main".into()]);
        assert_eq!(card[0].promotion, [("main".to_string(), 0, crate::branches::How::FastForwarded)]);
    }

    #[test]
    fn uncommitted_row_sits_on_its_commit_with_its_size() {
        let repo = Repo::new("worktree-row");
        repo.commit("a.txt", "1\n", "init");
        std::fs::write(repo.path().join("a.txt"), "1\n2\n").unwrap();
        let history = super::load(repo.path(), &query(&[])).unwrap();
        assert_eq!(history.commits[0].id, crate::git::WORKTREE_ID);
        assert_eq!(history.commits[0].stat, Some(crate::git::Stat { files: 1, added: 0, removed: 0 }));
        assert_eq!(history.current.as_deref(), Some("main"));
    }
}
