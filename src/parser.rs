use crate::model::{Heading, OrgDoc, Settings};
use std::collections::VecDeque;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Parse an org-mode document from its text content.
pub fn parse(input: &str) -> OrgDoc {
    let lines: Vec<&str> = input.lines().collect();

    // Locate headline lines.
    let headline_indices: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| headline_level(l).is_some())
        .map(|(i, _)| i)
        .collect();

    // Everything before the first headline is preamble.
    let preamble_end = headline_indices.first().copied().unwrap_or(lines.len());
    let preamble = if preamble_end > 0 {
        let mut p = lines[..preamble_end].join("\n");
        p.push('\n');
        p
    } else {
        String::new()
    };

    let settings = parse_settings(&preamble);

    // Split into flat sections (headline + content until next headline).
    let mut flat: VecDeque<FlatSection> = VecDeque::new();
    for (pos, &start) in headline_indices.iter().enumerate() {
        let end = headline_indices
            .get(pos + 1)
            .copied()
            .unwrap_or(lines.len());
        let hl = lines[start];
        let content = &lines[start + 1..end];
        flat.push_back(parse_flat_section(hl, content, &settings));
    }

    let headings = build_tree(&mut flat, 0);

    OrgDoc {
        preamble,
        headings,
        settings,
    }
}

// ---------------------------------------------------------------------------
// Flat section (intermediate representation)
// ---------------------------------------------------------------------------

struct FlatSection {
    level: usize,
    keyword: Option<String>,
    priority: Option<char>,
    title: String,
    tags: Vec<String>,
    planning: Option<String>,
    properties: Vec<(String, String)>,
    body: String,
}

fn parse_flat_section(headline: &str, content: &[&str], settings: &Settings) -> FlatSection {
    let (level, keyword, priority, title, tags) = parse_headline(headline, settings);
    let (planning, properties, body) = parse_content(content);
    FlatSection {
        level,
        keyword,
        priority,
        title,
        tags,
        planning,
        properties,
        body,
    }
}

// ---------------------------------------------------------------------------
// Tree construction
// ---------------------------------------------------------------------------

/// Recursively consume flat sections whose level > parent_level and
/// assemble them into a tree of Heading nodes.
fn build_tree(sections: &mut VecDeque<FlatSection>, parent_level: usize) -> Vec<Heading> {
    let mut headings: Vec<Heading> = Vec::new();

    while let Some(front) = sections.front() {
        if front.level <= parent_level {
            break;
        }
        let sec = sections.pop_front().unwrap();
        let level = sec.level;
        let mut heading = Heading {
            level: sec.level,
            keyword: sec.keyword,
            priority: sec.priority,
            title: sec.title,
            tags: sec.tags,
            planning: sec.planning,
            properties: sec.properties,
            body: sec.body,
            children: Vec::new(),
        };
        heading.children = build_tree(sections, level);
        headings.push(heading);
    }

    headings
}

// ---------------------------------------------------------------------------
// Headline parsing
// ---------------------------------------------------------------------------

/// Return the heading level (number of stars) if `line` is a headline.
pub fn headline_level(line: &str) -> Option<usize> {
    if !line.starts_with('*') {
        return None;
    }
    let stars = line.bytes().take_while(|&b| b == b'*').count();
    // Must be followed by a space or be the entire line.
    if line.len() == stars || line.as_bytes().get(stars) == Some(&b' ') {
        Some(stars)
    } else {
        None
    }
}

/// Parse a headline into (level, keyword, priority, title, tags).
fn parse_headline(
    line: &str,
    settings: &Settings,
) -> (usize, Option<String>, Option<char>, String, Vec<String>) {
    let stars = line.bytes().take_while(|&b| b == b'*').count();
    let rest = line[stars..].trim();

    // Keyword
    let (keyword, rest) = extract_keyword(rest, settings);
    // Priority — only valid immediately after a keyword
    let (priority, rest) = if keyword.is_some() {
        extract_priority(rest)
    } else {
        (None, rest)
    };
    // Tags (from the end)
    let (title, tags) = extract_tags(rest);

    (stars, keyword, priority, title, tags)
}

fn extract_keyword<'a>(text: &'a str, settings: &Settings) -> (Option<String>, &'a str) {
    if let Some(space) = text.find(' ') {
        let word = &text[..space];
        if settings.is_keyword(word) {
            return (Some(word.to_string()), text[space..].trim_start());
        }
    } else if settings.is_keyword(text) {
        return (Some(text.to_string()), "");
    }
    (None, text)
}

fn extract_priority(text: &str) -> (Option<char>, &str) {
    if text.len() >= 4
        && text.starts_with("[#")
        && text.as_bytes()[3] == b']'
        && text.as_bytes()[2].is_ascii_uppercase()
    {
        let ch = text.as_bytes()[2] as char;
        let rest = if text.len() > 4 {
            text[4..].trim_start()
        } else {
            ""
        };
        return (Some(ch), rest);
    }
    (None, text)
}

