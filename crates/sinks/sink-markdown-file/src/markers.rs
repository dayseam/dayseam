//! Marker-block parser + splicer.
//!
//! The on-disk marker-block contract is deliberately small so a human
//! reading the markdown file in any editor can understand what the
//! sink did without tooling:
//!
//! ```text
//! <!-- dayseam:begin date="2026-04-18" run_id="…" template="dayseam.dev_eod" version="2026-04-18" -->
//! ## Commits
//!
//! - **repo** — summary _1 commit_
//! <!-- dayseam:end -->
//! ```
//!
//! ## Guarantees
//!
//! - `render(parse(x))` is byte-identical to `x` for any well-formed
//!   input. Unit test: `parse_then_render_round_trips_well_formed_files`.
//! - `splice` replaces only the block whose `date` matches the new
//!   block's `date`; every other byte of the document (user prose,
//!   other blocks, trailing newlines) is preserved verbatim. Unit
//!   test: `splice_preserves_surrounding_prose_byte_for_byte`.
//! - A malformed begin/end pair (overlapping blocks, begin without
//!   end, end without begin, missing required attribute) returns
//!   [`MarkerError`] **without mutating** the input. Unit test:
//!   `malformed_begin_or_end_is_rejected`.
//! - **Fenced code blocks are opaque to marker recognition** (DAY-187).
//!   A line whose trimmed form is `<!-- dayseam:begin … -->` or
//!   `<!-- dayseam:end -->` inside a fenced code block is treated as
//!   prose / body content, never as a real begin/end marker. This
//!   protects the contract against a user pasting Dayseam's own
//!   marker-syntax illustration into their journal — without the
//!   guard, the sink would rewrite the user's example on the next
//!   save. Unit test: `fenced_marker_syntax_inside_prose_is_not_a_marker`.
//! - **Empty marker bodies round-trip without an injected blank line**
//!   (DAY-187). `render(parse("<begin>\n<end>\n"))` is byte-identical
//!   to the input. Unit test: `empty_block_round_trips_without_blank_line`.
//! - **Marker-line line endings are preserved** (DAY-187). When the
//!   parsed file's marker lines were CRLF-terminated, the rendered
//!   output emits CRLF on its marker lines too. Without this guard,
//!   Windows users see every marker line drift from CRLF → LF on the
//!   first save. Unit test:
//!   `crlf_marker_lines_round_trip_preserved`.
//!
//! ## Deliberate non-goals
//!
//! - No regex dependency. A line-oriented state machine handles the
//!   whitespace-tolerant parse in ~40 lines and makes the fuzz target
//!   in Task 8 trivial.
//! - No partial parse. A file with one malformed block fails the whole
//!   read; the sink then returns `SINK_MALFORMED_MARKER` and refuses
//!   to write. Refusing loudly is safer than rewriting a file whose
//!   structure we don't fully understand.

use chrono::NaiveDate;
use thiserror::Error;

/// Sentinel line that opens a marker block. Callers match against this
/// after trimming leading/trailing whitespace from a line.
pub(crate) const BEGIN_PREFIX: &str = "<!-- dayseam:begin ";
pub(crate) const BEGIN_SUFFIX: &str = " -->";
pub(crate) const END_MARKER: &str = "<!-- dayseam:end -->";

/// Line terminator observed on a [`Block`]'s marker lines.
///
/// DAY-187 audit follow-up: previously the renderer always emitted
/// `\n` for the begin and end marker lines. On a Windows-saved file
/// with CRLF marker lines, the round-trip silently rewrote them to
/// LF on every save and the user's git diff showed every marker
/// line as a CRLF→LF change after the first save. Tracking the
/// observed terminator per block (rather than at the doc level)
/// keeps mixed-line-ending files honest without hand-rolling a
/// dominant-line-ending heuristic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum LineEnding {
    #[default]
    Lf,
    CrLf,
}

impl LineEnding {
    /// Observe the line terminator of `raw_line`. The `raw_line` is
    /// the slice [`split_lines_preserving_newlines`] yielded — i.e.
    /// the line including its trailing terminator (or the file's
    /// final, terminator-less tail).
    fn observe(raw_line: &str) -> LineEnding {
        if raw_line.ends_with("\r\n") {
            LineEnding::CrLf
        } else {
            LineEnding::Lf
        }
    }

