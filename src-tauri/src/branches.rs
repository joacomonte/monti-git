//! What the graph knows about branches beyond the log: which ones are environments, and the behind/merged facts of the
//! branches on screen (fetched lazily, only for the visible rows of "Commits collapsed").
use crate::git;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// Branch names (whole name, any case) treated as environments.
const ENVIRONMENT_NAMES: [&str; 12] =
    ["qa", "dev", "develop", "development", "staging", "stage", "test", "testing", "uat", "preprod", "pre-prod", "release"];

// shortcut: same origin/upstream assumption as RefBadge.
fn local_name(reference: &str) -> String {
    match reference.split_once('/') {
        Some(("origin" | "upstream", rest)) => rest.to_string(),
        _ => reference.to_string(),
    }
}

fn ok(args: &[&str], repo: &Path) -> Option<String> {
    git::run(args, repo).ok()
}

fn count(args: &[&str], repo: &Path) -> u32 {
    let args: Vec<&str> = ["rev-list", "--count"].iter().copied().chain(args.iter().copied()).collect();
    ok(&args, repo).and_then(|s| s.trim().parse().ok()).unwrap_or(0)
}

/// Every local branch, newest commit first (for "Change base"); the environments (qa, develop…) as refs git knows:
/// the local name, else "origin/<name>"; and every branch name, local or on origin (for the environments picker).
/// `chosen`: the user's environments for this repo (by name); None = detected by name. One cheap for-each-ref, nothing per branch.
pub fn branches(repo: &Path, base: Option<&str>, chosen: Option<&[String]>) -> (Vec<String>, Vec<String>, Vec<String>) {
    let refs = ok(&["for-each-ref", "--sort=-committerdate", "--format=%(refname)", "refs/heads", "refs/remotes/origin"], repo).unwrap_or_default();
    let local: Vec<String> = refs.lines().filter_map(|r| r.strip_prefix("refs/heads/")).map(String::from).collect();
    let remote: Vec<String> = refs.lines().filter_map(|r| r.strip_prefix("refs/remotes/")).map(String::from).collect();
    let local_set: HashSet<&str> = local.iter().map(String::as_str).collect();
    let base_name = base.map(local_name);
    let candidates = local.iter().cloned().chain(remote.iter().filter(|r| *r != "origin/HEAD" && *r != "origin").cloned());
    let is_environment = |name: &str| match chosen {
        Some(chosen) => chosen.iter().any(|c| c == name),
        None => ENVIRONMENT_NAMES.contains(&name.to_lowercase().as_str()),
    };
    let mut environments: Vec<String> = vec![];
    let mut names: Vec<String> = vec![];
    for candidate in candidates {
        let name = local_name(&candidate);
        let resolved = if local_set.contains(name.as_str()) { name.clone() } else { candidate };
        if is_environment(&name) && Some(&name) != base_name.as_ref() && !environments.contains(&resolved) {
            environments.push(resolved);
        }
        if !names.contains(&name) {
            names.push(name);
        }
    }
    (local, environments, names)
}

/// Filters, fast pass: every branch (`all`, short names: "x", "origin/x") and, per target, those whose tip is already
/// in it by history. One for-each-ref per target; squash / cherry-pick merges are confirmed later with `status`.
#[derive(Serialize)]
pub struct Merged {
    all: Vec<String>,
    merged: Vec<(String, Vec<String>)>,
}

pub fn merged_into(repo: &Path, targets: &[String], only_mine: bool) -> Merged {
    let list = |merged: Option<&str>| -> Vec<String> {
        let flag = merged.map(|t| format!("--merged={t}"));
        let mut args = vec!["for-each-ref", "--format=%(refname:short)"];
        args.extend(flag.as_deref());
        args.extend(["refs/heads", "refs/remotes/origin"]);
        ok(&args, repo).unwrap_or_default().lines().filter(|r| *r != "origin/HEAD" && *r != "origin").map(String::from).collect()
    };
    let all = if only_mine {
        let short = |r: String| r.strip_prefix("refs/heads/").or_else(|| r.strip_prefix("refs/remotes/")).map(String::from).unwrap_or(r);
        git::my_branches(repo).into_iter().map(short).collect()
    } else {
        list(None)
    };
    Merged { all, merged: targets.iter().map(|t| (t.clone(), list(Some(t)))).collect() }
}