fn extract_tags(text: &str) -> (String, Vec<String>) {
    // Tags sit at the end of the headline:  `:tag1:tag2:`
    // preceded by at least one space.
    if let Some(idx) = text.rfind(" :") {
        let candidate = text[idx + 1..].trim_end();
        if candidate.starts_with(':')
            && candidate.ends_with(':')
            && candidate.len() > 2
        {
            let inner = &candidate[1..candidate.len() - 1];
            let parts: Vec<&str> = inner.split(':').collect();
            if parts
                .iter()
                .all(|p| !p.is_empty() && p.chars().all(|c| c.is_alphanumeric() || "_@#%".contains(c)))
            {
                let tags = parts.iter().map(|s| s.to_string()).collect();
                return (text[..idx].trim_end().to_string(), tags);
            }
        }
    }
    (text.to_string(), Vec::new())
}

// ---------------------------------------------------------------------------
// Content parsing (planning, properties, body)
// ---------------------------------------------------------------------------

fn parse_content(lines: &[&str]) -> (Option<String>, Vec<(String, String)>, String) {
    let mut idx = 0;

    // Planning line
    let planning = if idx < lines.len() && is_planning_line(lines[idx]) {
        let p = lines[idx].to_string();
        idx += 1;
        Some(p)
    } else {
        None
    };

    // Property drawer
    let mut properties: Vec<(String, String)> = Vec::new();
    if idx < lines.len() && lines[idx].trim() == ":PROPERTIES:" {
        idx += 1;
        while idx < lines.len() && lines[idx].trim() != ":END:" {
            if let Some(kv) = parse_property_line(lines[idx]) {
                properties.push(kv);
            }
            idx += 1;
        }
        if idx < lines.len() {
            idx += 1; // skip :END:
        }
    }

    // Body: everything remaining
    let body = if idx < lines.len() {
        let mut b = lines[idx..].join("\n");
        b.push('\n');
        b
    } else {
        String::new()
    };

    (planning, properties, body)
}

fn is_planning_line(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("SCHEDULED:") || t.starts_with("DEADLINE:") || t.starts_with("CLOSED:")
}