    /// Render this terminator into a string buffer. Called by
    /// [`render_block_into`] for the begin and end marker lines.
    fn push_into(self, out: &mut String) {
        match self {
            LineEnding::Lf => out.push('\n'),
            LineEnding::CrLf => out.push_str("\r\n"),
        }
    }

    /// `true` iff this is the `\r\n` variant. Used by the renderer
    /// to decide whether the block body — which is preserved
    /// byte-for-byte — should be padded with CRLF or LF before the
    /// closing marker when its trailing terminator is missing.
    fn is_crlf(self) -> bool {
        matches!(self, LineEnding::CrLf)
    }
}

/// Attributes on the begin marker. The parser is strict about presence
/// (all four are required) and tolerant about order, interior
/// whitespace, and attribute-value character set (the values are ASCII
/// in practice: UUIDs, ISO dates, dotted template ids).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MarkerAttrs {
    pub date: NaiveDate,
    pub run_id: String,
    pub template: String,
    pub version: String,
}

/// One parsed segment of a document — either raw user prose (kept as
/// owned `String` so downstream splicing doesn't need lifetime juggling)
/// or a recognised marker block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Segment {
    /// Arbitrary markdown or text the user wrote between (or outside)
    /// marker blocks. Preserved byte-for-byte including its trailing
    /// newline.
    Prose(String),
    /// One recognised `<!-- dayseam:begin ... --> ... <!-- dayseam:end -->`
    /// block.
    Block(Block),
}

/// A single parsed marker block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Block {
    pub attrs: MarkerAttrs,
    /// Content between the begin and end markers, excluding the marker
    /// lines themselves but including the trailing newline of the last
    /// body line.
    pub body: String,
    /// Line terminator observed on the begin / end marker lines when
    /// this block was parsed. New blocks constructed at render time
    /// (the orchestrator's freshly-rendered draft) default to LF; on
    /// splice we copy the existing block's terminator forward so the
    /// rewritten output keeps the file's per-marker-line ending shape.
    /// See [`LineEnding`] for the rationale.
    pub line_ending: LineEnding,
}

/// Parsed view of a markdown file, segmented into prose and marker
/// blocks in source order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ParsedDoc {
    pub segments: Vec<Segment>,
}

/// Reasons the parser or splicer rejected an input. Every variant maps
/// to `DayseamError::Internal { code: SINK_MALFORMED_MARKER, .. }` at
/// the adapter boundary so the UI shows a single, stable error code
/// regardless of the specific malformation.
#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum MarkerError {
    /// A `dayseam:begin` line was seen while another block was still
    /// open. Nested blocks are a structural error; we refuse to guess
    /// which one was intended to close first.
    #[error("nested dayseam:begin at line {line} (another block is still open)")]
    NestedBegin { line: usize },
    /// The file ended before an open block was closed.
    #[error("unclosed dayseam:begin starting at line {line}")]
    UnclosedBegin { line: usize },
    /// A `dayseam:end` line was seen without a preceding begin.
    #[error("dangling dayseam:end at line {line} (no matching begin)")]
    DanglingEnd { line: usize },
    /// A begin line was found but a required attribute
    /// (`date`, `run_id`, `template`, `version`) was missing or
    /// malformed.
    #[error("malformed dayseam:begin at line {line}: {detail}")]
    MalformedBegin { line: usize, detail: String },
}

impl MarkerError {
    /// Human-readable one-liner suitable for an `Internal.message`
    /// field. The adapter wraps this in `DayseamError::Internal { code:
    /// SINK_MALFORMED_MARKER, .. }`; the `Display` impl above is what
    /// ends up in the `message` field.
    pub(crate) fn describe(&self) -> String {
        self.to_string()
    }
}

