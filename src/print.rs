use crate::diff::FileDiff;

pub fn print_human(files: &[FileDiff]) -> String {
    let mut out = String::new();
    for (idx, file) in files.iter().enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        if file.is_binary {
            out.push_str(&format!(
                "Binary files {} and {} differ\n",
                file.old_path, file.new_path
            ));
            continue;
        }
        out.push_str(&format!("--- {}\n", file.old_path));
        out.push_str(&format!("+++ {}\n", file.new_path));
        for hunk in &file.hunks {
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
        out.push_str(",\"is_binary\":");
        out.push_str(if file.is_binary { "true" } else { "false" });
        out.push_str(",\"hunks\":[");
        for (hi, hunk) in file.hunks.iter().enumerate() {
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
        out.push_str("]}");
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
        assert!(print_json(&files).contains("\"is_binary\":true,\"hunks\":[]"));
    }
}
