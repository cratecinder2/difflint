# difflint

A validating parser and pretty printer for unified text diffs.

`diff -u` and `git diff` both produce the "unified diff" format, but most
tools that consume it just trust the hunk headers (the `@@ -1,4 +1,5 @@`
lines) without checking that the line counts they promise actually match the
lines that follow. When a diff gets hand-edited, truncated by a flaky
pipeline, or mangled by copy-paste, those counts drift and downstream tools
either crash confusingly or silently apply the wrong lines. difflint parses
the format properly, checks the counts, and tells you exactly which hunk and
line disagree.

## Usage

Given a diff like this, saved as `example.diff`:

```diff
--- a/greeting.txt
+++ b/greeting.txt
@@ -1,3 +1,3 @@
 Hello there,
-old line
+new line
 Goodbye.
```

Pretty-print it:

```
$ difflint example.diff
--- a/greeting.txt
+++ b/greeting.txt
@@ -1,3 +1,3 @@ (2 context, 1 added, 1 removed)
 Hello there,
-old line
+new line
 Goodbye.
```

Or ask for JSON, for feeding into another program:

```
$ difflint --json example.diff
{"files":[{"old_path":"a/greeting.txt","new_path":"b/greeting.txt","hunks":[{"old_start":1,"old_len":3,"new_start":1,"new_len":3,"section_heading":null,"lines":[{"kind":"context","text":"Hello there,","no_newline_at_eof":false},{"kind":"deletion","text":"old line","no_newline_at_eof":false},{"kind":"addition","text":"new line","no_newline_at_eof":false},{"kind":"context","text":"Goodbye.","no_newline_at_eof":false}]}]}]}
```

Input also comes from stdin if you omit the file argument, or pass `-`
explicitly:

```
$ git diff | difflint --json
```

If a hunk header's claimed counts don't match its body, difflint refuses to
guess and reports the line number instead:

```
$ difflint broken.diff
parse error: line 3: header claims -1,5 +1,3 but the body has 3 old line(s) and 3 new line(s)
```

difflint also tolerates the preamble lines git adds (`diff --git ...`,
`index ...`) before the `--- ` / `+++ ` header pair, so real `git diff`
output works as-is.

## Building

Standard `cargo build` / `cargo run` / `cargo test`. No external crates.

## Status

Early skeleton: single-file unified diffs with additions, deletions, context
lines, and "no newline at end of file" markers all parse and validate. Not
yet handled: three-way diffs, binary file markers, and combined diffs (`diff
--cc`).

## License

MIT, see [LICENSE](LICENSE).