/// Parse `text` into a segmented document. Returns [`MarkerError`] on
/// any structural problem; in that case the document is untouched (the
/// function never writes anywhere), so the caller can safely refuse
/// the write and leave the file on disk as-is.
pub(crate) fn parse(text: &str) -> Result<ParsedDoc, MarkerError> {
    let mut segments: Vec<Segment> = Vec::new();
    let mut prose_buf = String::new();
    // (attrs, body, start_line, line_ending observed on the begin line)
    let mut open: Option<(MarkerAttrs, String, usize, LineEnding)> = None;
    // DAY-187: track fenced-code-block nesting so a `<!-- dayseam:begin
    // … -->` line inside a fenced code block (the user's own
    // illustration of Dayseam's syntax in their daily-note prose, or
    // an inert example inside a block body) is treated as prose / body
    // rather than a real marker. The fence sentinel records the exact
    // ASCII run that opened the fence so an inner indented run cannot
    // accidentally close the outer fence.
    let mut fence: Option<FenceSentinel> = None;

    for (idx, line) in split_lines_preserving_newlines(text).enumerate() {
        let line_no = idx + 1;
        let trimmed = line.trim_end_matches(['\r', '\n']).trim();

        // DAY-187 fence-toggle pre-check: a line that opens or closes
        // a fence is itself just prose / body content, never a
        // marker — even if the fence delimiter happens to appear on
        // the same line as a begin/end marker (which our renderer
        // never emits, but a hand-edited file could carry).
        if let Some(active) = fence {
            // Inside a fence: only the matching closing-fence line
            // can pop us out. Markdown's contract is that the
            // closing fence is the same character with at least the
            // same count; we accept any equal-or-larger run of the
            // same delimiter here.
            if active.line_closes(trimmed) {
                fence = None;
            }
            // Treat this line as body / prose verbatim.
            if let Some((_, body, _, _)) = open.as_mut() {
                body.push_str(line);
            } else {
                prose_buf.push_str(line);
            }
            continue;
        }

        if let Some(opened) = FenceSentinel::detect_open(trimmed) {
            fence = Some(opened);
            // Opening fence line itself is body / prose.
            if let Some((_, body, _, _)) = open.as_mut() {
                body.push_str(line);
            } else {
                prose_buf.push_str(line);
            }
            continue;
        }

        if let Some((attrs, ref mut body, start_line, _line_ending)) = open.as_mut() {
            if is_end_marker(trimmed) {
                let block = Block {
                    attrs: attrs.clone(),
                    body: std::mem::take(body),
                    line_ending: *_line_ending,
                };
                segments.push(Segment::Block(block));
                open = None;
                continue;
            }
            if is_begin_marker(trimmed) {
                return Err(MarkerError::NestedBegin { line: line_no });
            }
            // Everything else inside an open block is body content.
            body.push_str(line);
            // Silence "unused" warning on start_line: we remember it
            // solely so the `UnclosedBegin` reported below is actionable.
            let _ = start_line;
            continue;
        }

        if is_end_marker(trimmed) {
            return Err(MarkerError::DanglingEnd { line: line_no });
        }

        if is_begin_marker(trimmed) {
            if !prose_buf.is_empty() {
                segments.push(Segment::Prose(std::mem::take(&mut prose_buf)));
            }
            let attrs = parse_begin_attrs(trimmed, line_no)?;
            let observed = LineEnding::observe(line);
            open = Some((attrs, String::new(), line_no, observed));
            continue;
        }

        prose_buf.push_str(line);
    }

    if let Some((_attrs, _body, start_line, _le)) = open {
        return Err(MarkerError::UnclosedBegin { line: start_line });
    }
    if !prose_buf.is_empty() {
        segments.push(Segment::Prose(prose_buf));
    }

    Ok(ParsedDoc { segments })
}

/// Append `new_block` or replace the existing block whose
/// [`MarkerAttrs::date`] matches. The rest of the document is
/// preserved byte-for-byte. Returns `true` if an existing block was
/// replaced, `false` if the block was appended.
///
/// DAY-187: when replacing an existing block, the per-block line
/// ending is copied from the old block onto the new one so a
/// CRLF-bearing file does not silently flip to LF on save. New
/// blocks appended to a fresh file inherit the new block's
/// caller-supplied terminator (LF by default — the orchestrator's
/// freshly-rendered draft is LF on every platform).
pub(crate) fn splice(doc: &mut ParsedDoc, mut new_block: Block) -> bool {
    for seg in doc.segments.iter_mut() {
        if let Segment::Block(existing) = seg {
            if existing.attrs.date == new_block.attrs.date {
                // Preserve the existing block's marker-line ending so
                // a CRLF Windows file stays CRLF after rewrite.
                new_block.line_ending = existing.line_ending;
                *existing = new_block;
                return true;
            }
        }
    }

    // Appending: make sure there is a newline separating the previous
    // content from the new block so the file stays well-formed for any
    // downstream markdown reader.
    ensure_trailing_newline(&mut doc.segments);
    doc.segments.push(Segment::Block(new_block));
    false
}