/// Runs `build` for every ref on a few threads (read-only git is safe to run concurrently), keeping the order.
fn parallel<T: Send>(refs: &[String], build: impl Fn(&str) -> T + Sync) -> Vec<T> {
    let next = AtomicUsize::new(0);
    let built: Mutex<Vec<Option<T>>> = Mutex::new((0..refs.len()).map(|_| None).collect());
    let workers = std::thread::available_parallelism().map_or(4, |n| n.get()).min(refs.len());
    std::thread::scope(|s| {
        for _ in 0..workers {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= refs.len() {
                    break;
                }
                let item = build(&refs[i]);
                built.lock().unwrap()[i] = Some(item);
            });
        }
    });
    built.into_inner().unwrap().into_iter().flatten().collect()
}

/// Graph "Commits collapsed": what a branch's card says.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    #[serde(rename = "ref")]
    reference: String,
    behind_base: u32,
    // Per target: own commits (not in base) not in it yet, and how they got there when 0: "merged", "cherry-picked"
    // (same diffs copied: cherry-pick or a rebase merge), "squashed" (contents already there) or "fast-forwarded"
    // (the base: its tip sits on the base's main line, no merge commit); "" while missing.
    pub(crate) promotion: Vec<(String, u32, &'static str)>,
    first_parent_count: u32, // its own line since the base: "N commits"
    has_upstream: bool, // pushed: delete offers the server copy too, a rebase needs a force push
}

/// A ref's facts only change when it, the base or a target moves: kept per (repo, ref), keyed by those commits.
static CACHE: Mutex<Option<HashMap<(String, String), (String, Status)>>> = Mutex::new(None);

