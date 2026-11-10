//! Branch names: how a ref is written ("main", "origin/main", "refs/heads/main"), which branch is the base,
//! which are environments, and which are the user's.
use crate::git::{read_opt, run};
use std::collections::HashSet;
use std::path::Path;

/// The remote new branches are pushed to and bases are compared with.
pub const DEFAULT_REMOTE: &str = "origin";

/// Remotes whose branches stand for the local branch of the same name ("origin/qa" is qa).
const REMOTES: [&str; 2] = ["origin", "upstream"];

/// Branch names (whole name, any case) treated as environments when the user didn't pick them.
pub const ENVIRONMENT_NAMES: [&str; 12] =
    ["qa", "dev", "develop", "development", "staging", "stage", "test", "testing", "uat", "preprod", "pre-prod", "release"];

/// Where work ends up: an environment or a production branch. The global search marks these with a ✓.
pub fn is_target(name: &str) -> bool {
    let name = local_name(name).to_lowercase();
    ENVIRONMENT_NAMES.contains(&name.as_str()) || ["main", "master", "production", "prod"].contains(&name.as_str())
}

/// "origin/x" / "upstream/x" -> "x"; anything else as is.
pub fn local_name(name: &str) -> &str {
    match name.split_once('/') {
        Some((remote, rest)) if REMOTES.contains(&remote) => rest,
        _ => name,
    }
}

/// "origin/x" -> "refs/remotes/origin/x", "x" -> "refs/heads/x"; a full ref stays.
pub fn full_name(name: &str) -> String {
    if name.starts_with("refs/") {
        name.to_string()
    } else if local_name(name) != name {
        format!("refs/remotes/{name}")
    } else {
        format!("refs/heads/{name}")
    }
}

/// "refs/heads/x" -> "x", "refs/remotes/origin/x" -> "origin/x".
pub fn short_name(full: &str) -> &str {
    full.strip_prefix("refs/heads/").or_else(|| full.strip_prefix("refs/remotes/")).unwrap_or(full)
}

/// "origin/HEAD" and a bare "origin" are aliases, not branches.
pub fn is_alias(short: &str) -> bool {
    short.ends_with("/HEAD") || REMOTES.contains(&short)
}

/// Checked-out branch, None when detached.
pub fn current_branch(repo: &Path) -> Option<String> {
    read_opt(&["symbolic-ref", "--quiet", "--short", "HEAD"], repo).map(|s| s.trim().to_string())
}

/// Commit a ref points at, None if there's no such ref.
pub fn tip(name: &str, repo: &Path) -> Option<String> {
    read_opt(&["rev-parse", "--verify", "--quiet", "--end-of-options", &format!("{name}^{{commit}}")], repo).map(|s| s.trim().to_string())
}

/// The branch pinned to the left and its commit: the one chosen for this repo, else main, else production
/// (local first, then origin/). One for-each-ref for every candidate.
pub fn main_ref(repo: &Path, preferred: Option<&str>) -> Option<(String, String)> {
    let candidates: Vec<String> =
        preferred.into_iter().chain(["main", "production"]).flat_map(|name| [name.to_string(), format!("origin/{name}")]).collect();
    let mut args = vec!["for-each-ref".to_string(), "--format=%(refname)%09%(objectname)".to_string()];
    args.extend(candidates.iter().map(|c| full_name(c)));
    let out = read_opt(&args.iter().map(String::as_str).collect::<Vec<_>>(), repo).unwrap_or_default();
    let found: Vec<(&str, &str)> = out.lines().filter_map(|l| l.split_once('\t')).collect();
    candidates.into_iter().find_map(|c| found.iter().find(|(r, _)| *r == full_name(&c)).map(|(_, oid)| (c, oid.to_string())))
}

/// Every local and remote branch (full refnames), aliases left out, newest commit first, with its author.
struct Listed {
    full: String,
    author: String,
    email: String,
    date: i64, // its newest commit, unix seconds
    subject: String,
    tip: String,
}

fn list_branches(repo: &Path) -> Vec<Listed> {
    let out = read_opt(
        &["for-each-ref", "--sort=-committerdate", "--format=%(refname)%09%(authorname)%09%(authoremail:trim)%09%(committerdate:unix)%09%(objectname)%09%(contents:subject)", "refs/heads", "refs/remotes"],
        repo,
    )
    .unwrap_or_default();
    out.lines()
        .filter_map(|line| {
            let mut f = line.split('\t');
            let (full, author, email, date, tip) = (f.next()?, f.next()?, f.next()?, f.next()?, f.next()?);
            let subject = f.next().unwrap_or_default();
            (!is_alias(short_name(full))).then(|| Listed {
                full: full.into(), author: author.into(), email: email.into(), date: date.parse().unwrap_or(0), subject: subject.into(), tip: tip.into(),
            })
        })
        .collect()
}

/// Whether a branch's newest commit was authored by the identity git commits with in `repo`.
fn mine(repo: &Path) -> impl Fn(&Listed) -> bool {
    // "Ana <ana@x.com> 1700000000 +0100": the effective name and email, wherever they are configured.
    let ident = run(&["var", "GIT_AUTHOR_IDENT"], repo).unwrap_or_default();
    let (name, rest) = ident.split_once(" <").unwrap_or_default();
    let email = rest.split_once('>').map(|(e, _)| e).unwrap_or_default();
    let (name, email) = (name.to_string(), email.to_string());
    move |b: &Listed| {
        let same = |a: &str, b: &str| !b.is_empty() && a.eq_ignore_ascii_case(b);
        same(&b.author, &name) || same(&b.email, &email)
    }
}