/// Render a [`ParsedDoc`] back to a `String`. Round-trips well-formed
/// documents byte-for-byte.
pub(crate) fn render(doc: &ParsedDoc) -> String {
    let mut out = String::new();
    for seg in &doc.segments {
        match seg {
            Segment::Prose(p) => out.push_str(p),
            Segment::Block(b) => render_block_into(&mut out, b),
        }
    }
    out
}

fn render_block_into(out: &mut String, block: &Block) {
    out.push_str(BEGIN_PREFIX);
    out.push_str(&format!("date=\"{}\"", block.attrs.date));
    out.push_str(&format!(" run_id=\"{}\"", block.attrs.run_id));
    out.push_str(&format!(" template=\"{}\"", block.attrs.template));
    out.push_str(&format!(" version=\"{}\"", block.attrs.version));
    out.push_str(BEGIN_SUFFIX);
    block.line_ending.push_into(out);
    out.push_str(&block.body);
    // DAY-187: only force a separator newline when the body is
    // non-empty AND does not already end in one. The previous
    // shape (`!body.ends_with('\n') -> push('\n')`) injected an
    // extra newline for empty bodies because `"" ends_with('\n')`
    // is false. The empty-body round-trip test
    // (`empty_block_round_trips_without_blank_line`) pins this
    // invariant.
    if !block.body.is_empty() && !ends_with_compatible_newline(&block.body, block.line_ending) {
        block.line_ending.push_into(out);
    }
    out.push_str(END_MARKER);
    block.line_ending.push_into(out);
}

/// `true` if `body` already ends in a line terminator that is
/// compatible with the marker line's ending. We accept either `\n`
/// or `\r\n` for an LF-terminated marker (a body line ending in
/// CRLF is still a valid line break before the LF closing marker)
/// and require `\r\n` for a CRLF-terminated marker (otherwise the
/// closing marker line would appear glued onto a body line, which
/// markdown readers would render as a heading-paragraph collision).
fn ends_with_compatible_newline(body: &str, le: LineEnding) -> bool {
    if le.is_crlf() {
        body.ends_with("\r\n")
    } else {
        body.ends_with('\n')
    }
}

fn is_begin_marker(trimmed: &str) -> bool {
    trimmed.starts_with(BEGIN_PREFIX) && trimmed.ends_with(BEGIN_SUFFIX)
}

fn is_end_marker(trimmed: &str) -> bool {
    trimmed == END_MARKER
}

/// DAY-187: marker-aware fenced-code-block sentinel. A "fence" is the
/// markdown construct ` ```rust ` or ` ~~~ ` opening a code span. We
/// support backtick and tilde fences with three or more delimiters.
/// The sentinel records the exact delimiter character and its run
/// length so an inner shorter run cannot accidentally close the
/// outer fence — matching CommonMark's "closing fence must use the
/// same character and at least the same count" rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FenceSentinel {
    delim: char,
    len: usize,
}

impl FenceSentinel {
    fn detect_open(trimmed: &str) -> Option<FenceSentinel> {
        for delim in ['`', '~'] {
            let count = trimmed.chars().take_while(|c| *c == delim).count();
            if count >= 3 {
                return Some(FenceSentinel { delim, len: count });
            }
        }
        None
    }

    /// `true` if `trimmed` is a closing fence for `self`. CommonMark
    /// requires same character, equal-or-greater run, and no other
    /// non-whitespace content on the line. We approximate by
    /// requiring the entire trimmed line to be the delimiter
    /// repeated — info-strings on the closing fence are not legal.
    fn line_closes(self, trimmed: &str) -> bool {
        if trimmed.is_empty() {
            return false;
        }
        let count = trimmed.chars().take_while(|c| *c == self.delim).count();
        count >= self.len && trimmed.len() == count
    }
}