pub fn status(repo: &Path, refs: &[String], base: &str, targets: &[String]) -> Vec<Status> {
    // One for-each-ref for every branch: its commit, its upstream and how far it is from the base (git 2.41+;
    // older git falls back to one rev-list per count below).
    let list = |ahead_behind: bool| {
        let format = format!("--format=%(refname)%09%(objectname)%09%(upstream){}", if ahead_behind { format!("%09%(ahead-behind:{base})") } else { String::new() });
        ok(&["for-each-ref", &format, "refs/heads", "refs/remotes"], repo)
    };
    let listed = list(true).or_else(|| list(false)).unwrap_or_default();
    let mut tips: HashMap<&str, &str> = HashMap::new();
    let mut tracked: HashSet<&str> = HashSet::new();
    let mut ahead_behind: HashMap<&str, (u32, u32)> = HashMap::new();
    for line in listed.lines() {
        let mut fields = line.split('\t');
        let (Some(full), Some(oid)) = (fields.next(), fields.next()) else { continue };
        let Some(name) = full.strip_prefix("refs/heads/").or_else(|| full.strip_prefix("refs/remotes/")) else { continue };
        tips.insert(name, oid);
        if fields.next().is_some_and(|up| !up.is_empty()) && full.starts_with("refs/heads/") {
            tracked.insert(name);
        }
        if let Some((ahead, behind)) = fields.next().and_then(|ab| ab.split_once(' ')) {
            if let (Ok(a), Ok(b)) = (ahead.parse(), behind.parse()) {
                ahead_behind.insert(name, (a, b));
            }
        }
    }
    let against: Vec<&str> = std::iter::once(base).chain(targets.iter().map(String::as_str)).map(|r| tips.get(r).copied().unwrap_or("?")).collect();
    let key_of = |reference: &str| format!("{} {}", tips.get(reference).copied().unwrap_or("?"), against.join(" "));
    let repo_key = repo.to_string_lossy().to_string();
    let cached: HashMap<String, Status> = {
        let cache = CACHE.lock().unwrap();
        refs.iter()
            .filter_map(|r| cache.as_ref()?.get(&(repo_key.clone(), r.clone())).filter(|(key, _)| *key == key_of(r)).map(|(_, s)| (r.clone(), s.clone())))
            .collect()
    };
    let missing: Vec<String> = refs.iter().filter(|r| !cached.contains_key(*r)).cloned().collect();

    // GitHub's rebase and squash merges land the same changes as new commits, so the plain count misses them:
    // `git cherry` matches rebased commits by content (patch id), and a merge that would change nothing means squashed.
    let merged_another_way = |target: &str, reference: &str, missing: u32| -> (u32, &'static str) {
        let limit = (target != base).then(|| ok(&["merge-base", base, reference], repo)).flatten().map(|s| s.trim().to_string());
        let mut args = vec!["cherry", target, reference];
        args.extend(limit.as_deref());
        let not_in = ok(&args, repo).map_or(missing, |out| out.lines().filter(|l| l.starts_with('+')).count() as u32);
        if not_in == 0 {
            (0, "cherry-picked")
        } else if git::already_contains(target, reference, repo) {
            (0, "squashed")
        } else {
            (not_in.min(missing), "")
        }
    };
    // The base's main line (cached): a tip on it reached the base by a fast-forward, not a merge.
    let main_line = git::main_chain(repo, tips.get(base).copied());
    let built = parallel(&missing, |reference| {
        let (ahead, behind) = ahead_behind
            .get(reference)
            .copied()
            .unwrap_or_else(|| (count(&[&format!("{base}..{reference}")], repo), count(&[&format!("{reference}..{base}")], repo)));
        Status {
            reference: reference.to_string(),
            behind_base: behind,
            // Only this branch's own work: a feature cut from the base would otherwise count every base commit the environment doesn't have.
            promotion: targets
                .iter()
                .map(|t| {
                    let missing = if t == base { ahead } else { count(&[reference, &format!("^{t}"), &format!("^{base}")], repo) };
                    let on_main_line = t == base && tips.get(reference).is_some_and(|oid| main_line.contains(*oid));
                    let (missing, how) = if missing == 0 { (0, if on_main_line { "fast-forwarded" } else { "merged" }) } else { merged_another_way(t, reference, missing) };
                    (t.clone(), missing, how)
                })
                .collect(),
            first_parent_count: count(&["--first-parent", reference, &format!("^{base}")], repo),
            has_upstream: false,
        }
    });
    {
        let mut cache = CACHE.lock().unwrap();
        let cache = cache.get_or_insert_with(HashMap::new);
        for s in &built {
            cache.insert((repo_key.clone(), s.reference.clone()), (key_of(&s.reference), s.clone()));
        }
    }
    let mut built: HashMap<String, Status> = built.into_iter().map(|s| (s.reference.clone(), s)).collect();
    refs.iter()
        .filter_map(|r| cached.get(r).cloned().or_else(|| built.remove(r)))
        .map(|mut s| { s.has_upstream = tracked.contains(s.reference.as_str()); s })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::testutil::Repo;
    use std::collections::HashMap;

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

        let (local, environments, names) = super::branches(repo.path(), Some("main"), None);
        assert_eq!(environments, ["qa"]);
        assert!(local.contains(&"feat/open".to_string()) && names.contains(&"feat/open".to_string()));
        let (_, chosen, _) = super::branches(repo.path(), Some("main"), Some(&["feat/open".into(), "main".into()]));
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
        assert_eq!(how("feat/merged"), ["merged", "merged"]);
        assert_eq!(how("feat/squashed"), ["squashed", "squashed"]);
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
        assert_eq!(copy[0].promotion, [("qa".to_string(), 0, "cherry-picked")]);
    }
}
