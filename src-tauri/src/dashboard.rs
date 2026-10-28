//! Port of Dashboard.swift: one card per branch, against the base, the environments and its remote.
use crate::git::{self, Commit};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// A feature with no activity of mine for this long goes to "Hidden" unless the user says otherwise.
const ACTIVE_DAYS: i64 = 14;

/// Branch names (whole name, any case) treated as environments unless the user overrides the list.
const ENVIRONMENT_NAMES: [&str; 12] =
    ["qa", "dev", "develop", "development", "staging", "stage", "test", "testing", "uat", "preprod", "pre-prod", "release"];

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    date: i64, // last commit
    ahead: u32, // commits not pushed to the upstream
    behind: u32, // upstream commits not pulled yet
    has_upstream: bool, // false = never pushed
}

/// Size of a set of changed files.
#[derive(Serialize)]
pub struct Totals {
    files: usize,
    added: u32,
    removed: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    #[serde(rename = "ref")]
    reference: String, // what git commands use: "qa", or "origin/qa" when there is no local branch
    local: String,
    summary: Option<Summary>, // None for remote-only refs
    last: Option<Commit>, // latest commit
    promotion: Vec<(String, u32)>, // commits of this branch not yet in each target
    behind_base: u32,
    parent: Option<String>, // active features: branch it was cut from
    diff: Option<Totals>, // active features: changes against `parent`
    uncommitted: Option<Totals>, // current branch only
    worktree: Option<(usize, usize, usize)>, // current branch only: staged, not staged, untracked
    feature_commits: Vec<Commit>, // active features: own commits on top of `parent`, newest first, no merges
    squash_base: Option<String>, // active features: merge-base with `parent`
    already_in: Vec<String>, // active features: base/environments that already have some of its own commits
    behind_parent: u32,
    parent_tip: Option<String>, // a dismissed "update available" notice comes back when this moves
}

#[derive(Serialize)]
pub struct Dashboard {
    base: Option<Card>,
    environments: Vec<Card>,
    pinned: Vec<Card>,
    features: Vec<Card>, // "Recent activity"
    old: Vec<Card>, // "Hidden"
    current: Option<String>,
    shelved: Vec<String>, // branches with changes saved when leaving them (WIP)
    local: Vec<String>, // every local branch, for the "Change base branch" list
}

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

fn parse_commits(log: &str) -> Vec<Commit> {
    log.lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.split('\u{1f}').collect();
            (f.len() == 4).then(|| Commit {
                id: f[0].into(),
                parents: vec![],
                author: f[1].into(),
                date: f[2].parse().unwrap_or(0),
                refs: vec![],
                subject: f[3].into(),
            })
        })
        .collect()
}

/// Last commit date and ahead/behind vs upstream of every local branch.
fn summaries(repo: &Path) -> HashMap<String, Summary> {
    let output = ok(&["for-each-ref", "--format=%(refname:short)%09%(committerdate:unix)%09%(upstream:track,nobracket)%09%(upstream)", "refs/heads"], repo)
        .unwrap_or_default();
    output
        .lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() != 4 {
                return None;
            }
            // track: "ahead 2, behind 1", "behind 3", "gone" or empty.
            let n = |word: &str| f[2].split(", ").find_map(|p| p.strip_prefix(word)?.strip_prefix(' ')?.parse().ok()).unwrap_or(0);
            Some((f[0].to_string(), Summary { date: f[1].parse().unwrap_or(0), ahead: n("ahead"), behind: n("behind"), has_upstream: !f[3].is_empty() }))
        })
        .collect()
}

/// When I last did something on each local branch, from the HEAD reflog. Walking newest -> oldest, entries
/// belong to the branch checked out at that moment, which switches back at every "checkout: moving from A to B".
fn my_activity(repo: &Path, current: Option<&str>) -> HashMap<String, i64> {
    let output = ok(&["log", "-g", "--date=unix", "--max-count=3000", "--format=%gd%x09%gs", "HEAD"], repo).unwrap_or_default();
    let mut activity = HashMap::new();
    let mut branch = current.map(String::from);
    for line in output.lines() {
        let Some((selector, subject)) = line.split_once('\t') else { continue };
        // %gd with --date=unix is "HEAD@{1793026851}": when the reflog entry happened.
        let Ok(date) = selector.chars().filter(char::is_ascii_digit).collect::<String>().parse::<i64>() else { continue };
        if let Some(b) = &branch {
            activity.entry(b.clone()).or_insert(date);
        }
        if let Some((from, _)) = subject.strip_prefix("checkout: moving from ").and_then(|rest| rest.split_once(" to ")) {
            activity.entry(from.to_string()).or_insert(date); // leaving a branch counts too
            branch = Some(from.to_string());
        }
    }
    activity
}