fn parse_begin_attrs(trimmed: &str, line_no: usize) -> Result<MarkerAttrs, MarkerError> {
    // Strip the well-known wrapping so only the attribute region is
    // left (e.g. `date="…" run_id="…" template="…" version="…"`).
    let inner = trimmed
        .strip_prefix(BEGIN_PREFIX)
        .and_then(|s| s.strip_suffix(BEGIN_SUFFIX))
        .ok_or_else(|| MarkerError::MalformedBegin {
            line: line_no,
            detail: "does not match '<!-- dayseam:begin … -->'".to_string(),
        })?
        .trim();

    let mut date: Option<NaiveDate> = None;
    let mut run_id: Option<String> = None;
    let mut template: Option<String> = None;
    let mut version: Option<String> = None;

    for (key, value) in iter_attrs(inner).map_err(|detail| MarkerError::MalformedBegin {
        line: line_no,
        detail,
    })? {
        match key {
            "date" => {
                let parsed = NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|e| {
                    MarkerError::MalformedBegin {
                        line: line_no,
                        detail: format!("date=\"{value}\" is not YYYY-MM-DD ({e})"),
                    }
                })?;
                date = Some(parsed);
            }
            "run_id" => run_id = Some(value.to_string()),
            "template" => template = Some(value.to_string()),
            "version" => version = Some(value.to_string()),
            _other => {
                // Unknown keys are ignored so later versions of the
                // sink can add new attributes without invalidating
                // files written by older versions. The round-trip
                // guarantee only covers the four required keys.
            }
        }
    }

    Ok(MarkerAttrs {
        date: date.ok_or_else(|| MarkerError::MalformedBegin {
            line: line_no,
            detail: "missing required attribute `date`".to_string(),
        })?,
        run_id: run_id.ok_or_else(|| MarkerError::MalformedBegin {
            line: line_no,
            detail: "missing required attribute `run_id`".to_string(),
        })?,
        template: template.ok_or_else(|| MarkerError::MalformedBegin {
            line: line_no,
            detail: "missing required attribute `template`".to_string(),
        })?,
        version: version.ok_or_else(|| MarkerError::MalformedBegin {
            line: line_no,
            detail: "missing required attribute `version`".to_string(),
        })?,
    })
}

/// Parse `key="value" key="value"` pairs. Deliberately strict: values
/// must be double-quoted and must not contain a literal `"`. The sink
/// only ever emits UUID / ISO-date / dotted-template-id values, so no
/// escaping is required in practice.
fn iter_attrs(s: &str) -> Result<Vec<(&str, &str)>, String> {
    let mut out = Vec::new();
    let mut rest = s.trim();
    while !rest.is_empty() {
        let eq = rest
            .find('=')
            .ok_or_else(|| format!("expected 'key=\"value\"' near: {rest:?}"))?;
        let key = rest[..eq].trim();
        if key.is_empty() || key.contains(char::is_whitespace) {
            return Err(format!("malformed attribute key near: {rest:?}"));
        }
        let after_eq = rest[eq + 1..].trim_start();
        if !after_eq.starts_with('"') {
            return Err(format!(
                "value for `{key}` must be double-quoted (got: {after_eq:?})"
            ));
        }
        let after_quote = &after_eq[1..];
        let close = after_quote
            .find('"')
            .ok_or_else(|| format!("unterminated quoted value for `{key}`"))?;
        let value = &after_quote[..close];
        out.push((key, value));
        rest = after_quote[close + 1..].trim_start();
    }
    Ok(out)
}

/// Split `text` into logical lines, preserving each line's terminator.
/// Guarantees `text == result.concat()`.
fn split_lines_preserving_newlines(text: &str) -> impl Iterator<Item = &str> {
    SplitPreservingNewlines { rest: text }
}

struct SplitPreservingNewlines<'a> {
    rest: &'a str,
}

impl<'a> Iterator for SplitPreservingNewlines<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        if self.rest.is_empty() {
            return None;
        }
        match self.rest.find('\n') {
            Some(idx) => {
                let (line, rest) = self.rest.split_at(idx + 1);
                self.rest = rest;
                Some(line)
            }
            None => {
                let line = self.rest;
                self.rest = "";
                Some(line)
            }
        }
    }
}

