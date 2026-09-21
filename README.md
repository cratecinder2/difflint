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
{"files":[{"old_path":"a/greeting.txt","new_path":"b/greeting.txt","type":"text","hunks":[{"old_start":1,"old_len":3,"new_start":1,"new_len":3,"section_heading":null,"lines":[{"kind":"context","text":"Hello there,","no_newline_at_eof":false},{"kind":"deletion","text":"old line","no_newline_at_eof":false},{"kind":"addition","text":"new line","no_newline_at_eof":false},{"kind":"context","text":"Goodbye.","no_newline_at_eof":false}]}]}]}
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

Binary files show up in git diffs as a single marker line instead of a
`--- `/`+++ ` pair and hunks:

```
$ difflint binary.diff
Binary files a/logo.png and b/logo.png differ
```

which becomes `{"old_path":"a/logo.png","new_path":"b/logo.png","type":"binary"}`
in `--json` mode.

Merge commits produce combined diffs (`git diff --cc` / `git show` on a merge
commit), where each hunk header lists one old-file range per parent and each
line has one marker column per parent instead of a single `+`/`-`/space:

```
$ difflint merge.diff
--- a/describe.c
+++ b/describe.c
@@@ -98,3 -98,3 +98,3 @@@ (1 merged, 0 added, 1 removed)
  return (a_date > b_date) ? -1 : (a_date == b_date) ? 0 : 1;
- static void describe(struct commit *cmit, int last_one)
++static void describe(char *arg, int last_one)
```

difflint validates each parent's old-file line count against the marker
column for that parent, same as it validates a normal two-file hunk.

## Building

Standard `cargo build` / `cargo run` / `cargo test`. No external crates.

## Status

Single-file unified diffs (additions, deletions, context lines, "no newline
at end of file" markers), binary file markers, and combined diffs from merge
commits (including octopus merges with more than two parents) all parse and
validate.

## License

MIT, see [LICENSE](LICENSE).