/// Branches with changes saved when leaving them: a "monti-git: <branch>" stash or a WIP commit on top.
fn shelved(repo: &Path) -> Vec<String> {
    let stashes = ok(&["stash", "list", "--format=%gs"], repo).unwrap_or_default();
    let tips = ok(&["for-each-ref", "--format=%(refname:short)%09%(contents:subject)", "refs/heads"], repo).unwrap_or_default();
    stashes
        .lines()
        .filter_map(|line| line.split_once(": monti-git: ").map(|(_, b)| b.to_string()))
        .chain(tips.lines().filter_map(|line| match line.split_once('\t') {
            Some((name, "WIP [monti-git]")) => Some(name.to_string()),
            _ => None,
        }))
        .collect()
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
                let card = build(&refs[i]);
                built.lock().unwrap()[i] = Some(card);
            });
        }
    });
    built.into_inner().unwrap().into_iter().flatten().collect()
}

#[derive(PartialEq, Clone, Copy)]
enum Detail {
    None,
    Light, // old features: name, last commit and sync only, no counting against other branches
    Full,
}

/// `pinned`, `environment_override` and `states` ("active" / "old" per feature) are the user's prefs for this repo.
pub fn load(repo: &Path, preferred: Option<&str>, pinned: &[String], environment_override: Option<&[String]>, states: &HashMap<String, String>) -> Dashboard {
    let current = ok(&["symbolic-ref", "--quiet", "--short", "HEAD"], repo).map(|s| s.trim().to_string());
    let activity = my_activity(repo, current.as_deref());
    // Most recent activity of mine first; branches I never touched keep commit-date order after them (stable sort).
    let mut local: Vec<String> = ok(&["for-each-ref", "--sort=-committerdate", "--format=%(refname:short)", "refs/heads"], repo)
        .unwrap_or_default()
        .lines()
        .map(String::from)
        .collect();
    local.sort_by_key(|b| std::cmp::Reverse(activity.get(b).copied().unwrap_or(i64::MIN)));
    let base = git::main_ref(repo, preferred);
    let summaries = summaries(repo);

    let local_set: HashSet<&str> = local.iter().map(String::as_str).collect();
    let resolve = |name: &str| if local_set.contains(name) { name.to_string() } else { format!("origin/{name}") };
    let remote_only: Vec<String> = ok(&["for-each-ref", "--format=%(refname:short)", "refs/remotes/origin"], repo)
        .unwrap_or_default()
        .lines()
        .filter(|r| *r != "origin/HEAD" && *r != "origin" && !local_set.contains(local_name(r).as_str()))
        .map(String::from)
        .collect();
    let base_name = base.as_deref().map(local_name);
    let candidates: Vec<String> = local.iter().cloned().chain(remote_only.iter().map(|r| local_name(r))).collect();
    let environments: Vec<String> = match environment_override {
        Some(list) => list.to_vec(),
        None => candidates.iter().filter(|c| ENVIRONMENT_NAMES.contains(&c.to_lowercase().as_str())).cloned().collect(),
    }
    .into_iter()
    .filter(|e| Some(e) != base_name.as_ref() && candidates.contains(e))
    .map(|e| resolve(&e))
    .collect();

    let mine: HashSet<String> = git::my_branches(repo).into_iter().filter_map(|r| r.strip_prefix("refs/heads/").map(String::from)).collect();
    let pins: Vec<String> = pinned
        .iter()
        .filter(|p| local_set.contains(p.as_str()) && Some(*p) != base_name.as_ref() && !environments.contains(p))
        .cloned()
        .collect();
    let features: Vec<String> = local
        .iter()
        .filter(|b| mine.contains(*b) && Some(*b) != base_name.as_ref() && !environments.contains(b) && !pins.contains(b))
        .take(40)
        .cloned()
        .collect();

    let cutoff = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64) - ACTIVE_DAYS * 86400;
    let is_active = |name: &String| {
        if Some(name) == current.as_ref() {
            return true;
        }
        match states.get(name).map(String::as_str) {
            Some("active") => true,
            Some("old") => false,
            _ => activity.get(name).copied().or(summaries.get(name).map(|s| s.date)).unwrap_or(0) > cutoff,
        }
    };

    let parent_candidates: Vec<String> = base.iter().cloned().chain(environments.iter().cloned()).collect();

    // git records "branch: Created from main" in the branch's reflog; else the base/environment it has the fewest commits on top of.
    let parent_of = |reference: &str, name: &str| -> Option<String> {
        let reflog = ok(&["reflog", "show", "--format=%gs", &format!("refs/heads/{name}")], repo).unwrap_or_default();
        if let Some(from) = reflog.lines().last().and_then(|l| l.strip_prefix("branch: Created from ")) {
            let from = local_name(from);
            if from != name && from != "HEAD" && ok(&["rev-parse", "--verify", "--quiet", &resolve(&from)], repo).is_some() {
                return Some(resolve(&from));
            }
        }
        parent_candidates
            .iter()
            .filter(|c| c.as_str() != reference)
            .min_by_key(|c| count(&[&format!("{c}..{reference}")], repo))
            .cloned()
    };

    let totals = |status: &HashMap<String, String>, counts: &HashMap<String, (u32, u32)>| Totals {
        files: status.len(),
        added: status.keys().filter_map(|p| counts.get(p)).map(|c| c.0).sum(),
        removed: status.keys().filter_map(|p| counts.get(p)).map(|c| c.1).sum(),
    };

    let card = |reference: &str, targets: &[String], detail: Detail| -> Card {
        let light = detail == Detail::Light;
        let last = parse_commits(&ok(&["log", "-1", "--format=%H%x1f%an%x1f%at%x1f%s", reference, "--"], repo).unwrap_or_default()).pop();
        let mut card = Card {
            reference: reference.to_string(),
            local: local_name(reference),
            summary: summaries.get(&local_name(reference)).cloned(),
            last,
            // Only this branch's own work: a feature cut from the base would otherwise count every base commit the environment doesn't have.
            promotion: if light { vec![] } else {
                targets
                    .iter()
                    .map(|t| match &base {
                        Some(b) if b != t => (t.clone(), count(&[reference, &format!("^{t}"), &format!("^{b}")], repo)),
                        _ => (t.clone(), count(&[&format!("{t}..{reference}")], repo)),
                    })
                    .collect()
            },
            behind_base: if light { 0 } else { base.as_ref().map_or(0, |b| count(&[&format!("{reference}..{b}")], repo)) },
            parent: None,
            diff: None,
            uncommitted: None,
            worktree: None,
            feature_commits: vec![],
            squash_base: None,
            already_in: vec![],
            behind_parent: 0,
            parent_tip: None,
        };
        if detail == Detail::Full {
            card.parent = parent_of(reference, &card.local);
            if let Some(parent) = card.parent.clone() {
                if let Some(merge_base) = ok(&["merge-base", &parent, reference], repo).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
                    card.diff = Some(totals(&git::changed_files(Some(&merge_base), reference, repo), &git::line_counts(Some(&merge_base), Some(reference), repo)));
                    // shortcut: last 100, enough to read and to write a squash message.
                    let range = format!("{merge_base}..{reference}");
                    card.feature_commits = parse_commits(&ok(&["log", "--no-merges", "-100", "--format=%H%x1f%an%x1f%at%x1f%s", &range], repo).unwrap_or_default());
                    let not_base = format!("^{merge_base}");
                    let own = count(&[reference, &not_base], repo);
                    card.already_in = parent_candidates
                        .iter()
                        .filter(|c| **c != parent && count(&[reference, &not_base, &format!("^{c}")], repo) < own)
                        .map(|c| local_name(c))
                        .collect();
                    card.behind_parent = count(&[&format!("{reference}..{parent}")], repo);
                    card.parent_tip = ok(&["rev-parse", &parent], repo).map(|s| s.trim().to_string());
                    card.squash_base = Some(merge_base);
                }
            }
        }
        if Some(reference) == current.as_deref() {
            let status = git::worktree_files(repo);
            let mut counts = git::line_counts(None, None, repo);
            for (path, letter) in &status {
                // Untracked files are not in `git diff`: count their lines.
                if letter == "A" && !counts.contains_key(path) {
                    if let Ok(text) = std::fs::read_to_string(repo.join(path)) {
                        counts.insert(path.clone(), (text.matches('\n').count() as u32, 0));
                    }
                }
            }
            card.uncommitted = (!status.is_empty()).then(|| totals(&status, &counts));
            // "XY path": X = staged, Y = not staged, "??" = new file git doesn't track yet.
            let porcelain = ok(&["status", "--porcelain", "--untracked-files=all"], repo).unwrap_or_default();
            let lines: Vec<&[u8]> = porcelain.lines().map(str::as_bytes).collect();
            let marked = |i: usize| lines.iter().filter(|l| l.get(i).is_some_and(|c| *c != b' ' && *c != b'?')).count();
            card.worktree = Some((marked(0), marked(1), lines.iter().filter(|l| l.starts_with(b"??")).count()));
        }
        card
    };

    let base_targets: Vec<String> = base.iter().cloned().collect();
    let targets: Vec<String> = environments.iter().cloned().chain(base.iter().cloned()).collect();
    let (active, old): (Vec<String>, Vec<String>) = features.into_iter().partition(is_active);
    Dashboard {
        base: base.as_deref().map(|b| card(b, &[], Detail::None)),
        environments: parallel(&environments, |r| card(r, &base_targets, Detail::None)),
        pinned: parallel(&pins, |r| card(r, &targets, Detail::Full)),
        features: parallel(&active, |r| card(r, &targets, Detail::Full)),
        old: parallel(&old, |r| card(r, &[], Detail::Light)),
        shelved: shelved(repo),
        local: local.clone(),
        current,
    }
}