fn ensure_trailing_newline(segments: &mut [Segment]) {
    let Some(last) = segments.last_mut() else {
        return;
    };
    if let Segment::Prose(p) = last {
        if !p.ends_with('\n') {
            p.push('\n');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attrs(date: &str) -> MarkerAttrs {
        MarkerAttrs {
            date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
            run_id: "11111111-2222-3333-4444-555555555555".to_string(),
            template: "dayseam.dev_eod".to_string(),
            version: "2026-04-18".to_string(),
        }
    }

    fn block(date: &str, body: &str) -> Block {
        Block {
            attrs: attrs(date),
            body: body.to_string(),
            line_ending: LineEnding::Lf,
        }
    }

    #[test]
    fn parse_empty_file_is_empty_doc() {
        let doc = parse("").unwrap();
        assert!(doc.segments.is_empty());
    }

    #[test]
    fn parse_file_without_blocks_is_one_prose_segment() {
        let text = "# My notes\n\nsome prose\n";
        let doc = parse(text).unwrap();
        assert_eq!(doc.segments.len(), 1);
        assert!(matches!(&doc.segments[0], Segment::Prose(p) if p == text));
    }

    #[test]
    fn parse_then_render_round_trips_well_formed_files() {
        let text = concat!(
            "## Prelude prose\n",
            "\n",
            "<!-- dayseam:begin date=\"2026-04-18\" run_id=\"r1\" template=\"dayseam.dev_eod\" version=\"v1\" -->\n",
            "- first day body\n",
            "<!-- dayseam:end -->\n",
            "\n",
            "user prose between blocks\n",
            "\n",
            "<!-- dayseam:begin date=\"2026-04-17\" run_id=\"r2\" template=\"dayseam.dev_eod\" version=\"v1\" -->\n",
            "- second day body\n",
            "<!-- dayseam:end -->\n",
            "\n",
            "trailing prose\n",
        );
        let doc = parse(text).expect("well-formed doc parses");
        assert_eq!(render(&doc), text);
    }

    #[test]
    fn splice_replaces_only_matching_date_block() {
        let text = concat!(
            "<!-- dayseam:begin date=\"2026-04-18\" run_id=\"r1\" template=\"dayseam.dev_eod\" version=\"v1\" -->\n",
            "- old D1 body\n",
            "<!-- dayseam:end -->\n",
            "\n",
            "middle user prose\n",
            "\n",
            "<!-- dayseam:begin date=\"2026-04-17\" run_id=\"r2\" template=\"dayseam.dev_eod\" version=\"v1\" -->\n",
            "- D2 body untouched\n",
            "<!-- dayseam:end -->\n",
        );
        let mut doc = parse(text).unwrap();
        let replaced = splice(&mut doc, block("2026-04-18", "- new D1 body\n"));
        assert!(replaced);
        let out = render(&doc);
        assert!(out.contains("- new D1 body"));
        assert!(!out.contains("- old D1 body"));
        assert!(out.contains("- D2 body untouched"));
        assert!(out.contains("middle user prose"));
    }

    #[test]
    fn splice_preserves_surrounding_prose_byte_for_byte() {
        let original = concat!(
            "# Daily journal\n",
            "\n",
            "<!-- dayseam:begin date=\"2026-04-18\" run_id=\"r1\" template=\"dayseam.dev_eod\" version=\"v1\" -->\n",
            "- old body\n",
            "<!-- dayseam:end -->\n",
            "\n",
            "-- free-form note kept verbatim --\n",
        );
        let mut doc = parse(original).unwrap();
        splice(&mut doc, block("2026-04-18", "- refreshed body\n"));
        let out = render(&doc);
        assert!(out.starts_with("# Daily journal\n\n"));
        assert!(out.ends_with("-- free-form note kept verbatim --\n"));
    }

    #[test]
    fn splice_appends_when_no_matching_date() {
        let original = concat!(
            "existing prose\n",
            "<!-- dayseam:begin date=\"2026-04-17\" run_id=\"r0\" template=\"dayseam.dev_eod\" version=\"v1\" -->\n",
            "- yesterday\n",
            "<!-- dayseam:end -->\n",
        );
        let mut doc = parse(original).unwrap();
        let replaced = splice(&mut doc, block("2026-04-18", "- today\n"));
        assert!(!replaced);
        let out = render(&doc);
        assert!(out.contains("- yesterday"));
        assert!(out.contains("- today"));
        let today_pos = out.find("- today").unwrap();
        let yesterday_pos = out.find("- yesterday").unwrap();
        assert!(today_pos > yesterday_pos);
    }

    #[test]
    fn splice_appends_into_empty_doc() {
        let mut doc = parse("").unwrap();
        splice(&mut doc, block("2026-04-18", "- first ever\n"));
        let out = render(&doc);
        assert!(out.starts_with("<!-- dayseam:begin"));
        assert!(out.contains("- first ever"));
        assert!(out.ends_with("<!-- dayseam:end -->\n"));
    }

    #[test]
    fn nested_begin_is_rejected() {
        let text = concat!(
            "<!-- dayseam:begin date=\"2026-04-18\" run_id=\"r1\" template=\"t\" version=\"v\" -->\n",
            "<!-- dayseam:begin date=\"2026-04-17\" run_id=\"r2\" template=\"t\" version=\"v\" -->\n",
            "<!-- dayseam:end -->\n",
        );
        assert!(matches!(parse(text), Err(MarkerError::NestedBegin { .. })));
    }

    #[test]
    fn unclosed_begin_is_rejected() {
        let text = concat!(
            "<!-- dayseam:begin date=\"2026-04-18\" run_id=\"r1\" template=\"t\" version=\"v\" -->\n",
            "still body\n",
        );
        assert!(matches!(
            parse(text),
            Err(MarkerError::UnclosedBegin { .. })
        ));
    }

    #[test]
    fn dangling_end_is_rejected() {
        let text = concat!("prose\n", "<!-- dayseam:end -->\n", "more prose\n");
        assert!(matches!(parse(text), Err(MarkerError::DanglingEnd { .. })));
    }

    #[test]
    fn missing_required_attr_is_rejected() {
        let text = concat!(
            "<!-- dayseam:begin date=\"2026-04-18\" run_id=\"r1\" template=\"t\" -->\n",
            "<!-- dayseam:end -->\n",
        );
        assert!(matches!(
            parse(text),
            Err(MarkerError::MalformedBegin { .. })
        ));
    }

    #[test]
    fn unknown_attributes_are_ignored_for_forward_compat() {
        let text = concat!(
            "<!-- dayseam:begin date=\"2026-04-18\" run_id=\"r1\" template=\"t\" version=\"v\" extra=\"ok\" -->\n",
            "- body\n",
            "<!-- dayseam:end -->\n",
        );
        let doc = parse(text).expect("unknown attrs are forward-compatible");
        assert_eq!(doc.segments.len(), 1);
    }

    #[test]
    fn whitespace_between_attributes_is_tolerated() {
        let text = concat!(
            "<!-- dayseam:begin   date=\"2026-04-18\"   run_id=\"r1\"  template=\"t\"  version=\"v\"   -->\n",
            "- body\n",
            "<!-- dayseam:end -->\n",
        );
        parse(text).expect("tolerant of multi-space separators");
    }

    #[test]
    fn round_trip_preserves_unix_and_windows_line_endings() {
        // Windows-style CRLF body is preserved byte-for-byte inside the
        // block even though the marker lines themselves are ASCII LF.
        let text = "<!-- dayseam:begin date=\"2026-04-18\" run_id=\"r1\" template=\"t\" version=\"v\" -->\n\
            - body with \r\n in it\r\n\
            <!-- dayseam:end -->\n";
        let doc = parse(text).unwrap();
        assert_eq!(render(&doc), text);
    }

    /// DAY-187: a fenced code block in user prose containing the
    /// literal marker syntax must NOT be parsed as a real begin/end
    /// pair. Without the fence-tracking guard, the first save would
    /// rewrite the user's example block on disk.
    #[test]
    fn fenced_marker_syntax_inside_prose_is_not_a_marker() {
        let text = concat!(
            "Here's how Dayseam stores its blocks:\n",
            "\n",
            "```md\n",
            "<!-- dayseam:begin date=\"2026-04-18\" run_id=\"example\" template=\"dayseam.dev_eod\" version=\"v\" -->\n",
            "## Commits\n",
            "- example body\n",
            "<!-- dayseam:end -->\n",
            "```\n",
            "\n",
            "End of explanation.\n",
        );
        let doc = parse(text).expect("fence-wrapped marker syntax must not be parsed as a marker");
        // The whole text should round-trip as prose — no Block segment
        // should exist because the fence makes the markers opaque.
        assert!(
            doc.segments.iter().all(|s| matches!(s, Segment::Prose(_))),
            "fence-wrapped marker syntax must be treated as prose, got {doc:?}"
        );
        assert_eq!(render(&doc), text);
    }

    /// DAY-187 (companion): tilde fences must work the same as
    /// backtick fences — CommonMark allows both.
    #[test]
    fn tilde_fenced_marker_syntax_is_not_a_marker() {
        let text = concat!(
            "~~~md\n",
            "<!-- dayseam:begin date=\"2026-04-18\" run_id=\"example\" template=\"t\" version=\"v\" -->\n",
            "<!-- dayseam:end -->\n",
            "~~~\n",
        );
        let doc = parse(text).expect("tilde-fenced markers are inert");
        assert!(doc.segments.iter().all(|s| matches!(s, Segment::Prose(_))));
        assert_eq!(render(&doc), text);
    }

    /// DAY-187: a closing fence with a shorter run does NOT close
    /// the outer fence (CommonMark rule). This pins the
    /// `FenceSentinel::line_closes` invariant.
    #[test]
    fn fence_closing_run_must_match_or_exceed_opening() {
        let text = concat!(
            "````md\n",
            "```\n", // shorter inner run — does NOT close the outer
            "<!-- dayseam:end -->\n",
            "```\n",  // still inner
            "````\n", // matching close
        );
        let doc = parse(text).expect("inner shorter run must not close outer fence");
        assert!(doc.segments.iter().all(|s| matches!(s, Segment::Prose(_))));
    }

    /// DAY-187: an empty marker block must round-trip without an
    /// injected blank line. The previous shape pushed a `\n`
    /// unconditionally because `"" ends_with('\n')` is false; after
    /// the fix, an empty body emits exactly one `\n` between begin
    /// and end (the begin's terminator) plus the end's terminator.
    #[test]
    fn empty_block_round_trips_without_blank_line() {
        let text = "<!-- dayseam:begin date=\"2026-04-18\" run_id=\"r1\" template=\"t\" version=\"v\" -->\n\
                    <!-- dayseam:end -->\n";
        let doc = parse(text).unwrap();
        assert_eq!(render(&doc), text);
    }

    /// DAY-187: a CRLF-terminated marker line must round-trip CRLF.
    /// Without per-block line-ending tracking, the marker lines
    /// silently flipped to LF on every save and Windows users saw a
    /// one-line drift on first contact.
    #[test]
    fn crlf_marker_lines_round_trip_preserved() {
        let text = "<!-- dayseam:begin date=\"2026-04-18\" run_id=\"r1\" template=\"t\" version=\"v\" -->\r\n\
                    - body\r\n\
                    <!-- dayseam:end -->\r\n";
        let doc = parse(text).unwrap();
        assert_eq!(render(&doc), text);
    }

    /// DAY-187 (companion): splicing a new block into a CRLF-bearing
    /// document copies the existing block's terminator forward so
    /// the rewrite stays CRLF on the marker lines. Without this, a
    /// CRLF Windows file flipped to LF on every save the moment the
    /// orchestrator's freshly-rendered LF block was spliced in.
    #[test]
    fn splice_preserves_existing_blocks_line_ending() {
        let text = "<!-- dayseam:begin date=\"2026-04-18\" run_id=\"r1\" template=\"t\" version=\"v\" -->\r\n\
                    - old body\r\n\
                    <!-- dayseam:end -->\r\n";
        let mut doc = parse(text).unwrap();
        let replaced = splice(&mut doc, block("2026-04-18", "- new body\r\n"));
        assert!(replaced);
        let out = render(&doc);
        // Marker lines must still end in CRLF.
        assert!(
            out.contains("<!-- dayseam:end -->\r\n"),
            "expected CRLF on end marker, got: {out:?}"
        );
        assert!(
            out.contains("\" -->\r\n"),
            "expected CRLF on begin marker, got: {out:?}"
        );
    }

    /// DAY-187 (companion to the L5 audit nit): splice against a doc
    /// whose last segment is a `Block` (no prose tail) emits a
    /// syntactically clean concatenation. Pins the
    /// `ensure_trailing_newline` no-op behaviour for the
    /// last-segment-is-Block case.
    #[test]
    fn splice_against_doc_ending_in_block_produces_clean_output() {
        // First parse a doc with one block and no trailing prose.
        let text = "<!-- dayseam:begin date=\"2026-04-17\" run_id=\"r0\" template=\"t\" version=\"v\" -->\n\
                    - yesterday\n\
                    <!-- dayseam:end -->\n";
        let mut doc = parse(text).unwrap();
        // Splice in a new (different-date) block. The doc's last
        // segment is currently a Block; ensure_trailing_newline is
        // a no-op there (Blocks are emitted with their own
        // terminator). The resulting text must be parseable
        // again.
        let replaced = splice(&mut doc, block("2026-04-18", "- today\n"));
        assert!(!replaced);
        let out = render(&doc);
        // Round-trip through parse to assert structural validity.
        let reparsed = parse(&out).expect("appended doc must re-parse");
        assert_eq!(reparsed.segments.len(), 2);
    }
}
