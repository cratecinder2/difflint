use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Context,
    Addition,
    Deletion,
}

impl LineKind {
    pub fn marker(&self) -> char {
        match self {
            LineKind::Context => ' ',
            LineKind::Addition => '+',
            LineKind::Deletion => '-',
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            LineKind::Context => "context",
            LineKind::Addition => "addition",
            LineKind::Deletion => "deletion",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub kind: LineKind,
    pub text: String,
    pub no_newline_at_eof: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    pub old_start: u64,
    pub old_len: u64,
    pub new_start: u64,
    pub new_len: u64,
    pub section_heading: Option<String>,
    pub lines: Vec<Line>,
}

impl Hunk {
    pub fn context_count(&self) -> usize {
        self.lines.iter().filter(|l| l.kind == LineKind::Context).count()
    }

    pub fn addition_count(&self) -> usize {
        self.lines.iter().filter(|l| l.kind == LineKind::Addition).count()
    }

    pub fn deletion_count(&self) -> usize {
        self.lines.iter().filter(|l| l.kind == LineKind::Deletion).count()
    }
}

/// One parent's old-file range in a combined (`diff --cc`) hunk header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OldRange {
    pub start: u64,
    pub len: u64,
}

/// A line inside a combined diff hunk, with one marker per parent instead
/// of the single +/-/space marker a two-file unified diff uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombinedLine {
    pub markers: Vec<LineKind>,
    pub text: String,
    pub no_newline_at_eof: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombinedHunk {
    pub old_ranges: Vec<OldRange>,
    pub new_start: u64,
    pub new_len: u64,
    pub section_heading: Option<String>,
    pub lines: Vec<CombinedLine>,
}

impl CombinedHunk {
    /// Lines that are new relative to every parent (all markers are '+').
    pub fn added_count(&self) -> usize {
        self.lines
            .iter()
            .filter(|l| l.markers.iter().all(|m| *m == LineKind::Addition))
            .count()
    }

    /// Lines dropped from the merge result (any marker is '-').
    pub fn removed_count(&self) -> usize {
        self.lines
            .iter()
            .filter(|l| l.markers.iter().any(|m| *m == LineKind::Deletion))
            .count()
    }

