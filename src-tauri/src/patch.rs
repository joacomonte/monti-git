//! The one reader of `git diff` output (unified format, `a/` `b/` prefixes pinned by git.rs): files and their hunks.
//! Inside a hunk every line is content, even one that looks like a header ("--- x" is a removed "-- x").

pub struct FilePatch<'a> {
    pub path: String,      // new path; the old one for a deleted file
    pub header: Vec<&'a str>, // "diff --git" … "+++": what `git apply` needs before a hunk
    pub hunks: Vec<Hunk<'a>>,
}

pub struct Hunk<'a> {
    pub header: &'a str, // "@@ -12,7 +12,8 @@ fn x()"
    pub old_start: u32,
    pub new_start: u32,
    pub lines: Vec<&'a str>, // with their " ", "-", "+" or "\" prefix
}

impl Hunk<'_> {
    /// The hunk as `git apply` reads it, after its file's header.
    pub fn text(&self) -> String {
        std::iter::once(self.header).chain(self.lines.iter().copied()).map(|l| format!("{l}\n")).collect()
    }

    /// Every added ("+", new side) and removed ("-", old side) line with its number on that side.
    pub fn changes(&self) -> impl Iterator<Item = (bool, u32, &str)> {
        let (mut old, mut new) = (self.old_start, self.new_start);
        self.lines.iter().filter_map(move |line| {
            let (prefix, text) = line.split_at(line.len().min(1));
            match prefix {
                "-" => { old += 1; Some((true, old - 1, text)) }
                "+" => { new += 1; Some((false, new - 1, text)) }
                " " => { old += 1; new += 1; None }
                _ => None, // "\ No newline at end of file"
            }
        })
    }
}

pub fn parse(diff: &str) -> Vec<FilePatch<'_>> {
    let mut files: Vec<FilePatch> = vec![];
    let mut in_hunk = false;
    for line in diff.lines() {
        if line.starts_with("diff --git ") || files.is_empty() && !line.starts_with("@@") {
            files.push(FilePatch { path: String::new(), header: vec![line], hunks: vec![] });
            in_hunk = false;
            continue;
        }
        if let Some(header) = line.strip_prefix("@@ -") {
            if files.is_empty() {
                files.push(FilePatch { path: String::new(), header: vec![], hunks: vec![] });
            }
            // "@@ -12,7 +12,8 @@": starting line of each side ("-0,0" for a new file).
            let mut sides = header.split(' ');
            let start = |s: Option<&str>| s.and_then(|s| s.trim_start_matches('+').split(',').next()?.parse().ok()).unwrap_or(1);
            let (old_start, new_start) = (start(sides.next()), start(sides.next()));
            files.last_mut().unwrap().hunks.push(Hunk { header: line, old_start, new_start, lines: vec![] });
            in_hunk = true;
            continue;
        }
        let file = files.last_mut().unwrap();
        if in_hunk {
            file.hunks.last_mut().unwrap().lines.push(line);
            continue;
        }
        file.header.push(line);
        // "+++ b/path" ("+++ /dev/null" for a deletion: the "--- a/path" before it names the file).
        let path = line.strip_prefix("+++ b/").or_else(|| line.strip_prefix("--- a/").filter(|_| file.path.is_empty()));
        if let Some(path) = path {
            file.path = path.trim_end_matches('\t').to_string(); // git adds a tab after names with spaces
        }
    }
    files
}

#[cfg(test)]
mod tests {
    #[test]
    fn content_that_looks_like_a_header_stays_content() {
        let diff = "diff --git a/q.sql b/q.sql\nindex 1..2 100644\n--- a/q.sql\n+++ b/q.sql\n@@ -3 +3,2 @@\n--- old comment\n+++ new comment\n+select 1;\ndiff --git a/gone b/gone\ndeleted file mode 100644\n--- a/gone\n+++ /dev/null\n@@ -1 +0,0 @@\n-x\n";
        let files = super::parse(diff);
        assert_eq!(files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), ["q.sql", "gone"]);
        let changes: Vec<_> = files[0].hunks[0].changes().collect();
        assert_eq!(changes, [(true, 3, "-- old comment"), (false, 3, "++ new comment"), (false, 4, "select 1;")]);
        assert_eq!(files[0].header.len(), 4);
    }
}