/// Local and remote branches (full refnames) whose newest commit is yours.
pub fn my_branches(repo: &Path) -> Vec<String> {
    let is_mine = mine(repo);
    list_branches(repo).into_iter().filter(|b| is_mine(b)).map(|b| b.full).collect()
}

/// A branch with a commit since `since` (unix seconds), newest first, for the dashboard. Named by the local branch when
/// there is one (that's what Commit and Push act on), else by its copy on the server; dated by whichever copy is newest,
/// so a teammate's push shows even if yours is older. `only_mine`: only those whose newest commit is yours. `keep`:
/// branches that show whatever their date (work waiting on this computer).
#[derive(serde::Serialize, Clone)]
pub struct Recent {
    pub name: String, // "x", or "origin/x" when there's no local branch
    pub tip: String,
    pub date: i64,
    pub author: String,
    pub subject: String,
}

pub fn recent(repo: &Path, since: i64, only_mine: bool, keep: &[String]) -> Vec<Recent> {
    let listed = list_branches(repo);
    let is_mine = mine(repo);
    let local: HashSet<&str> = listed.iter().filter_map(|b| b.full.strip_prefix("refs/heads/")).collect();
    let mut seen: HashSet<&str> = HashSet::new();
    listed
        .iter()
        // Sorted newest first: the first copy of a name is its newest.
        .filter(|b| seen.insert(local_name(short_name(&b.full))))
        .filter(|b| keep.iter().any(|k| k == local_name(short_name(&b.full))) || (b.date >= since && (!only_mine || is_mine(b))))
        .map(|b| {
            let short = short_name(&b.full);
            let name = if local.contains(local_name(short)) { local_name(short) } else { short };
            Recent { name: name.into(), tip: b.tip.clone(), date: b.date, author: b.author.clone(), subject: b.subject.clone() }
        })
        .collect()
}

/// Local branches with commits their server copy doesn't have yet: pushed once, committed to since.
pub fn ahead_of_upstream(repo: &Path) -> Vec<String> {
    let out = read_opt(&["for-each-ref", "--format=%(refname:short)%09%(upstream:track)", "refs/heads"], repo).unwrap_or_default();
    out.lines().filter_map(|l| l.split_once('\t')).filter(|(_, track)| track.contains("ahead")).map(|(name, _)| name.to_string()).collect()
}

/// What the graph needs to know about branches besides the log.
pub struct Branches {
    pub local: Vec<String>,        // every local branch, newest first: "Change base"
    pub environments: Vec<String>, // as refs git knows: the local name, else "origin/<name>"
    pub names: Vec<String>,        // every branch name, local or on a remote: the environments picker
    pub on_server: Vec<String>,    // names with a copy on a remote: what Pull can bring
}

/// `chosen`: the user's environments for this repo (by name); None = detected by name. Never the base.
pub fn branches(repo: &Path, base: Option<&str>, chosen: Option<&[String]>) -> Branches {
    let listed = list_branches(repo);
    let local: Vec<String> = listed.iter().filter_map(|b| b.full.strip_prefix("refs/heads/")).map(String::from).collect();
    let local_set: HashSet<&str> = local.iter().map(String::as_str).collect();
    let base = base.map(local_name);
    let is_environment = |name: &str| match chosen {
        Some(chosen) => chosen.iter().any(|c| c == name),
        None => ENVIRONMENT_NAMES.contains(&name.to_lowercase().as_str()),
    };
    let (mut environments, mut names) = (vec![], vec![]);
    let on_server: Vec<String> = listed.iter().filter(|b| b.full.starts_with("refs/remotes/")).map(|b| local_name(short_name(&b.full)).to_string()).collect();
    let mut seen: HashSet<String> = HashSet::new();
    for branch in &listed {
        let short = short_name(&branch.full);
        let name = local_name(short);
        if !seen.insert(name.to_string()) {
            continue; // origin/qa after qa
        }
        names.push(name.to_string());
        if is_environment(name) && Some(name) != base {
            environments.push(if local_set.contains(name) { name.to_string() } else { short.to_string() });
        }
    }
    Branches { local, environments, names, on_server }
}

/// Short names of every branch ("x", "origin/x"), or only those whose tip is in `merged` by history.
pub fn branch_names(repo: &Path, merged: Option<&str>) -> Vec<String> {
    let flag = merged.map(|t| format!("--merged={t}"));
    let mut args = vec!["for-each-ref", "--format=%(refname:short)"];
    args.extend(flag.as_deref());
    args.extend(["refs/heads", "refs/remotes"]);
    read_opt(&args, repo).unwrap_or_default().lines().filter(|r| !is_alias(r)).map(String::from).collect()
}

#[cfg(test)]
mod tests {
    use crate::testutil::Repo;

    #[test]
    fn names_and_base() {
        assert_eq!(super::local_name("origin/feat/x"), "feat/x");
        assert_eq!(super::local_name("feat/x"), "feat/x");
        assert_eq!(super::full_name("origin/qa"), "refs/remotes/origin/qa");
        assert_eq!(super::full_name("qa"), "refs/heads/qa");
        let repo = Repo::new("refs");
        repo.commit("a.txt", "1", "init");
        let tip = repo.git(&["rev-parse", "HEAD"]).trim().to_string();
        // Preferred first when it exists, else main; a missing preferred falls through.
        repo.git(&["branch", "develop"]);
        repo.git(&["branch", "feature/head"]);
        assert_eq!(super::main_ref(repo.path(), Some("develop")), Some(("develop".into(), tip.clone())));
        assert_eq!(super::main_ref(repo.path(), Some("nope")), Some(("main".into(), tip)));
        let mine = super::my_branches(repo.path());
        assert!(mine.contains(&"refs/heads/feature/head".to_string()), "{mine:?}");
        assert_eq!(super::branches(repo.path(), Some("main"), None).environments, ["develop"]);
    }
}