fn parse_property_line(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim();
    if !trimmed.starts_with(':') || trimmed.len() < 3 {
        return None;
    }
    let rest = &trimmed[1..];
    if let Some(colon) = rest.find(':') {
        let key = rest[..colon].to_string();
        let value = rest[colon + 1..].trim().to_string();
        if !key.is_empty() {
            return Some((key, value));
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Settings (#+TODO: lines)
// ---------------------------------------------------------------------------

fn parse_settings(preamble: &str) -> Settings {
    let mut todo_kw: Vec<String> = Vec::new();
    let mut done_kw: Vec<String> = Vec::new();
    let mut found = false;

    for line in preamble.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed
            .strip_prefix("#+TODO:")
            .or_else(|| trimmed.strip_prefix("#+SEQ_TODO:"))
            .or_else(|| trimmed.strip_prefix("#+TYP_TODO:"))
        {
            found = true;
            let rest = rest.trim();
            let mut after_pipe = false;
            for word in rest.split_whitespace() {
                if word == "|" {
                    after_pipe = true;
                    continue;
                }
                if after_pipe {
                    done_kw.push(word.to_string());
                } else {
                    todo_kw.push(word.to_string());
                }
            }
        }
    }

    if found {
        // If no pipe was found, all keywords are active (no done keywords).
        // That's unusual but valid; leave done_kw empty.
        Settings {
            todo_keywords: todo_kw,
            done_keywords: done_kw,
        }
    } else {
        Settings::default()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // headline_level
    // -----------------------------------------------------------------------

    #[test]
    fn headline_level_basic() {
        assert_eq!(headline_level("* Hello"), Some(1));
        assert_eq!(headline_level("** Sub"), Some(2));
        assert_eq!(headline_level("*** TODO Foo"), Some(3));
        assert_eq!(headline_level("***** Deep"), Some(5));
    }

    #[test]
    fn headline_level_bare_stars() {
        // A bare `*` with nothing after it is a valid (empty-title) heading.
        assert_eq!(headline_level("*"), Some(1));
        assert_eq!(headline_level("***"), Some(3));
    }

    #[test]
    fn headline_level_not_a_heading() {
        assert_eq!(headline_level(""), None);
        assert_eq!(headline_level("not a heading"), None);
        assert_eq!(headline_level("  * indented"), None);
        assert_eq!(headline_level("*bold*"), None);
        assert_eq!(headline_level("**also bold**"), None);
        assert_eq!(headline_level("*word"), None);
        assert_eq!(headline_level(" "), None);
    }

    // -----------------------------------------------------------------------
    // extract_keyword
    // -----------------------------------------------------------------------

    #[test]
    fn keyword_extraction_default_keywords() {
        let s = Settings::default();
        assert_eq!(extract_keyword("TODO Buy milk", &s), (Some("TODO".into()), "Buy milk"));
        assert_eq!(extract_keyword("DONE Laundry", &s), (Some("DONE".into()), "Laundry"));
        assert_eq!(extract_keyword("NEXT Call dentist", &s), (Some("NEXT".into()), "Call dentist"));
        assert_eq!(extract_keyword("WAITING Response", &s), (Some("WAITING".into()), "Response"));
        assert_eq!(extract_keyword("CANCELLED Old task", &s), (Some("CANCELLED".into()), "Old task"));
    }

    #[test]
    fn keyword_extraction_no_keyword() {
        let s = Settings::default();
        assert_eq!(extract_keyword("Buy milk", &s), (None, "Buy milk"));
        assert_eq!(extract_keyword("", &s), (None, ""));
        // DOING is not a default keyword
        assert_eq!(extract_keyword("DOING something", &s), (None, "DOING something"));
    }

    #[test]
    fn keyword_extraction_keyword_alone() {
        let s = Settings::default();
        // Heading is just the keyword with no title
        assert_eq!(extract_keyword("TODO", &s), (Some("TODO".into()), ""));
    }

    #[test]
    fn keyword_extraction_custom() {
        let s = Settings {
            todo_keywords: vec!["OPEN".into(), "IN_PROGRESS".into()],
            done_keywords: vec!["CLOSED".into()],
        };
        assert_eq!(extract_keyword("OPEN Bug report", &s), (Some("OPEN".into()), "Bug report"));
        assert_eq!(extract_keyword("TODO not recognized", &s), (None, "TODO not recognized"));
        // Done keyword from custom set
        assert_eq!(extract_keyword("CLOSED Fixed it", &s), (Some("CLOSED".into()), "Fixed it"));
        // Keyword with underscore
        assert_eq!(extract_keyword("IN_PROGRESS Working on it", &s), (Some("IN_PROGRESS".into()), "Working on it"));
        // Keyword is prefix of next word — should NOT match since there's no space boundary
        assert_eq!(extract_keyword("OPENSSL error", &s), (None, "OPENSSL error"));
    }

    // -----------------------------------------------------------------------
    // extract_priority
    // -----------------------------------------------------------------------

    #[test]
    fn priority_extraction() {
        assert_eq!(extract_priority("[#A] Title"), (Some('A'), "Title"));
        assert_eq!(extract_priority("[#B] Title"), (Some('B'), "Title"));
        assert_eq!(extract_priority("[#C] Title"), (Some('C'), "Title"));
        assert_eq!(extract_priority("[#Z] Title"), (Some('Z'), "Title"));
        // Middle of range
        assert_eq!(extract_priority("[#M] Title"), (Some('M'), "Title"));
    }

    #[test]
    fn priority_alone() {
        assert_eq!(extract_priority("[#A]"), (Some('A'), ""));
    }

    #[test]
    fn no_priority() {
        assert_eq!(extract_priority("Title without priority"), (None, "Title without priority"));
        assert_eq!(extract_priority(""), (None, ""));
        // Lowercase is not a valid priority cookie
        assert_eq!(extract_priority("[#a] Title"), (None, "[#a] Title"));
        // Malformed
        assert_eq!(extract_priority("[#AB] Title"), (None, "[#AB] Title"));
        assert_eq!(extract_priority("[# ] Title"), (None, "[# ] Title"));
        // Digit is not a valid priority
        assert_eq!(extract_priority("[#0] Title"), (None, "[#0] Title"));
        assert_eq!(extract_priority("[#9] Title"), (None, "[#9] Title"));
        // Symbols
        assert_eq!(extract_priority("[#!] Title"), (None, "[#!] Title"));
    }

    #[test]
    fn priority_rejected_when_after_keyword_in_headline() {
        // These malformed cookies should be rejected even after a keyword.
        let s = Settings::default();
        let (_, _, pri, title, _) = parse_headline("* TODO [#a] lowered", &s);
        assert_eq!(pri, None);
        assert_eq!(title, "[#a] lowered");
        let (_, _, pri, title, _) = parse_headline("* TODO [#0] digit", &s);
        assert_eq!(pri, None);
        assert_eq!(title, "[#0] digit");
    }

    // -----------------------------------------------------------------------
    // extract_tags
    // -----------------------------------------------------------------------

    #[test]
    fn tags_extraction_multiple() {
        let (title, tags) = extract_tags("Urgent task :work:urgent:");
        assert_eq!(title, "Urgent task");
        assert_eq!(tags, vec!["work", "urgent"]);
    }

    #[test]
    fn tags_extraction_single() {
        let (title, tags) = extract_tags("Task :work:");
        assert_eq!(title, "Task");
        assert_eq!(tags, vec!["work"]);
    }

    #[test]
    fn tags_extraction_special_chars() {
        let (title, tags) = extract_tags("Task :@home:errand_1:#project:%ctx:");
        assert_eq!(title, "Task");
        assert_eq!(tags, vec!["@home", "errand_1", "#project", "%ctx"]);
    }

    #[test]
    fn tags_not_tags() {
        // No space before colon — not tags
        let (title, tags) = extract_tags("key:value");
        assert_eq!(title, "key:value");
        assert!(tags.is_empty());

        // Empty tag segment — not valid
        let (title, tags) = extract_tags("Title :::");
        assert_eq!(title, "Title :::");
        assert!(tags.is_empty());

        // Plain text, no colons
        let (title, _tags) = extract_tags("Just text no colons");
        assert_eq!(title, "Just text no colons");

        // URL in title — colons shouldn't become tags
        let (title, tags) = extract_tags("Visit https://example.com today");
        assert_eq!(title, "Visit https://example.com today");
        assert!(tags.is_empty());

        // Time range with colons
        let (title, tags) = extract_tags("Meeting 10:00-11:00");
        assert_eq!(title, "Meeting 10:00-11:00");
        assert!(tags.is_empty());

        // Trailing single colon — not a tag
        let (title, tags) = extract_tags("Note:");
        assert_eq!(title, "Note:");
        assert!(tags.is_empty());

        // Tag-like segment with a space inside — not valid
        let (title, tags) = extract_tags("Title :has space:");
        assert_eq!(title, "Title :has space:");
        assert!(tags.is_empty());
    }

    #[test]
    fn tags_with_trailing_whitespace() {
        let (title, tags) = extract_tags("Task :work:  ");
        assert_eq!(title, "Task");
        assert_eq!(tags, vec!["work"]);
    }

    // -----------------------------------------------------------------------
    // parse_headline (full pipeline)
    // -----------------------------------------------------------------------

    #[test]
    fn headline_all_components() {
        let s = Settings::default();
        let (level, kw, pri, title, tags) =
            parse_headline("** TODO [#A] Urgent task :work:urgent:", &s);
        assert_eq!(level, 2);
        assert_eq!(kw.as_deref(), Some("TODO"));
        assert_eq!(pri, Some('A'));
        assert_eq!(title, "Urgent task");
        assert_eq!(tags, vec!["work", "urgent"]);
    }

    #[test]
    fn headline_title_only() {
        let s = Settings::default();
        let (level, kw, pri, title, tags) = parse_headline("* Just a title", &s);
        assert_eq!(level, 1);
        assert_eq!(kw, None);
        assert_eq!(pri, None);
        assert_eq!(title, "Just a title");
        assert!(tags.is_empty());
    }

    #[test]
    fn headline_keyword_no_title() {
        let s = Settings::default();
        let (level, kw, _pri, title, tags) = parse_headline("* TODO", &s);
        assert_eq!(level, 1);
        assert_eq!(kw.as_deref(), Some("TODO"));
        assert_eq!(title, "");
        assert!(tags.is_empty());
    }

    #[test]
    fn headline_priority_without_keyword() {
        let s = Settings::default();
        let (_, kw, pri, title, _) = parse_headline("* [#B] Some task", &s);
        // Without a keyword, [#B] is treated as part of the title (org-mode
        // spec says priority cookie only follows a keyword).
        assert_eq!(kw, None);
        assert_eq!(pri, None);
        assert_eq!(title, "[#B] Some task");
    }

    #[test]
    fn headline_title_with_colons_not_tags() {
        let s = Settings::default();
        let (_, _, _, title, tags) = parse_headline("* Meeting at 10:30", &s);
        // "10:30" doesn't satisfy tag format — no surrounding colons
        assert_eq!(title, "Meeting at 10:30");
        assert!(tags.is_empty());
    }

    // -----------------------------------------------------------------------
    // Content parsing (planning, properties, body)
    // -----------------------------------------------------------------------

    #[test]
    fn content_empty() {
        let (planning, properties, body) = parse_content(&[]);
        assert!(planning.is_none());
        assert!(properties.is_empty());
        assert_eq!(body, "");
    }

    #[test]
    fn content_body_only() {
        let (planning, properties, body) = parse_content(&["Body line 1", "Body line 2"]);
        assert!(planning.is_none());
        assert!(properties.is_empty());
        assert_eq!(body, "Body line 1\nBody line 2\n");
    }

    #[test]
    fn content_planning_plus_body() {
        let (planning, properties, body) =
            parse_content(&["SCHEDULED: <2024-01-15 Mon>", "Body text"]);
        assert_eq!(planning.as_deref(), Some("SCHEDULED: <2024-01-15 Mon>"));
        assert!(properties.is_empty());
        assert_eq!(body, "Body text\n");
    }

    #[test]
    fn content_deadline_plus_body() {
        let (planning, _, body) =
            parse_content(&["DEADLINE: <2024-06-01 Sat>", "Do it now"]);
        assert_eq!(planning.as_deref(), Some("DEADLINE: <2024-06-01 Sat>"));
        assert_eq!(body, "Do it now\n");
    }

    #[test]
    fn content_closed_plus_body() {
        let (planning, _, body) =
            parse_content(&["CLOSED: [2024-06-01 Sat 15:30]", "Finished."]);
        assert!(planning.as_ref().unwrap().contains("CLOSED"));
        assert_eq!(body, "Finished.\n");
    }

    #[test]
    fn content_planning_plus_properties_plus_body() {
        let lines = &[
            "DEADLINE: <2024-06-01 Sat>",
            ":PROPERTIES:",
            ":ID: x",
            ":END:",
            "Body",
        ];
        let (planning, properties, body) = parse_content(lines);
        assert!(planning.as_ref().unwrap().contains("DEADLINE"));
        assert_eq!(properties, vec![("ID".into(), "x".into())]);
        assert_eq!(body, "Body\n");
    }

    #[test]
    fn content_properties_without_planning() {
        let lines = &[":PROPERTIES:", ":FOO: bar", ":END:", "Body"];
        let (planning, properties, body) = parse_content(lines);
        assert!(planning.is_none());
        assert_eq!(properties, vec![("FOO".into(), "bar".into())]);
        assert_eq!(body, "Body\n");
    }

    #[test]
    fn content_property_empty_value() {
        let lines = &[":PROPERTIES:", ":EMPTY:", ":END:"];
        let (_, properties, _) = parse_content(lines);
        assert_eq!(properties, vec![("EMPTY".into(), "".into())]);
    }

    #[test]
    fn content_property_mixed_empty_and_nonempty() {
        let lines = &[
            ":PROPERTIES:",
            ":ID: abc",
            ":MARKER:",
            ":EFFORT: 2h",
            ":END:",
        ];
        let (_, properties, _) = parse_content(lines);
        assert_eq!(properties.len(), 3);
        assert_eq!(properties[0], ("ID".into(), "abc".into()));
        assert_eq!(properties[1], ("MARKER".into(), "".into()));
        assert_eq!(properties[2], ("EFFORT".into(), "2h".into()));
    }

    #[test]
    fn content_property_value_with_colons() {
        let lines = &[":PROPERTIES:", ":URL: https://example.com:8080/path", ":END:"];
        let (_, properties, _) = parse_content(lines);
        assert_eq!(properties[0].0, "URL");
        assert_eq!(properties[0].1, "https://example.com:8080/path");
    }

    #[test]
    fn content_property_value_with_whitespace() {
        let lines = &[":PROPERTIES:", ":DESC:   spaced out  ", ":END:"];
        let (_, properties, _) = parse_content(lines);
        assert_eq!(properties[0].0, "DESC");
        // parse_property_line trims the value
        assert_eq!(properties[0].1, "spaced out");
    }

    #[test]
    fn content_properties_not_at_start() {
        // If there's body text before the property drawer, it's not a drawer.
        let lines = &["Body first", ":PROPERTIES:", ":ID: x", ":END:"];
        let (_, properties, body) = parse_content(lines);
        assert!(properties.is_empty());
        // The whole thing is body text
        assert!(body.contains("Body first"));
        assert!(body.contains(":PROPERTIES:"));
    }

    #[test]
    fn content_body_with_blank_lines() {
        let lines = &["Line 1", "", "Line 3", "", ""];
        let (_, _, body) = parse_content(lines);
        assert_eq!(body, "Line 1\n\nLine 3\n\n\n");
    }

    #[test]
    fn content_deadline_and_scheduled() {
        // Org supports DEADLINE + SCHEDULED on the same line
        let lines = &["DEADLINE: <2024-06-01> SCHEDULED: <2024-05-15>", "Body"];
        let (planning, _, body) = parse_content(lines);
        let p = planning.unwrap();
        assert!(p.contains("DEADLINE"));
        assert!(p.contains("SCHEDULED"));
        assert_eq!(body, "Body\n");
    }

    #[test]
    fn content_closed_line() {
        let lines = &["CLOSED: [2024-06-01 Sat 15:30]"];
        let (planning, _, _) = parse_content(lines);
        assert!(planning.as_ref().unwrap().contains("CLOSED"));
    }

    // -----------------------------------------------------------------------
    // Settings parsing
    // -----------------------------------------------------------------------

    #[test]
    fn settings_default_when_absent() {
        let s = parse_settings("#+TITLE: Foo\n");
        assert_eq!(s.todo_keywords, vec!["TODO", "NEXT", "WAITING", "HOLD"]);
        assert_eq!(s.done_keywords, vec!["DONE", "CANCELLED"]);
    }

    #[test]
    fn settings_todo_with_pipe() {
        let s = parse_settings("#+TODO: TODO NEXT | DONE CANCELLED\n");
        assert_eq!(s.todo_keywords, vec!["TODO", "NEXT"]);
        assert_eq!(s.done_keywords, vec!["DONE", "CANCELLED"]);
    }

    #[test]
    fn settings_todo_without_pipe() {
        let s = parse_settings("#+TODO: TODO NEXT DONE\n");
        assert_eq!(s.todo_keywords, vec!["TODO", "NEXT", "DONE"]);
        assert!(s.done_keywords.is_empty());
    }

    #[test]
    fn settings_seq_todo() {
        let s = parse_settings("#+SEQ_TODO: OPEN IN_REVIEW | MERGED\n");
        assert_eq!(s.todo_keywords, vec!["OPEN", "IN_REVIEW"]);
        assert_eq!(s.done_keywords, vec!["MERGED"]);
    }

    #[test]
    fn settings_multiple_todo_lines() {
        let s = parse_settings("#+TODO: TODO | DONE\n#+TODO: OPEN | CLOSED\n");
        assert_eq!(s.todo_keywords, vec!["TODO", "OPEN"]);
        assert_eq!(s.done_keywords, vec!["DONE", "CLOSED"]);
    }

    #[test]
    fn settings_keywords_affect_parsing() {
        let input = "#+TODO: OPEN | CLOSED\n\n* OPEN New task\n* CLOSED Old task\n";
        let doc = parse(input);
        assert_eq!(doc.headings[0].keyword.as_deref(), Some("OPEN"));
        assert_eq!(doc.headings[1].keyword.as_deref(), Some("CLOSED"));
    }

    #[test]
    fn settings_non_keyword_not_parsed() {
        // With custom settings, default keywords shouldn't be recognised
        let input = "#+TODO: OPEN | CLOSED\n\n* TODO This is a title\n";
        let doc = parse(input);
        // "TODO" is not a keyword here — it becomes part of the title
        assert_eq!(doc.headings[0].keyword, None);
        assert!(doc.headings[0].title.starts_with("TODO"));
    }

    // -----------------------------------------------------------------------
    // Full document parsing
    // -----------------------------------------------------------------------

    #[test]
    fn parse_empty_input() {
        let doc = parse("");
        assert_eq!(doc.preamble, "");
        assert!(doc.headings.is_empty());
    }

    #[test]
    fn parse_preamble_only() {
        let doc = parse("#+TITLE: Notes\nSome text\n");
        assert!(doc.preamble.contains("#+TITLE: Notes"));
        assert!(doc.preamble.contains("Some text"));
        assert!(doc.headings.is_empty());
    }

    #[test]
    fn parse_single_heading_no_body() {
        let doc = parse("* Heading\n");
        assert_eq!(doc.headings.len(), 1);
        assert_eq!(doc.headings[0].title, "Heading");
        assert_eq!(doc.headings[0].body, "");
        assert!(doc.headings[0].children.is_empty());
    }

    #[test]
    fn parse_simple() {
        let input = "* TODO Buy milk\n** Whole milk\n** Skim milk\n* DONE Laundry\n";
        let doc = parse(input);
        assert_eq!(doc.headings.len(), 2);
        assert_eq!(doc.headings[0].title, "Buy milk");
        assert_eq!(doc.headings[0].keyword.as_deref(), Some("TODO"));
        assert_eq!(doc.headings[0].children.len(), 2);
        assert_eq!(doc.headings[0].children[0].title, "Whole milk");
        assert_eq!(doc.headings[1].title, "Laundry");
        assert_eq!(doc.headings[1].keyword.as_deref(), Some("DONE"));
    }

    #[test]
    fn parse_deep_nesting() {
        let input = "* L1\n** L2\n*** L3\n**** L4\n***** L5\n";
        let doc = parse(input);
        assert_eq!(doc.headings.len(), 1);
        assert_eq!(doc.headings[0].children.len(), 1);
        assert_eq!(doc.headings[0].children[0].children.len(), 1);
        assert_eq!(doc.headings[0].children[0].children[0].children.len(), 1);
        let l5 = &doc.headings[0].children[0].children[0].children[0].children[0];
        assert_eq!(l5.title, "L5");
        assert_eq!(l5.level, 5);
    }

    #[test]
    fn parse_tags_and_priority() {
        let input = "* TODO [#A] Urgent task :work:urgent:\n";
        let doc = parse(input);
        let h = &doc.headings[0];
        assert_eq!(h.keyword.as_deref(), Some("TODO"));
        assert_eq!(h.priority, Some('A'));
        assert_eq!(h.title, "Urgent task");
        assert_eq!(h.tags, vec!["work", "urgent"]);
    }

    #[test]
    fn parse_properties() {
        let input = "\
* Heading
:PROPERTIES:
:ID: abc123
:EFFORT: 2h
:END:
Body text here
";
        let doc = parse(input);
        let h = &doc.headings[0];
        assert_eq!(h.properties.len(), 2);
        assert_eq!(h.properties[0], ("ID".into(), "abc123".into()));
        assert_eq!(h.properties[1], ("EFFORT".into(), "2h".into()));
        assert!(h.body.contains("Body text here"));
    }

    #[test]
    fn parse_planning() {
        let input = "* TODO Task\nSCHEDULED: <2024-01-15 Mon>\nBody\n";
        let doc = parse(input);
        let h = &doc.headings[0];
        assert!(h.planning.as_ref().unwrap().contains("SCHEDULED"));
        assert!(h.body.contains("Body"));
    }

    #[test]
    fn parse_preamble() {
        let input = "#+TITLE: My file\n#+TODO: TODO NEXT | DONE\n\n* First\n";
        let doc = parse(input);
        assert!(doc.preamble.contains("#+TITLE: My file"));
        assert_eq!(doc.settings.todo_keywords, vec!["TODO", "NEXT"]);
        assert_eq!(doc.settings.done_keywords, vec!["DONE"]);
        assert_eq!(doc.headings[0].title, "First");
    }

    #[test]
    fn parse_skipped_levels() {
        let input = "* A\n*** B\n** C\n";
        let doc = parse(input);
        assert_eq!(doc.headings.len(), 1);
        assert_eq!(doc.headings[0].children.len(), 2);
        assert_eq!(doc.headings[0].children[0].title, "B");
        assert_eq!(doc.headings[0].children[0].level, 3);
        assert_eq!(doc.headings[0].children[1].title, "C");
        assert_eq!(doc.headings[0].children[1].level, 2);
    }

    #[test]
    fn parse_skipped_levels_two() {
        // Skip two levels: * → ****
        let input = "* A\n**** B\n** C\n";
        let doc = parse(input);
        assert_eq!(doc.headings[0].children.len(), 2);
        assert_eq!(doc.headings[0].children[0].title, "B");
        assert_eq!(doc.headings[0].children[0].level, 4);
        assert_eq!(doc.headings[0].children[1].title, "C");
        assert_eq!(doc.headings[0].children[1].level, 2);
    }

    #[test]
    fn parse_deep_then_shallow_zigzag() {
        let input = "\
* A
**** Deep1
** Mid1
**** Deep2
** Mid2
* B
";
        let doc = parse(input);
        assert_eq!(doc.headings.len(), 2);
        // Deep1 and Mid1 are children of A, but Deep2 nests under Mid1
        // because its level (4) > Mid1's level (2).
        assert_eq!(doc.headings[0].children.len(), 3);
        assert_eq!(doc.headings[0].children[0].title, "Deep1");
        assert_eq!(doc.headings[0].children[0].level, 4);
        assert_eq!(doc.headings[0].children[1].title, "Mid1");
        assert_eq!(doc.headings[0].children[1].level, 2);
        assert_eq!(doc.headings[0].children[1].children.len(), 1);
        assert_eq!(doc.headings[0].children[1].children[0].title, "Deep2");
        assert_eq!(doc.headings[0].children[2].title, "Mid2");
        assert_eq!(doc.headings[0].children[2].level, 2);
    }

    #[test]
    fn parse_jump_up_multiple_levels() {
        // Go deep then jump back to top
        let input = "* A\n** B\n*** C\n**** D\n* E\n";
        let doc = parse(input);
        assert_eq!(doc.headings.len(), 2);
        assert_eq!(doc.headings[0].title, "A");
        assert_eq!(doc.headings[1].title, "E");
        // D is a great-grandchild of A
        let d = &doc.headings[0].children[0].children[0].children[0];
        assert_eq!(d.title, "D");
    }

    #[test]
    fn parse_body_between_siblings() {
        // Body text after a child heading belongs to that child, not the parent
        let input = "* Parent\n** Child 1\n** Child 2\nText after child 2\n";
        let doc = parse(input);
        assert_eq!(doc.headings[0].body, "");
        assert_eq!(doc.headings[0].children.len(), 2);
        assert!(doc.headings[0].children[1].body.contains("Text after child 2"));
    }

    #[test]
    fn parse_parent_body_then_children() {
        let input = "* Parent\nParent body\n** Child\nChild body\n";
        let doc = parse(input);
        assert_eq!(doc.headings[0].body, "Parent body\n");
        assert_eq!(doc.headings[0].children[0].body, "Child body\n");
    }

    #[test]
    fn parse_body_with_blank_lines() {
        let input = "* Heading\n\nParagraph 1\n\nParagraph 2\n\n";
        let doc = parse(input);
        assert_eq!(doc.headings[0].body, "\nParagraph 1\n\nParagraph 2\n\n");
    }

    #[test]
    fn parse_no_trailing_newline() {
        let input = "* Heading\nBody";
        let doc = parse(input);
        assert_eq!(doc.headings[0].title, "Heading");
        assert_eq!(doc.headings[0].body, "Body\n");
    }

    #[test]
    fn parse_star_in_body_not_headline() {
        // `*bold*` in body text is not a headline
        let input = "* Heading\nThis is *bold* text\n*italic* here too\n";
        let doc = parse(input);
        assert_eq!(doc.headings.len(), 1);
        assert!(doc.headings[0].body.contains("*bold*"));
        assert!(doc.headings[0].body.contains("*italic*"));
    }

    #[test]
    fn parse_unicode_title() {
        let input = "* 日本語タイトル :タグ:\n本文テキスト\n";
        let doc = parse(input);
        // char::is_alphanumeric() is true for CJK, so :タグ: is parsed as a tag.
        assert_eq!(doc.headings[0].title, "日本語タイトル");
        assert_eq!(doc.headings[0].tags, vec!["タグ"]);
        assert!(doc.headings[0].body.contains("本文テキスト"));
    }

    #[test]
    fn parse_unicode_emoji_in_title() {
        let input = "* TODO 🚀 Launch day :release:\n";
        let doc = parse(input);
        assert_eq!(doc.headings[0].keyword.as_deref(), Some("TODO"));
        assert!(doc.headings[0].title.contains("🚀"));
        assert_eq!(doc.headings[0].tags, vec!["release"]);
    }

    #[test]
    fn parse_unicode_accented_latin() {
        let input = "* Café résumé naïve\n";
        let doc = parse(input);
        assert_eq!(doc.headings[0].title, "Café résumé naïve");
    }

    #[test]
    fn parse_unicode_cyrillic() {
        let input = "* TODO Задача :работа:\nТекст задачи\n";
        let doc = parse(input);
        assert_eq!(doc.headings[0].keyword.as_deref(), Some("TODO"));
        assert_eq!(doc.headings[0].title, "Задача");
        // Cyrillic chars are alphanumeric, so :работа: is parsed as a tag
        assert_eq!(doc.headings[0].tags, vec!["работа"]);
        assert!(doc.headings[0].body.contains("Текст"));
    }

    #[test]
    fn parse_unicode_tag_with_nonalpha_rejected() {
        // Tags with spaces or punctuation inside should NOT be parsed
        let input = "* Title :hello world:\n";
        let doc = parse(input);
        assert!(doc.headings[0].tags.is_empty());
        assert!(doc.headings[0].title.contains("hello world"));
    }

    #[test]
    fn parse_many_siblings() {
        let mut input = String::new();
        for i in 0..100 {
            input.push_str(&format!("* Heading {}\n", i));
        }
        let doc = parse(&input);
        assert_eq!(doc.headings.len(), 100);
        assert_eq!(doc.headings[99].title, "Heading 99");
    }

    #[test]
    fn parse_complex_interleaved_levels() {
        let input = "\
* A
** A1
*** A1a
** A2
* B
** B1
*** B1a
*** B1b
** B2
* C
";
        let doc = parse(input);
        assert_eq!(doc.headings.len(), 3);
        assert_eq!(doc.headings[0].children.len(), 2); // A1, A2
        assert_eq!(doc.headings[0].children[0].children.len(), 1); // A1a
        assert_eq!(doc.headings[1].children.len(), 2); // B1, B2
        assert_eq!(doc.headings[1].children[0].children.len(), 2); // B1a, B1b
        assert!(doc.headings[2].children.is_empty()); // C
    }

    #[test]
    fn parse_all_components_present() {
        let input = "\
* TODO [#A] Complex heading :work:urgent:
SCHEDULED: <2024-06-15 Sat> DEADLINE: <2024-06-20 Thu>
:PROPERTIES:
:ID: complex-1
:EFFORT: 4h
:CATEGORY: projects
:END:
First paragraph of body.

Second paragraph with a blank line above.
** TODO Subtask 1
** DONE Subtask 2
";
        let doc = parse(input);
        let h = &doc.headings[0];
        assert_eq!(h.keyword.as_deref(), Some("TODO"));
        assert_eq!(h.priority, Some('A'));
        assert_eq!(h.title, "Complex heading");
        assert_eq!(h.tags, vec!["work", "urgent"]);
        assert!(h.planning.as_ref().unwrap().contains("SCHEDULED"));
        assert!(h.planning.as_ref().unwrap().contains("DEADLINE"));
        assert_eq!(h.properties.len(), 3);
        assert!(h.body.contains("First paragraph"));
        assert!(h.body.contains("Second paragraph"));
        assert_eq!(h.children.len(), 2);
        assert_eq!(h.children[0].keyword.as_deref(), Some("TODO"));
        assert_eq!(h.children[1].keyword.as_deref(), Some("DONE"));
    }
}