    /// Lines kept in the result but resolved differently against at least
    /// one parent (neither purely new nor dropped).
    pub fn merged_count(&self) -> usize {
        self.lines.len() - self.added_count() - self.removed_count()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileContent {
    Binary,
    Text(Vec<Hunk>),
    Combined { parents: usize, hunks: Vec<CombinedHunk> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    pub old_path: String,
    pub new_path: String,
    pub content: FileContent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ParseError {}

/// Parses one or more unified-diff file sections out of `input`.
///
/// Lines that appear before a "--- " header (git's "diff --git" / "index"
/// preamble, commit messages, mail headers, etc.) are skipped rather than
/// rejected, since real-world diffs are rarely just the bare unified format.
pub fn parse(input: &str) -> Result<Vec<FileDiff>, ParseError> {
    let lines: Vec<&str> = input.lines().collect();
    let mut i = 0usize;
    let mut files = Vec::new();

    while i < lines.len() {
        if lines[i].starts_with("Binary files ") {
            let (old_path, new_path) = parse_binary_marker(lines[i], i + 1)?;
            files.push(FileDiff {
                old_path,
                new_path,
                content: FileContent::Binary,
            });
            i += 1;
            continue;
        }

        if !lines[i].starts_with("--- ") {
            i += 1;
            continue;
        }

        let old_path = strip_header_path(lines[i], "--- ").ok_or_else(|| ParseError {
            line: i + 1,
            message: "malformed --- header".to_string(),
        })?;
        i += 1;

        if i >= lines.len() || !lines[i].starts_with("+++ ") {
            return Err(ParseError {
                line: i,
                message: "expected a +++ header immediately after the --- header".to_string(),
            });
        }
        let new_path = strip_header_path(lines[i], "+++ ").ok_or_else(|| ParseError {
            line: i + 1,
            message: "malformed +++ header".to_string(),
        })?;
        i += 1;

        let at_count = if i < lines.len() { count_leading_ats(lines[i]) } else { 0 };

        if at_count >= 3 {
            let marker = "@".repeat(at_count);
            let num_parents = at_count - 1;
            let mut hunks = Vec::new();

            while i < lines.len() && lines[i].starts_with(&marker) {
                let header_line_no = i + 1;
                let (old_ranges, new_start, new_len, section_heading) =
                    parse_combined_hunk_header(lines[i], header_line_no, num_parents)?;
                i += 1;

                let mut body: Vec<CombinedLine> = Vec::new();
                let mut old_counts = vec![0u64; num_parents];
                let mut new_count = 0u64;

                while i < lines.len() {
                    let line = lines[i];
                    if line.starts_with(&marker) || line.starts_with("--- ") {
                        break;
                    }
                    if line.starts_with('\\') {
                        if let Some(last) = body.last_mut() {
                            last.no_newline_at_eof = true;
                        }
                        i += 1;
                        continue;
                    }
                    let (markers, text): (Vec<LineKind>, &str) = if line.is_empty() {
                        (vec![LineKind::Context; num_parents], "")
                    } else if line.len() >= num_parents
                        && line.as_bytes()[..num_parents]
                            .iter()
                            .all(|&b| b == b' ' || b == b'+' || b == b'-')
                    {
                        let markers = line[..num_parents]
                            .chars()
                            .map(|c| match c {
                                '+' => LineKind::Addition,
                                '-' => LineKind::Deletion,
                                _ => LineKind::Context,
                            })
                            .collect();
                        (markers, &line[num_parents..])
                    } else {
                        break;
                    };

                    let dropped = markers.iter().any(|m| *m == LineKind::Deletion);
                    if !dropped {
                        new_count += 1;
                    }
                    for (idx, m) in markers.iter().enumerate() {
                        if *m != LineKind::Addition {
                            old_counts[idx] += 1;
                        }
                    }

                    body.push(CombinedLine {
                        markers,
                        text: text.to_string(),
                        no_newline_at_eof: false,
                    });
                    i += 1;
                }

                for (idx, range) in old_ranges.iter().enumerate() {
                    if old_counts[idx] != range.len {
                        return Err(ParseError {
                            line: header_line_no,
                            message: format!(
                                "header claims parent {} has -{},{} but the body has {} line(s) from that parent",
                                idx + 1,
                                range.start,
                                range.len,
                                old_counts[idx]
                            ),
                        });
                    }
                }
                if new_count != new_len {
                    return Err(ParseError {
                        line: header_line_no,
                        message: format!(
                            "header claims +{},{} but the body has {} new line(s)",
                            new_start, new_len, new_count
                        ),
                    });
                }

                hunks.push(CombinedHunk {
                    old_ranges,
                    new_start,
                    new_len,
                    section_heading,
                    lines: body,
                });
            }

            files.push(FileDiff {
                old_path,
                new_path,
                content: FileContent::Combined {
                    parents: num_parents,
                    hunks,
                },
            });
            continue;
        }

        let mut hunks = Vec::new();
        while i < lines.len() && lines[i].starts_with("@@ ") {
            let header_line_no = i + 1;
            let (old_start, old_len, new_start, new_len, section_heading) =
                parse_hunk_header(lines[i], header_line_no)?;
            i += 1;

            let mut body: Vec<Line> = Vec::new();
            let mut old_count = 0u64;
            let mut new_count = 0u64;

            while i < lines.len() {
                let line = lines[i];
                if line.starts_with("@@ ") || line.starts_with("--- ") {
                    break;
                }
                if line.starts_with('\\') {
                    if let Some(last) = body.last_mut() {
                        last.no_newline_at_eof = true;
                    }
                    i += 1;
                    continue;
                }
                let (kind, text) = if let Some(t) = line.strip_prefix(' ') {
                    (LineKind::Context, t)
                } else if let Some(t) = line.strip_prefix('+') {
                    (LineKind::Addition, t)
                } else if let Some(t) = line.strip_prefix('-') {
                    (LineKind::Deletion, t)
                } else if line.is_empty() {
                    (LineKind::Context, line)
                } else {
                    break;
                };
                match kind {
                    LineKind::Context => {
                        old_count += 1;
                        new_count += 1;
                    }
                    LineKind::Deletion => old_count += 1,
                    LineKind::Addition => new_count += 1,
                }
                body.push(Line {
                    kind,
                    text: text.to_string(),
                    no_newline_at_eof: false,
                });
                i += 1;
            }

            if old_count != old_len || new_count != new_len {
                return Err(ParseError {
                    line: header_line_no,
                    message: format!(
                        "header claims -{},{} +{},{} but the body has {} old line(s) and {} new line(s)",
                        old_start, old_len, new_start, new_len, old_count, new_count
                    ),
                });
            }

            hunks.push(Hunk {
                old_start,
                old_len,
                new_start,
                new_len,
                section_heading,
                lines: body,
            });
        }

        files.push(FileDiff {
            old_path,
            new_path,
            content: FileContent::Text(hunks),
        });
    }

    Ok(files)
}

/// Counts the run of leading '@' characters, used to tell a normal `@@ `
/// hunk header apart from a combined diff's `@@@ ` (or more, for octopus
/// merges) header.
fn count_leading_ats(line: &str) -> usize {
    line.bytes().take_while(|&b| b == b'@').count()
}

fn strip_header_path(line: &str, prefix: &str) -> Option<String> {
    let rest = line.strip_prefix(prefix)?;
    // Real tools often append a tab and a timestamp after the path.
    let path = rest.split('\t').next().unwrap_or(rest);
    Some(path.to_string())
}

/// Parses git's `Binary files a/x and b/x differ` marker, which stands in
/// place of the usual `--- `/`+++ ` header pair and has no hunks at all.
fn parse_binary_marker(line: &str, line_no: usize) -> Result<(String, String), ParseError> {
    let rest = line.strip_prefix("Binary files ").ok_or_else(|| ParseError {
        line: line_no,
        message: "malformed binary file marker".to_string(),
    })?;
    let rest = rest.strip_suffix(" differ").ok_or_else(|| ParseError {
        line: line_no,
        message: "binary file marker is missing the trailing 'differ'".to_string(),
    })?;
    let sep = rest.find(" and ").ok_or_else(|| ParseError {
        line: line_no,
        message: "binary file marker is missing ' and ' between paths".to_string(),
    })?;
    let old_path = rest[..sep].to_string();
    let new_path = rest[sep + " and ".len()..].to_string();
    Ok((old_path, new_path))
}

fn parse_hunk_header(
    line: &str,
    line_no: usize,
) -> Result<(u64, u64, u64, u64, Option<String>), ParseError> {
    let rest = line.strip_prefix("@@ ").ok_or_else(|| ParseError {
        line: line_no,
        message: "malformed hunk header".to_string(),
    })?;
    let close = rest.find("@@").ok_or_else(|| ParseError {
        line: line_no,
        message: "hunk header is missing its closing '@@'".to_string(),
    })?;
    let ranges = rest[..close].trim();
    let heading = rest[close + 2..].trim();
    let heading = if heading.is_empty() {
        None
    } else {
        Some(heading.to_string())
    };

    let mut parts = ranges.split_whitespace();
    let old_range = parts.next().ok_or_else(|| ParseError {
        line: line_no,
        message: "hunk header is missing the old-file range".to_string(),
    })?;
    let new_range = parts.next().ok_or_else(|| ParseError {
        line: line_no,
        message: "hunk header is missing the new-file range".to_string(),
    })?;
    if parts.next().is_some() {
        return Err(ParseError {
            line: line_no,
            message: "hunk header has unexpected extra tokens".to_string(),
        });
    }

    let (old_start, old_len) = parse_range(old_range, '-', line_no)?;
    let (new_start, new_len) = parse_range(new_range, '+', line_no)?;
    Ok((old_start, old_len, new_start, new_len, heading))
}

/// Parses a combined-diff hunk header, e.g. `@@@ -1,4 -1,4 +1,4 @@@` for a
/// two-parent merge, or with more `@` marks and more `-` ranges for an
/// octopus merge with additional parents.
fn parse_combined_hunk_header(
    line: &str,
    line_no: usize,
    num_parents: usize,
) -> Result<(Vec<OldRange>, u64, u64, Option<String>), ParseError> {
    let marker = "@".repeat(num_parents + 1);
    let prefix = format!("{} ", marker);
    let rest = line.strip_prefix(&prefix).ok_or_else(|| ParseError {
        line: line_no,
        message: "malformed combined hunk header".to_string(),
    })?;
    let close = rest.find(&marker).ok_or_else(|| ParseError {
        line: line_no,
        message: "combined hunk header is missing its closing marker".to_string(),
    })?;
    let ranges = rest[..close].trim();
    let heading = rest[close + marker.len()..].trim();
    let heading = if heading.is_empty() {
        None
    } else {
        Some(heading.to_string())
    };

    let tokens: Vec<&str> = ranges.split_whitespace().collect();
    if tokens.len() != num_parents + 1 {
        return Err(ParseError {
            line: line_no,
            message: format!(
                "expected {} ranges in combined hunk header, found {}",
                num_parents + 1,
                tokens.len()
            ),
        });
    }

    let mut old_ranges = Vec::with_capacity(num_parents);
    for tok in &tokens[..num_parents] {
        let (start, len) = parse_range(tok, '-', line_no)?;
        old_ranges.push(OldRange { start, len });
    }
    let (new_start, new_len) = parse_range(tokens[num_parents], '+', line_no)?;
    Ok((old_ranges, new_start, new_len, heading))
}

fn parse_range(token: &str, expected_sign: char, line_no: usize) -> Result<(u64, u64), ParseError> {
    let body = token.strip_prefix(expected_sign).ok_or_else(|| ParseError {
        line: line_no,
        message: format!("expected a range starting with '{}', got '{}'", expected_sign, token),
    })?;
    let (start_str, len_str) = match body.split_once(',') {
        Some((s, l)) => (s, l),
        None => (body, "1"),
    };
    let start = start_str.parse::<u64>().map_err(|_| ParseError {
        line: line_no,
        message: format!("invalid range start '{}'", start_str),
    })?;
    let len = len_str.parse::<u64>().map_err(|_| ParseError {
        line: line_no,
        message: format!("invalid range length '{}'", len_str),
    })?;
    Ok((start, len))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_single_hunk() {
        let input = "--- a/old.txt\n+++ b/new.txt\n@@ -1,3 +1,3 @@\n line one\n-line two\n+line 2\n line three\n";
        let files = parse(input).expect("should parse");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].old_path, "a/old.txt");
        assert_eq!(files[0].new_path, "b/new.txt");
        match &files[0].content {
            FileContent::Text(hunks) => {
                assert_eq!(hunks.len(), 1);
                assert_eq!(hunks[0].lines.len(), 4);
            }
            other => panic!("expected a text diff, got {:?}", other),
        }
    }

    #[test]
    fn rejects_a_line_count_mismatch() {
        let input = "--- a/old.txt\n+++ b/new.txt\n@@ -1,5 +1,3 @@\n line one\n-line two\n+line 2\n line three\n";
        let err = parse(input).unwrap_err();
        assert!(err.message.contains("old line"));
    }

    #[test]
    fn skips_a_git_preamble() {
        let input = "diff --git a/x b/x\nindex 111..222 100644\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+b\n";
        let files = parse(input).expect("should parse");
        assert_eq!(files[0].old_path, "a/x");
    }

    #[test]
    fn parses_a_binary_file_marker() {
        let input = "diff --git a/logo.png b/logo.png\nindex 111..222 100644\nBinary files a/logo.png and b/logo.png differ\n";
        let files = parse(input).expect("should parse");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].content, FileContent::Binary);
        assert_eq!(files[0].old_path, "a/logo.png");
        assert_eq!(files[0].new_path, "b/logo.png");
    }

