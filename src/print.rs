use crate::diff::{FileContent, FileDiff};

pub fn print_human(files: &[FileDiff]) -> String {
    let mut out = String::new();
    for (idx, file) in files.iter().enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        match &file.content {
            FileContent::Binary => {
                out.push_str(&format!(
                    "Binary files {} and {} differ\n",
                    file.old_path, file.new_path
                ));
            }
            FileContent::Text(hunks) => {
                out.push_str(&format!("--- {}\n", file.old_path));
                out.push_str(&format!("+++ {}\n", file.new_path));
                for hunk in hunks {
                    let heading = match &hunk.section_heading {
                        Some(h) => format!(" {}", h),
                        None => String::new(),
                    };
                    out.push_str(&format!(
                        "@@ -{},{} +{},{} @@{} ({} context, {} added, {} removed)\n",
                        hunk.old_start,
                        hunk.old_len,
                        hunk.new_start,
                        hunk.new_len,
                        heading,
                        hunk.context_count(),
                        hunk.addition_count(),
                        hunk.deletion_count(),
                    ));
                    for line in &hunk.lines {
                        out.push(line.kind.marker());
                        out.push_str(&line.text);
                        out.push('\n');
                        if line.no_newline_at_eof {
                            out.push_str("\\ No newline at end of file\n");
                        }
                    }
                }
            }
            FileContent::Combined { parents, hunks } => {
                out.push_str(&format!("--- {}\n", file.old_path));
                out.push_str(&format!("+++ {}\n", file.new_path));
                let marker = "@".repeat(parents + 1);
                for hunk in hunks {
                    let heading = match &hunk.section_heading {
                        Some(h) => format!(" {}", h),
                        None => String::new(),
                    };
                    out.push_str(&marker);
                    for r in &hunk.old_ranges {
                        out.push_str(&format!(" -{},{}", r.start, r.len));
                    }
                    out.push_str(&format!(" +{},{} ", hunk.new_start, hunk.new_len));
                    out.push_str(&marker);
                    out.push_str(&heading);
                    out.push_str(&format!(
                        " ({} merged, {} added, {} removed)\n",
                        hunk.merged_count(),
                        hunk.added_count(),
                        hunk.removed_count(),
                    ));
                    for line in &hunk.lines {
                        for m in &line.markers {
                            out.push(m.marker());
                        }
                        out.push_str(&line.text);
                        out.push('\n');
                        if line.no_newline_at_eof {
                            out.push_str("\\ No newline at end of file\n");
                        }
                    }
                }
            }
        }
    }
    out
}

pub fn print_json(files: &[FileDiff]) -> String {
    let mut out = String::new();
    out.push_str("{\"files\":[");
    for (fi, file) in files.iter().enumerate() {
        if fi > 0 {
            out.push(',');
        }
        out.push('{');
        out.push_str("\"old_path\":");
        out.push_str(&json_string(&file.old_path));
        out.push_str(",\"new_path\":");
        out.push_str(&json_string(&file.new_path));
        match &file.content {
            FileContent::Binary => {
                out.push_str(",\"type\":\"binary\"");
            }
            FileContent::Text(hunks) => {
                out.push_str(",\"type\":\"text\",\"hunks\":[");
                for (hi, hunk) in hunks.iter().enumerate() {
                    if hi > 0 {
                        out.push(',');
                    }
                    out.push_str(&format!(
                        "{{\"old_start\":{},\"old_len\":{},\"new_start\":{},\"new_len\":{},\"section_heading\":",
                        hunk.old_start, hunk.old_len, hunk.new_start, hunk.new_len
                    ));
                    match &hunk.section_heading {
                        Some(h) => out.push_str(&json_string(h)),
                        None => out.push_str("null"),
                    }
                    out.push_str(",\"lines\":[");
                    for (li, line) in hunk.lines.iter().enumerate() {
                        if li > 0 {
                            out.push(',');
                        }
                        out.push_str(&format!(
                            "{{\"kind\":\"{}\",\"text\":{},\"no_newline_at_eof\":{}}}",
                            line.kind.as_str(),
                            json_string(&line.text),
                            line.no_newline_at_eof
                        ));
                    }
                    out.push_str("]}");
                }
                out.push(']');
            }
            FileContent::Combined { parents, hunks } => {
                out.push_str(&format!(",\"type\":\"combined\",\"parents\":{},\"hunks\":[", parents));
                for (hi, hunk) in hunks.iter().enumerate() {
                    if hi > 0 {
                        out.push(',');
                    }
                    out.push_str("{\"old_ranges\":[");
                    for (ri, r) in hunk.old_ranges.iter().enumerate() {
                        if ri > 0 {
                            out.push(',');
                        }
                        out.push_str(&format!("{{\"start\":{},\"len\":{}}}", r.start, r.len));
                    }
                    out.push_str(&format!(
                        "],\"new_start\":{},\"new_len\":{},\"section_heading\":",
                        hunk.new_start, hunk.new_len
                    ));
                    match &hunk.section_heading {
                        Some(h) => out.push_str(&json_string(h)),
                        None => out.push_str("null"),
                    }
                    out.push_str(",\"lines\":[");
                    for (li, line) in hunk.lines.iter().enumerate() {
                        if li > 0 {
                            out.push(',');
                        }
                        out.push_str("{\"markers\":[");
                        for (mi, m) in line.markers.iter().enumerate() {
                            if mi > 0 {
                                out.push(',');
                            }
                            out.push_str(&json_string(m.as_str()));
                        }
                        out.push_str(&format!(
                            "],\"text\":{},\"no_newline_at_eof\":{}}}",
                            json_string(&line.text),
                            line.no_newline_at_eof
                        ));
                    }
                    out.push_str("]}");
                }
                out.push(']');
            }
        }
        out.push('}');
    }
    out.push_str("]}");
    out
}

fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::parse;

    #[test]
    fn json_output_is_well_formed_enough_to_round_trip_strings() {
        let input = "--- a/f\n+++ b/f\n@@ -1 +1 @@\n-old \"quoted\"\n+new\n";
        let files = parse(input).unwrap();
        let json = print_json(&files);
        assert!(json.contains("\\\"quoted\\\""));
        assert!(json.starts_with("{\"files\":["));
    }

    #[test]
    fn binary_files_print_as_a_single_line_with_no_hunks() {
        let input = "Binary files a/logo.png and b/logo.png differ\n";
        let files = parse(input).unwrap();
        assert_eq!(
            print_human(&files),
            "Binary files a/logo.png and b/logo.png differ\n"
        );
        assert!(print_json(&files).contains("\"type\":\"binary\""));
    }

    #[test]
    fn combined_diffs_print_multi_column_markers() {
        let input = "--- a/file.txt\n+++ b/file.txt\n@@@ -1,3 -1,3 +1,3 @@@\n  context line\n +added relative to parent2\n+ added relative to parent1\n- removed relative to parent1\n";
        let files = parse(input).unwrap();
        let text = print_human(&files);
        assert!(text.contains("@@@ -1,3 -1,3 +1,3 @@@"));
        assert!(text.contains("- removed relative to parent1"));

        let json = print_json(&files);
        assert!(json.contains("\"type\":\"combined\""));
        assert!(json.contains("\"parents\":2"));
    }
}
