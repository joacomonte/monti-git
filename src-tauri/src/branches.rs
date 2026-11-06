//! The behind/merged facts of the branch cards in "Commits collapsed" (fetched lazily, only for the rows on screen),
//! and the filters' fast pass.
use crate::git::{self, read_opt};
use crate::{par, refs};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{LazyLock, Mutex};

fn count(args: &[&str], repo: &Path) -> u32 {
    let args: Vec<&str> = ["rev-list", "--count"].iter().copied().chain(args.iter().copied()).collect();
    read_opt(&args, repo).and_then(|s| s.trim().parse().ok()).unwrap_or(0)
}

/// Filters, fast pass: every branch (`all`, short names: "x", "origin/x") and, per target, those whose tip is already
/// in it by history. One for-each-ref per target; squash / cherry-pick merges are confirmed later with `status`.
#[derive(Serialize)]
pub struct Merged {
    all: Vec<String>,
    merged: Vec<(String, Vec<String>)>,
}

pub fn merged_into(repo: &Path, targets: &[String], only_mine: bool) -> Merged {
    let all = if only_mine {
        refs::my_branches(repo).iter().map(|r| refs::short_name(r).to_string()).collect()
    } else {
        refs::branch_names(repo, None)
    };
    Merged { all, merged: targets.iter().map(|t| (t.clone(), refs::branch_names(repo, Some(t)))).collect() }
}

/// How a branch's own commits got into a target, once none is missing.
#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum How {
    #[serde(rename = "")]
    Missing,
    Merged,
    FastForwarded, // the base moved up to its tip: no merge commit
    CherryPicked,  // same diffs copied: cherry-pick or a rebase merge
    Squashed,      // its contents are already there
}

/// Graph "Commits collapsed": what a branch's card says.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    #[serde(rename = "ref")]
    reference: String,
    behind_base: u32,
    /// Per target: own commits (not in base) not in it yet, and how they got there when 0.
    pub(crate) promotion: Vec<(String, u32, How)>,
    first_parent_count: u32, // its own line since the base: "N commits"
    has_upstream: bool,      // pushed: delete offers the server copy too, a rebase needs a force push
    unpushed: u32,           // of its own commits, those the server doesn't have: ahead of its upstream (at most all), or all if never pushed
}

impl Status {
    pub fn reference(&self) -> &str {
        &self.reference
    }

    /// In the base and every environment: safe to delete (cards.ts mergeState "done" is the same rule, for the web view).
    pub fn merged_everywhere(&self) -> bool {
        !self.promotion.is_empty() && self.promotion.iter().all(|(_, missing, _)| *missing == 0)
    }
}

/// A ref's facts only change when it, the base or a target moves: kept per (repo, ref), keyed by those commits.
static CACHE: LazyLock<Mutex<HashMap<(String, String), (String, Status)>>> = LazyLock::new(Mutex::default);

