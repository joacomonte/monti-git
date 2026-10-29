//! Global commit search: a text (e.g. "MOA-854") in commit titles or descriptions, across one or many repos.
use crate::git;
use serde::Serialize;
use std::path::Path;

const PER_REPO: usize = 30;

#[derive(Serialize)]
pub struct Hit {
    pub repo: String,
    pub id: String,
    pub author: String,
    pub date: i64, // unix seconds
    pub subject: String,
    pub snippet: Option<String>, // the description line that matched, when the title didn't
}

/// Newest matches first per repo, every branch (local and remote), case-insensitive, the text taken literally.
/// One thread per repo, so searching all of them takes about as long as the slowest one.
pub fn commits(repos: &[String], query: &str) -> Vec<Hit> {
    std::thread::scope(|scope| {
        let jobs: Vec<_> = repos.iter().map(|repo| scope.spawn(move || in_repo(repo, query))).collect();
        jobs.into_iter().flat_map(|job| job.join().unwrap_or_default()).collect()
    })
}

fn in_repo(repo: &str, query: &str) -> Vec<Hit> {
    let grep = format!("--grep={query}");
    let limit = format!("-n{PER_REPO}");
    let args = ["log", "--all", "-i", "-F", &grep, &limit, "--format=%H%x1f%an%x1f%at%x1f%s%x1f%b%x1e"];
    let Ok(out) = git::run(&args, Path::new(repo)) else { return vec![] };
    let needle = query.to_lowercase();
    out.split('\x1e')
        .filter_map(|record| {
            let mut f = record.trim_start_matches('\n').split('\x1f');
            let (id, author, date, subject, body) = (f.next()?, f.next()?, f.next()?, f.next()?, f.next().unwrap_or(""));
            if id.is_empty() {
                return None;
            }
            let snippet = (!subject.to_lowercase().contains(&needle))
                .then(|| body.lines().find(|l| l.to_lowercase().contains(&needle)).map(|l| l.trim().to_string()))
                .flatten();
            Some(Hit { repo: repo.to_string(), id: id.into(), author: author.into(), date: date.parse().unwrap_or(0), subject: subject.into(), snippet })
        })
        .collect()
}

/// Branches that contain a commit (local names; a remote-only branch keeps its origin/ prefix), for "where is it".
pub fn places(repo: &str, id: &str) -> Vec<String> {
    let Ok(out) = git::run(&["branch", "-a", "--contains", id, "--format=%(refname:short)"], Path::new(repo)) else { return vec![] };
    let names: Vec<&str> = out.lines().filter(|n| !n.ends_with("/HEAD") && *n != "origin" && !n.starts_with('(')).collect();
    let mut result: Vec<String> = Vec::new();
    for name in &names {
        let local = name.split_once('/').filter(|(remote, _)| *remote == "origin" || *remote == "upstream").map(|(_, rest)| rest);
        // origin/qa next to a local qa: one entry.
        if local.is_some_and(|l| names.contains(&l)) {
            continue;
        }
        result.push(name.to_string());
    }
    result
}
