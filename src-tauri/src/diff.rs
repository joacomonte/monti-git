//! A file's diff as side-by-side rows, ported from SideBySide in DiffView.swift.
use crate::patch::Hunk;
use serde::Serialize;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Side {
    pub number: u32,
    pub text: String,
    /// Differing middle part as UTF-16 offsets [start, end), so JS `slice` can use them directly.
    pub changed: Option<(usize, usize)>,
}

/// A line pair, or (with `hunk`) the "@@ … @@" separator that starts a block.
#[derive(Serialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DiffRow {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hunk: Option<usize>, // position among the file's hunks: what "Discard this block" sends back
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header: Option<String>, // the "@@ … @@" line itself
    pub left: Option<Side>,
    pub right: Option<Side>,
    pub is_change: bool,
}

/// Deletions followed by additions are paired line by line, and each pair gets the differing middle part marked.
pub fn rows(hunks: &[Hunk]) -> Vec<DiffRow> {
    let mut rows = vec![];
    for (index, hunk) in hunks.iter().enumerate() {
        rows.push(DiffRow { hunk: Some(index), header: Some(hunk.header.to_string()), ..Default::default() });
        let mut pending = Pending { old: hunk.old_start, new: hunk.new_start, removed: vec![], added: vec![] };
        for line in &hunk.lines {
            if let Some(text) = line.strip_prefix('-') {
                if !pending.added.is_empty() {
                    pending.flush(&mut rows);
                }
                pending.removed.push(text);
            } else if let Some(text) = line.strip_prefix('+') {
                pending.added.push(text);
            } else if let Some(text) = line.strip_prefix(' ') {
                pending.flush(&mut rows);
                let side = |number| Some(Side { number, text: text.into(), changed: None });
                rows.push(DiffRow { left: side(pending.old), right: side(pending.new), ..Default::default() });
                pending.old += 1;
                pending.new += 1;
            } // "\ No newline at end of file" is skipped
        }
        pending.flush(&mut rows);
    }
    rows
}

/// A file shown whole (unchanged in the selection): one column, every line numbered.
pub fn whole(text: &str) -> Vec<DiffRow> {
    text.lines()
        .zip(1..)
        .map(|(line, number)| DiffRow { right: Some(Side { number, text: line.into(), changed: None }), ..Default::default() })
        .collect()
}

/// A file not in git yet: every line added.
pub fn added(text: &str) -> Vec<DiffRow> {
    let mut rows = vec![DiffRow { hunk: Some(0), header: Some(format!("@@ -0,0 +1,{} @@ new file", crate::git::line_count(text))), ..Default::default() }];
    rows.extend(text.lines().zip(1..).map(|(line, number)| DiffRow {
        right: Some(Side { number, text: line.into(), changed: None }),
        is_change: true,
        ..Default::default()
    }));
    rows
}

/// Removed and added lines waiting to be paired, and where each side's numbering is.
struct Pending<'a> {
    old: u32,
    new: u32,
    removed: Vec<&'a str>,
    added: Vec<&'a str>,
}

impl Pending<'_> {
    fn flush(&mut self, rows: &mut Vec<DiffRow>) {
        for i in 0..self.removed.len().max(self.added.len()) {
            let side = |lines: &[&str], start: u32| lines.get(i).map(|t| Side { number: start + i as u32, text: t.to_string(), changed: None });
            let (mut left, mut right) = (side(&self.removed, self.old), side(&self.added, self.new));
            if let (Some(l), Some(r)) = (left.as_mut(), right.as_mut()) {
                (l.changed, r.changed) = changed_ranges(&l.text, &r.text);
            }
            rows.push(DiffRow { left, right, is_change: true, ..Default::default() });
        }
        self.old += self.removed.len() as u32;
        self.new += self.added.len() as u32;
        self.removed.clear();
        self.added.clear();
    }
}

/// Common prefix/suffix trimming: what is left in the middle of each line is what changed.
fn changed_ranges(old: &str, new: &str) -> (Option<(usize, usize)>, Option<(usize, usize)>) {
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
    (Some((utf16(&a[..start]), utf16(&a[..end_a]))), Some((utf16(&b[..start]), utf16(&b[..end_b]))))
}

#[cfg(test)]
mod tests {
    #[test]
    fn pairs_and_marks() {
        let diff = "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1,3 +1,3 @@\n same\n-let a = 1\n+let a = 2\n end";
        let files = crate::patch::parse(diff);
        let rows = super::rows(&files[0].hunks);
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].hunk, Some(0));
        assert_eq!(rows[2].left.as_ref().unwrap().changed, Some((8, 9)));
        assert_eq!(rows[2].right.as_ref().unwrap().number, 2);
        assert_eq!(rows[3].left.as_ref().unwrap().number, 3);
    }
}