pub fn status(repo: &Path, refs: &[String], base: &str, targets: &[String]) -> Vec<Status> {
    // One for-each-ref for every branch: its commit, its upstream and how far it is from the base (git 2.41+;
    // older git falls back to one rev-list per count below).
    let list = |ahead_behind: bool| {
        let format = format!("--format=%(refname)%09%(objectname)%09%(upstream)%09%(upstream:track,nobracket){}", if ahead_behind { format!("%09%(ahead-behind:{base})") } else { String::new() });
        read_opt(&["for-each-ref", &format, "refs/heads", "refs/remotes"], repo)
    };
    let listed = list(true).or_else(|| list(false)).unwrap_or_default();
    let mut tips: HashMap<&str, &str> = HashMap::new();
    let mut tracked: HashSet<&str> = HashSet::new();
    let mut ahead_behind: HashMap<&str, (u32, u32)> = HashMap::new();
    let mut ahead_of_upstream: HashMap<&str, u32> = HashMap::new();
    for line in listed.lines() {
        let mut fields = line.split('\t');
        let (Some(full), Some(oid)) = (fields.next(), fields.next()) else { continue };
        let name = refs::short_name(full);
        tips.insert(name, oid);
        if fields.next().is_some_and(|up| !up.is_empty()) && full.starts_with("refs/heads/") {
            tracked.insert(name);
        }
        // "ahead 2, behind 1", "behind 3", "gone" or empty (in sync).
        let track = fields.next().unwrap_or_default();
        let ahead = track.split(", ").find_map(|part| part.strip_prefix("ahead ")).and_then(|n| n.parse().ok()).unwrap_or(0);
        ahead_of_upstream.insert(name, ahead);
        if let Some((ahead, behind)) = fields.next().and_then(|ab| ab.split_once(' ')) {
            if let (Ok(a), Ok(b)) = (ahead.parse(), behind.parse()) {
                ahead_behind.insert(name, (a, b));
            }
        }
    }
    // Every query below uses these commits, not the names: a branch moving meanwhile can't mix two states.
    let oid = |name: &str| tips.get(name).copied().unwrap_or(name).to_string();
    let base_oid = oid(base);
    let against: Vec<String> = std::iter::once(base).chain(targets.iter().map(String::as_str)).map(oid).collect();
    let key_of = |reference: &str| format!("{} {}", oid(reference), against.join(" "));
    let repo_key = repo.to_string_lossy().to_string();
    let cached: HashMap<String, Status> = {
        let cache = CACHE.lock().unwrap();
        refs.iter()
            .filter_map(|r| cache.get(&(repo_key.clone(), r.clone())).filter(|(key, _)| *key == key_of(r)).map(|(_, s)| (r.clone(), s.clone())))
            .collect()
    };
    let missing: Vec<String> = refs.iter().filter(|r| !cached.contains_key(*r)).cloned().collect();

    // GitHub's rebase and squash merges land the same changes as new commits, so the plain count misses them:
    // `git cherry` matches rebased commits by content (patch id), and a merge that would change nothing means squashed.
    let merged_another_way = |target: &str, tip: &str, missing: u32| -> (u32, How) {
        let limit = (target != base_oid).then(|| read_opt(&["merge-base", &base_oid, tip], repo)).flatten().map(|s| s.trim().to_string());
        let mut args = vec!["cherry", target, tip];
        args.extend(limit.as_deref());
        let not_in = read_opt(&args, repo).map_or(missing, |out| out.lines().filter(|l| l.starts_with('+')).count() as u32);
        if not_in == 0 {
            (0, How::CherryPicked)
        } else if git::already_contains(target, tip, repo) {
            (0, How::Squashed)
        } else {
            (not_in.min(missing), How::Missing)
        }
    };
    // The base's main line (cached): a tip on it reached the base by a fast-forward, not a merge.
    let main_line = git::main_line(repo, &base_oid);
    let built = par::map(&missing, |reference| {
        let tip = oid(reference);
        let (ahead, behind) = ahead_behind
            .get(reference.as_str())
            .copied()
            .unwrap_or_else(|| (count(&[&format!("{base_oid}..{tip}")], repo), count(&[&format!("{tip}..{base_oid}")], repo)));
        let promotion = targets
            .iter()
            .map(|t| {
                let target = oid(t);
                let missing = if target == base_oid { ahead } else { count(&[&tip, &format!("^{target}"), &format!("^{base_oid}")], repo) };
                let (missing, how) = match missing {
                    0 if target == base_oid && main_line.set.contains(&tip) => (0, How::FastForwarded),
                    0 => (0, How::Merged),
                    _ => merged_another_way(&target, &tip, missing),
                };
                (t.clone(), missing, how)
            })
            .collect();
        Status {
            reference: reference.clone(),
            behind_base: behind,
            promotion,
            first_parent_count: count(&["--first-parent", &tip, &format!("^{base_oid}")], repo),
            has_upstream: false,
            unpushed: 0,
        }
    });
    {
        let mut cache = CACHE.lock().unwrap();
        for s in &built {
            cache.insert((repo_key.clone(), s.reference.clone()), (key_of(&s.reference), s.clone()));
        }
    }
    let mut built: HashMap<String, Status> = built.into_iter().map(|s| (s.reference.clone(), s)).collect();
    refs.iter()
        .filter_map(|r| cached.get(r).cloned().or_else(|| built.remove(r)))
        // Not cached: a push moves the upstream, not the branch.
        .map(|mut s| {
            let name = s.reference.as_str();
            s.has_upstream = tracked.contains(name);
            s.unpushed = if refs::local_name(name) != name { 0 } else if s.has_upstream { ahead_of_upstream.get(name).copied().unwrap_or(0).min(s.first_parent_count) } else { s.first_parent_count };
            s
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::How;
    use crate::testutil::Repo;
    use std::collections::HashMap;

    /// A card counts the commits the server doesn't have: ahead of its upstream, or all its own if never pushed.
    #[test]
    fn cards_tell_pushed_from_local_commits() {
        let server = Repo::new("pushed-server");
        server.commit("a.txt", "1", "init");
        let repo = Repo::new("pushed-local");
        repo.git(&["remote", "add", "origin", &server.path().display().to_string()]);
        repo.git(&["fetch", "-q", "origin"]);
        repo.git(&["checkout", "-q", "-b", "main", "origin/main"]);
        repo.git(&["checkout", "-q", "-b", "feat", "--no-track"]);
        repo.commit("f.txt", "1", "pushed one");
        repo.git(&["push", "-q", "-u", "origin", "feat"]);
        repo.commit("f.txt", "2", "local one");
        let status = super::status(repo.path(), &["feat".into()], "main", &["main".into()]);
        assert_eq!((status[0].first_parent_count, status[0].unpushed), (2, 1));
        repo.git(&["checkout", "-q", "-b", "fresh", "main"]);
        repo.commit("g.txt", "1", "never pushed");
        assert_eq!(super::status(repo.path(), &["fresh".into()], "main", &["main".into()])[0].unpushed, 1);
    }

    /// main and qa, and one feature per way of merging: plain merge, squash, cherry-pick (like a rebase merge),
    /// squash then more commits, and never merged.
    #[test]
    fn promotion_sees_every_kind_of_merge() {
        let repo = Repo::new("promotion");
        repo.commit("a.txt", "1", "init");
        repo.git(&["branch", "qa"]);
        let feature = |name: &str, file: &str| {
            repo.git(&["checkout", "-q", "-b", name, "main"]);
            repo.commit(file, "one", &format!("{name} 1"));
            repo.commit(file, "one\ntwo", &format!("{name} 2"));
        };
        let into_both = |how: &dyn Fn()| {
            for target in ["qa", "main"] {
                repo.git(&["checkout", "-q", target]);
                how();
            }
        };
        feature("feat/merged", "m.txt");
        into_both(&|| { repo.git(&["merge", "-q", "--no-ff", "-m", "merge", "feat/merged"]); });
        feature("feat/squashed", "s.txt");
        into_both(&|| { repo.git(&["merge", "-q", "--squash", "feat/squashed"]); repo.git(&["commit", "-q", "-m", "squash"]); });
        feature("feat/picked", "p.txt");
        into_both(&|| { repo.git(&["cherry-pick", "feat/picked~1", "feat/picked"]); });
        feature("feat/after", "f.txt");
        into_both(&|| { repo.git(&["merge", "-q", "--squash", "feat/after"]); repo.git(&["commit", "-q", "-m", "squash after"]); });
        repo.git(&["checkout", "-q", "feat/after"]);
        repo.commit("f.txt", "one\ntwo\nthree", "feat/after 3");
        feature("feat/open", "o.txt");
        repo.git(&["checkout", "-q", "main"]);

        let branches = crate::refs::branches(repo.path(), Some("main"), None);
        assert_eq!(branches.environments, ["qa"]);
        assert!(branches.local.contains(&"feat/open".to_string()) && branches.names.contains(&"feat/open".to_string()));
        let chosen = crate::refs::branches(repo.path(), Some("main"), Some(&["feat/open".into(), "main".into()])).environments;
        assert_eq!(chosen, ["feat/open"], "the user's pick replaces detection, and the base is never an environment");

        let names: Vec<String> = ["feat/merged", "feat/squashed", "feat/picked", "feat/after", "feat/open"].map(String::from).to_vec();
        let found = super::status(repo.path(), &names, "main", &["qa".into(), "main".into()]);
        let missing = |name: &str| {
            found.iter().find(|s| s.reference == name).unwrap().promotion.iter().map(|(t, n, _)| (t.clone(), *n)).collect::<HashMap<_, _>>()
        };
        for merged in ["feat/merged", "feat/squashed", "feat/picked"] {
            assert_eq!(missing(merged), HashMap::from([("qa".into(), 0), ("main".into(), 0)]), "{merged} should be in qa and main");
        }
        assert!(missing("feat/open").values().all(|n| *n == 2), "feat/open is nowhere");
        // Filters' fast pass: by history only, so the real merge is in both and the squash in neither.
        let fast = super::merged_into(repo.path(), &["qa".into(), "main".into()], false);
        assert!(fast.all.contains(&"feat/open".to_string()));
        assert!(fast.merged.iter().all(|(_, refs)| refs.contains(&"feat/merged".to_string()) && !refs.contains(&"feat/squashed".to_string())));
        let how = |name: &str| found.iter().find(|s| s.reference == name).unwrap().promotion.iter().map(|(_, _, h)| *h).collect::<Vec<_>>();
        assert_eq!(how("feat/merged"), [How::Merged, How::Merged]);
        assert_eq!(how("feat/squashed"), [How::Squashed, How::Squashed]);
        assert!(missing("feat/after").values().all(|n| *n > 0), "feat/after has a commit after the squash");

        // Cached until the branch moves: a new commit on feat/open must show up.
        repo.git(&["checkout", "-q", "feat/open"]);
        repo.commit("o.txt", "one\ntwo\nthree", "feat/open 3");
        let again = super::status(repo.path(), &["feat/open".into()], "main", &["qa".into(), "main".into()]);
        assert!(again[0].promotion.iter().all(|(_, n, _)| *n == 3) && again[0].first_parent_count == 3, "cache missed the new commit");

        // Copied into qa by cherry-pick only (a new commit there, never merged): "cherry-picked".
        repo.git(&["checkout", "-q", "-b", "feat/copy", "main"]);
        repo.commit("c.txt", "copy", "feat/copy 1");
        repo.git(&["checkout", "-q", "qa"]);
        repo.commit("q.txt", "qa only", "qa moves on"); // so the pick gets a new hash (same parent and second = same commit)
        repo.git(&["cherry-pick", "feat/copy"]);
        let copy = super::status(repo.path(), &["feat/copy".into()], "main", &["qa".into()]);
        assert_eq!(copy[0].promotion, [("qa".to_string(), 0, How::CherryPicked)]);
    }
}