    #[test]
    fn parses_a_binary_file_marker_for_a_new_file() {
        let input = "Binary files /dev/null and b/logo.png differ\n";
        let files = parse(input).expect("should parse");
        assert_eq!(files[0].old_path, "/dev/null");
        assert_eq!(files[0].new_path, "b/logo.png");
    }

    #[test]
    fn text_and_binary_file_diffs_can_be_mixed() {
        let input = "--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+b\ndiff --git a/logo.png b/logo.png\nBinary files a/logo.png and b/logo.png differ\n";
        let files = parse(input).expect("should parse");
        assert_eq!(files.len(), 2);
        assert!(matches!(files[0].content, FileContent::Text(_)));
        assert_eq!(files[1].content, FileContent::Binary);
    }

    #[test]
    fn parses_a_combined_diff_for_a_two_parent_merge() {
        let input = "--- a/file.txt\n+++ b/file.txt\n@@@ -1,3 -1,3 +1,3 @@@\n  context line\n +added relative to parent2\n+ added relative to parent1\n- removed relative to parent1\n";
        let files = parse(input).expect("should parse");
        assert_eq!(files.len(), 1);
        match &files[0].content {
            FileContent::Combined { parents, hunks } => {
                assert_eq!(*parents, 2);
                assert_eq!(hunks.len(), 1);
                assert_eq!(hunks[0].old_ranges.len(), 2);
                assert_eq!(hunks[0].lines.len(), 4);
                assert_eq!(hunks[0].added_count(), 0);
                assert_eq!(hunks[0].removed_count(), 1);
                assert_eq!(hunks[0].merged_count(), 3);
            }
            other => panic!("expected a combined diff, got {:?}", other),
        }
    }

    #[test]
    fn rejects_a_combined_diff_with_a_line_count_mismatch() {
        let input = "--- a/file.txt\n+++ b/file.txt\n@@@ -1,9 -1,3 +1,3 @@@\n  context line\n";
        let err = parse(input).unwrap_err();
        assert!(err.message.contains("parent 1"));
    }
}
