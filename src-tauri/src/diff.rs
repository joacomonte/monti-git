//! Unified `git diff` of one file -> side-by-side rows, ported from SideBySide in DiffView.swift.
use serde::Serialize;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Side {
    pub number: usize,
    pub text: String,
    /// Differing middle part as UTF-16 offsets [start, end), so JS `slice` can use them directly.
    pub changed: Option<(usize, usize)>,
}

#[derive(Serialize, Default, Debug)]
pub struct DiffRow {
    #[serde(rename = "hunk", skip_serializing_if = "Option::is_none")]
    pub hunk: Option<usize>, // set on "@@ … @@" separator rows: position among the file's hunks
    pub left: Option<Side>,
    pub right: Option<Side>,
    #[serde(rename = "isChange")]
    pub is_change: bool,
}

/// Deletions followed by additions are paired line by line, and each pair gets the differing middle part marked.
pub fn rows(diff: &str) -> Vec<DiffRow> {
    let mut rows = vec![];
    let (mut old_line, mut new_line) = (0usize, 0usize);
    let (mut removed, mut added): (Vec<&str>, Vec<&str>) = (vec![], vec![]);

    let flush = |rows: &mut Vec<DiffRow>, removed: &mut Vec<&str>, added: &mut Vec<&str>, old_line: &mut usize, new_line: &mut usize| {
        for i in 0..removed.len().max(added.len()) {
            let mut left = removed.get(i).map(|t| Side { number: *old_line + i, text: t.to_string(), changed: None });
            let mut right = added.get(i).map(|t| Side { number: *new_line + i, text: t.to_string(), changed: None });
            if let (Some(l), Some(r)) = (left.as_mut(), right.as_mut()) {
                let (a, b) = changed_ranges(&l.text, &r.text);
                l.changed = Some(a);
                r.changed = Some(b);
            }
            rows.push(DiffRow { left, right, is_change: true, ..Default::default() });
        }
        *old_line += removed.len();
        *new_line += added.len();
        removed.clear();
        added.clear();
    };

    let mut in_hunk = false;
    let mut hunks = 0;
    for line in diff.split('\n') {
        if line.starts_with("@@") {
            flush(&mut rows, &mut removed, &mut added, &mut old_line, &mut new_line);
            // "@@ -12,7 +12,8 @@ func x()": starting line numbers of each side.
            let numbers: Vec<usize> = line
                .split(' ')
                .skip(1)
                .take(2)
                .map(|n| n.get(1..).and_then(|n| n.split(',').next()).and_then(|n| n.parse().ok()).unwrap_or(1))
                .collect();
            old_line = numbers.first().copied().unwrap_or(1);
            new_line = numbers.last().copied().unwrap_or(1);
            in_hunk = true;
            rows.push(DiffRow { hunk: Some(hunks), right: Some(Side { number: 0, text: line.into(), changed: None }), ..Default::default() });
            hunks += 1;
        } else if line.starts_with("diff --git") {
            flush(&mut rows, &mut removed, &mut added, &mut old_line, &mut new_line);
            in_hunk = false;
        } else if !in_hunk {
            continue; // file header: index, ---/+++
        } else if let Some(text) = line.strip_prefix('-') {
            if !added.is_empty() {
                flush(&mut rows, &mut removed, &mut added, &mut old_line, &mut new_line);
            }
            removed.push(text);
        } else if let Some(text) = line.strip_prefix('+') {
            added.push(text);
        } else if let Some(text) = line.strip_prefix(' ') {
            flush(&mut rows, &mut removed, &mut added, &mut old_line, &mut new_line);
            let side = |n| Some(Side { number: n, text: text.into(), changed: None });
            rows.push(DiffRow { left: side(old_line), right: side(new_line), ..Default::default() });
            old_line += 1;
            new_line += 1;
        }
        // "\ No newline at end of file" and blank trailing lines are skipped.
    }
    flush(&mut rows, &mut removed, &mut added, &mut old_line, &mut new_line);
    rows
}

/// Common prefix/suffix trimming: what is left in the middle of each line is what changed.
fn changed_ranges(old: &str, new: &str) -> ((usize, usize), (usize, usize)) {
    let (a, b): (Vec<char>, Vec<char>) = (old.chars().collect(), new.chars().collect());
    let mut start = 0;
    while start < a.len() && start < b.len() && a[start] == b[start] {
        start += 1;
    }
    let (mut end_a, mut end_b) = (a.len(), b.len());
    while end_a > start && end_b > start && a[end_a - 1] == b[end_b - 1] {
        end_a -= 1;
        end_b -= 1;
    }
    let utf16 = |chars: &[char]| chars.iter().map(|c| c.len_utf16()).sum::<usize>();
    ((utf16(&a[..start]), utf16(&a[..end_a])), (utf16(&b[..start]), utf16(&b[..end_b])))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairs_and_marks() {
        let diff = "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1,3 +1,3 @@\n same\n-let a = 1\n+let a = 2\n end";
        let rows = rows(diff);
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].hunk, Some(0));
        assert_eq!(rows[2].left.as_ref().unwrap().changed, Some((8, 9)));
        assert_eq!(rows[2].right.as_ref().unwrap().number, 2);
        assert_eq!(rows[3].left.as_ref().unwrap().number, 3);
    }
}
