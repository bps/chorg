//! Edge-case tests covering parser, writer, path resolution, model helpers,
//! and operation correctness under unusual inputs.
//!
//! Each test targets a specific boundary, degenerate input, or tricky
//! combination that the main test suites don't exercise.

use chorg::model::{Heading, Settings};
use chorg::parser::{self, parse};
use chorg::path::resolve;
use chorg::writer::write;

// ===================================================================
// Helpers
// ===================================================================

fn h(level: usize, title: &str) -> Heading {
    Heading {
        level,
        keyword: None,
        priority: None,
        title: title.into(),
        tags: Vec::new(),
        planning: None,
        properties: Vec::new(),
        body: String::new(),
        children: Vec::new(),
    }
}

/// Parse → write round-trip assertion.
fn assert_roundtrip(input: &str) {
    let doc = parse(input);
    let out = write(&doc);
    assert_eq!(input, out, "\n--- INPUT ---\n{}\n--- OUTPUT ---\n{}", input, out);
}

/// Parse → write → parse → write stability assertion.
fn assert_stable(input: &str) {
    let first = write(&parse(input));
    let second = write(&parse(&first));
    assert_eq!(first, second, "not stable after two round-trips");
}

// ===================================================================
//  headline_level edge cases
// ===================================================================

#[test]
fn headline_tab_after_stars() {
    // Tab is not a space — not a valid headline
    assert_eq!(parser::headline_level("*\tTab"), None);
    assert_eq!(parser::headline_level("**\ttab"), None);
}

#[test]
fn headline_many_stars() {
    assert_eq!(parser::headline_level("********** Ten"), Some(10));
    let stars = "*".repeat(50);
    assert_eq!(parser::headline_level(&format!("{} Title", stars)), Some(50));
}

#[test]
fn headline_star_space_only() {
    // "* " with nothing after it — valid heading with empty title
    assert_eq!(parser::headline_level("* "), Some(1));
    assert_eq!(parser::headline_level("**  "), Some(2));
}

#[test]
fn headline_star_with_numbers() {
    assert_eq!(parser::headline_level("*1"), None);
    assert_eq!(parser::headline_level("*123"), None);
}

// ===================================================================
//  extract_tags edge cases
// ===================================================================

#[test]
fn tags_many_segments() {
    let doc = parse("* Title :a:b:c:d:e:f:g:h:\n");
    assert_eq!(doc.headings[0].tags.len(), 8);
    assert_eq!(doc.headings[0].tags[7], "h");
    assert_eq!(doc.headings[0].title, "Title");
}

#[test]
fn tags_single_char() {
    let doc = parse("* Title :x:\n");
    assert_eq!(doc.headings[0].tags, vec!["x"]);
}

#[test]
fn tags_hyphen_not_allowed() {
    // Hyphens are not in the allowed tag character set
    let doc = parse("* Title :work-item:\n");
    assert!(doc.headings[0].tags.is_empty());
    assert!(doc.headings[0].title.contains("work-item"));
}

#[test]
fn tags_dot_not_allowed() {
    let doc = parse("* Title :v1.0:\n");
    assert!(doc.headings[0].tags.is_empty());
}

#[test]
fn tags_multiple_spaces_before() {
    // Multiple spaces before tags — rfind(" :") still works
    let doc = parse("* Title   :work:\n");
    assert_eq!(doc.headings[0].tags, vec!["work"]);
    assert_eq!(doc.headings[0].title, "Title");
}

#[test]
fn tags_title_is_only_whitespace_before_tags() {
    // After "TODO" is extracted, the remaining text is "  :tag:".
    // rfind(" :") needs a space before the colon — but the title portion
    // is empty/whitespace, so the whole thing becomes the title text
    // because the space before : overlaps with the title's leading whitespace.
    let doc = parse("* TODO  :tag:\n");
    assert_eq!(doc.headings[0].keyword.as_deref(), Some("TODO"));
    // Depending on whitespace handling, :tag: may or may not parse as a tag.
    // With " :tag:" the rfind(" :") finds index 0 → title becomes "" and
    // tags become ["tag"]. But extract_tags receives "  :tag:" after trim
    // from parse_headline... let's just assert on what the parser actually does:
    // After keyword extraction rest is ":tag:" (trimmed). No space before : → no tags.
    assert_eq!(doc.headings[0].title, ":tag:");
    assert!(doc.headings[0].tags.is_empty());
}

// ===================================================================
//  Property drawer edge cases
// ===================================================================

#[test]
fn property_drawer_empty() {
    // Drawer with no properties inside
    let doc = parse("* H\n:PROPERTIES:\n:END:\n");
    assert!(doc.headings[0].properties.is_empty());
    assert_eq!(doc.headings[0].body, "");
}

#[test]
fn property_drawer_unclosed() {
    // No :END: — parser consumes everything as drawer entries
    let doc = parse("* H\n:PROPERTIES:\n:KEY: val\nBody line\n");
    // "Body line" has no colon-key pattern so parse_property_line returns None,
    // but the parser still eats it looking for :END:.
    // The body should be empty since everything was consumed inside the drawer.
    assert_eq!(doc.headings[0].body, "");
    assert_eq!(doc.headings[0].properties.len(), 1);
}

#[test]
fn property_drawer_indented() {
    // Org allows indented property drawers
    let doc = parse("* H\n  :PROPERTIES:\n  :ID: abc\n  :END:\nBody\n");
    assert_eq!(doc.headings[0].properties.len(), 1);
    assert_eq!(doc.headings[0].properties[0], ("ID".into(), "abc".into()));
    assert!(doc.headings[0].body.contains("Body"));
}

#[test]
fn property_key_with_underscore() {
    let doc = parse("* H\n:PROPERTIES:\n:CUSTOM_ID: foo\n:END:\n");
    assert_eq!(doc.headings[0].properties[0], ("CUSTOM_ID".into(), "foo".into()));
}

#[test]
fn property_key_with_plus() {
    // :KEY+: VALUE is an org append syntax. Our parser treats KEY+ as the key.
    let doc = parse("* H\n:PROPERTIES:\n:TAG+: extra\n:END:\n");
    assert_eq!(doc.headings[0].properties[0].0, "TAG+");
    assert_eq!(doc.headings[0].properties[0].1, "extra");
}

// ===================================================================
//  Planning line edge cases
// ===================================================================

#[test]
fn planning_with_leading_whitespace() {
    // Org allows indented planning lines
    let doc = parse("* TODO Task\n  SCHEDULED: <2024-01-01>\nBody\n");
    assert!(doc.headings[0].planning.is_some());
    assert!(doc.headings[0].body.contains("Body"));
}

#[test]
fn planning_only_no_body() {
    let doc = parse("* TODO Task\nSCHEDULED: <2024-01-01>\n");
    assert!(doc.headings[0].planning.is_some());
    assert_eq!(doc.headings[0].body, "");
}

#[test]
fn two_planning_like_lines() {
    // Only the first line is treated as planning; second becomes body
    let doc = parse("* H\nSCHEDULED: <2024-01-01>\nDEADLINE: <2024-02-01>\nBody\n");
    assert!(doc.headings[0].planning.as_ref().unwrap().contains("SCHEDULED"));
    // The DEADLINE line should be in the body
    assert!(doc.headings[0].body.contains("DEADLINE"));
}

#[test]
fn scheduled_word_in_body_not_planning() {
    // "SCHEDULED:" only counts if it's the first content line
    let doc = parse("* H\nSome body\nSCHEDULED: <2024-01-01>\n");
    assert!(doc.headings[0].planning.is_none());
    assert!(doc.headings[0].body.contains("SCHEDULED"));
}

// ===================================================================
//  Settings edge cases
// ===================================================================

#[test]
fn settings_empty_todo_line() {
    let doc = parse("#+TODO:\n\n* Hello\n");
    // Empty #+TODO: — found but no keywords
    assert!(doc.settings.todo_keywords.is_empty());
    assert!(doc.settings.done_keywords.is_empty());
    // "Hello" shouldn't be parsed as keyword (none configured)
    assert_eq!(doc.headings[0].keyword, None);
}

#[test]
fn settings_pipe_at_start() {
    let doc = parse("#+TODO: | DONE CANCELLED\n\n* DONE Task\n");
    assert!(doc.settings.todo_keywords.is_empty());
    assert_eq!(doc.settings.done_keywords, vec!["DONE", "CANCELLED"]);
    assert_eq!(doc.headings[0].keyword.as_deref(), Some("DONE"));
}

#[test]
fn settings_pipe_at_end() {
    let doc = parse("#+TODO: TODO NEXT |\n\n* TODO Task\n");
    assert_eq!(doc.settings.todo_keywords, vec!["TODO", "NEXT"]);
    assert!(doc.settings.done_keywords.is_empty());
}

#[test]
fn settings_typ_todo() {
    let doc = parse("#+TYP_TODO: BUG FEATURE | RESOLVED\n\n* BUG Crash\n");
    assert_eq!(doc.settings.todo_keywords, vec!["BUG", "FEATURE"]);
    assert_eq!(doc.settings.done_keywords, vec!["RESOLVED"]);
    assert_eq!(doc.headings[0].keyword.as_deref(), Some("BUG"));
}

#[test]
fn settings_case_sensitive() {
    // #+todo: (lowercase) should NOT be recognized
    let doc = parse("#+todo: OPEN | CLOSED\n\n* OPEN Task\n");
    // Falls through to defaults since #+todo: isn't matched
    assert!(doc.settings.is_keyword("TODO"));
    // OPEN is not a default keyword
    assert_eq!(doc.headings[0].keyword, None);
}

#[test]
fn settings_in_body_not_picked_up() {
    // #+TODO: inside body shouldn't affect settings
    let doc = parse("* H\n#+TODO: OPEN | CLOSED\n");
    assert!(doc.settings.is_keyword("TODO")); // defaults
    assert!(!doc.settings.is_keyword("OPEN"));
}

// ===================================================================
//  Document structure edge cases
// ===================================================================

#[test]
fn only_newlines() {
    let doc = parse("\n\n\n");
    assert!(doc.headings.is_empty());
    // lines() on "\n\n\n" gives ["", "", ""], joined back → "\n\n\n"
    assert_eq!(doc.preamble, "\n\n\n");
}

#[test]
fn only_whitespace() {
    let doc = parse("   \n  \n");
    assert!(doc.headings.is_empty());
}

#[test]
fn heading_immediately_at_start() {
    let doc = parse("* First\n");
    assert_eq!(doc.preamble, "");
    assert_eq!(doc.headings[0].title, "First");
}

#[test]
fn heading_level2_with_no_level1_parent() {
    // Level-2 heading at top level — unusual but must be handled
    let doc = parse("** Orphan\n*** Child\n");
    assert_eq!(doc.headings.len(), 1);
    assert_eq!(doc.headings[0].title, "Orphan");
    assert_eq!(doc.headings[0].level, 2);
    assert_eq!(doc.headings[0].children.len(), 1);
    assert_eq!(doc.headings[0].children[0].title, "Child");
}

#[test]
fn heading_with_extra_spaces_after_stars() {
    let doc = parse("*   Spaced out\n");
    assert_eq!(doc.headings[0].title, "Spaced out");
}

#[test]
fn heading_title_with_tab() {
    let doc = parse("* Title\twith\ttabs\n");
    assert_eq!(doc.headings[0].title, "Title\twith\ttabs");
}

#[test]
fn heading_title_with_slashes() {
    // Slashes in titles — relevant since / is the path separator
    let doc = parse("* path/to/thing\n");
    assert_eq!(doc.headings[0].title, "path/to/thing");
}

#[test]
fn heading_title_with_hash() {
    // Hash in title — relevant since # is the positional prefix
    let doc = parse("* Issue #42\n");
    assert_eq!(doc.headings[0].title, "Issue #42");
}

#[test]
fn two_identical_subtrees() {
    let doc = parse("* A\n** Child\n* A\n** Child\n");
    assert_eq!(doc.headings.len(), 2);
    assert_eq!(doc.headings[0].title, "A");
    assert_eq!(doc.headings[1].title, "A");
    assert_eq!(doc.headings[0].children[0].title, "Child");
    assert_eq!(doc.headings[1].children[0].title, "Child");
}

#[test]
fn very_long_title() {
    let long = "A".repeat(10_000);
    let input = format!("* {}\n", long);
    let doc = parse(&input);
    assert_eq!(doc.headings[0].title.len(), 10_000);
}

#[test]
fn body_starts_with_blank_lines() {
    let doc = parse("* H\n\n\nBody here\n");
    assert_eq!(doc.headings[0].body, "\n\nBody here\n");
}

#[test]
fn body_is_only_blank_lines() {
    let doc = parse("* H\n\n\n\n");
    assert_eq!(doc.headings[0].body, "\n\n\n");
}

#[test]
fn consecutive_blank_lines_between_headings() {
    let doc = parse("* A\n\n\n\n* B\n");
    assert_eq!(doc.headings.len(), 2);
    assert_eq!(doc.headings[0].body, "\n\n\n");
    assert_eq!(doc.headings[1].body, "");
}

#[test]
fn body_containing_properties_syntax() {
    // :PROPERTIES: in body (not after headline) is just text
    let doc = parse("* H\nSome text\n:PROPERTIES:\n:ID: x\n:END:\n");
    assert!(doc.headings[0].properties.is_empty());
    assert!(doc.headings[0].body.contains(":PROPERTIES:"));
    assert!(doc.headings[0].body.contains(":ID: x"));
}

#[test]
fn file_ending_no_newline_heading_only() {
    let doc = parse("* H");
    assert_eq!(doc.headings[0].title, "H");
    assert_eq!(doc.headings[0].body, "");
}

#[test]
fn file_ending_no_newline_with_body() {
    let doc = parse("* H\nBody");
    assert_eq!(doc.headings[0].body, "Body\n");
}

#[test]
fn file_ending_no_newline_multiple_headings() {
    let doc = parse("* A\n* B");
    assert_eq!(doc.headings.len(), 2);
    assert_eq!(doc.headings[1].title, "B");
}

// ===================================================================
//  Round-trip edge cases
// ===================================================================

#[test]
fn roundtrip_heading_level2_no_parent() {
    assert_roundtrip("** Orphan\n*** Child\n");
}

#[test]
fn roundtrip_only_newlines_in_preamble() {
    assert_stable("\n\n\n* H\n");
}

#[test]
fn roundtrip_planning_only_no_body() {
    assert_roundtrip("* TODO H\nSCHEDULED: <2024-01-01>\n");
}

#[test]
fn roundtrip_planning_and_properties_no_body() {
    assert_roundtrip("\
* TODO H
SCHEDULED: <2024-01-01>
:PROPERTIES:
:ID: x
:END:
");
}

#[test]
fn roundtrip_empty_property_drawer() {
    // Empty drawer isn't emitted by writer (no properties → no drawer),
    // so this is a stability test rather than strict round-trip.
    let input = "* H\n:PROPERTIES:\n:END:\n";
    let doc = parse(input);
    let out = write(&doc);
    let doc2 = parse(&out);
    let out2 = write(&doc2);
    assert_eq!(out, out2);
}

#[test]
fn roundtrip_property_value_with_equals() {
    assert_roundtrip("* H\n:PROPERTIES:\n:CMD: x=1 y=2\n:END:\n");
}

#[test]
fn roundtrip_body_with_drawer_syntax() {
    assert_roundtrip("* H\nSome text\n:LOGBOOK:\nCLOCK: ...\n:END:\n");
}

#[test]
fn roundtrip_consecutive_blank_lines() {
    assert_roundtrip("* A\n\n\n\n* B\n");
}

#[test]
fn roundtrip_many_siblings_many_levels() {
    let mut input = String::new();
    for i in 0..5 {
        input.push_str(&format!("* H{}\n", i));
        for j in 0..3 {
            input.push_str(&format!("** C{}_{}\n", i, j));
            for k in 0..2 {
                input.push_str(&format!("*** G{}_{}_{}\n", i, j, k));
            }
        }
    }
    assert_roundtrip(&input);
}

#[test]
fn roundtrip_kitchen_sink() {
    assert_roundtrip("\
#+TITLE: Edge cases
#+TODO: TODO NEXT WAITING | DONE CANCELLED

* TODO [#A] First heading :work:urgent:
SCHEDULED: <2024-06-15 Sat> DEADLINE: <2024-06-20 Thu>
:PROPERTIES:
:ID: h1
:EFFORT: 4h
:EMPTY:
:URL: https://example.com:8080/path?q=1&r=2
:END:
Body paragraph one.

Body paragraph two with *bold* and /italic/.

#+BEGIN_SRC python
print(\"hello\")
#+END_SRC
** TODO [#B] Sub 1 :code:
*** DONE Sub sub
CLOSED: [2024-05-01 Wed]
** NEXT Sub 2
** Sub 3
* Heading with no keyword
Some body.
** Child
* DONE Complete :archive:
CLOSED: [2024-06-01 Sat 10:00]
:PROPERTIES:
:ARCHIVE_TIME: 2024-06-01
:END:
");
}

// ===================================================================
//  Path resolution edge cases
// ===================================================================

#[test]
fn path_hash_only() {
    let doc = parse("* H\n");
    let err = resolve(&doc, "#").unwrap_err();
    let msg = format!("{:#}", err);
    assert!(msg.contains("invalid"), "error was: {}", msg);
}

#[test]
fn path_negative_number() {
    let doc = parse("* H\n");
    let err = resolve(&doc, "#-1").unwrap_err();
    let msg = format!("{:#}", err);
    assert!(msg.contains("invalid"), "error was: {}", msg);
}

#[test]
fn path_trailing_slash() {
    let doc = parse("* H\n** C\n");
    let err = resolve(&doc, "H/").unwrap_err();
    let msg = format!("{:#}", err);
    assert!(msg.contains("empty path segment"), "error was: {}", msg);
}

#[test]
fn path_leading_slash() {
    let doc = parse("* H\n");
    let err = resolve(&doc, "/H").unwrap_err();
    let msg = format!("{:#}", err);
    assert!(msg.contains("empty path segment"), "error was: {}", msg);
}

#[test]
fn path_title_with_slash() {
    // Title containing a slash — can't be addressed by title path
    // but can be addressed positionally
    let doc = parse("* path/thing\n");
    // "path/thing" splits into ["path", "thing"] so it fails
    assert!(resolve(&doc, "#1").is_ok());
    assert_eq!(resolve(&doc, "#1").unwrap(), vec![0]);
}

#[test]
fn path_title_with_hash() {
    // Title starting with # — might conflict with positional syntax
    let doc = parse("* #42 Bug\n");
    // This will be interpreted as positional index 42, which is out of range
    assert!(resolve(&doc, "#42 Bug").is_err());
    // But positional #1 works
    assert_eq!(resolve(&doc, "#1").unwrap(), vec![0]);
}

#[test]
fn path_single_heading_doc() {
    let doc = parse("* Only\n");
    assert_eq!(resolve(&doc, "Only").unwrap(), vec![0]);
    assert_eq!(resolve(&doc, "#1").unwrap(), vec![0]);
}

#[test]
fn path_exact_wins_over_multiple_substrings() {
    // "A" matches exactly "A" even though "AB" and "AC" also contain "A"
    let doc = parse("* AB\n* A\n* AC\n");
    assert_eq!(resolve(&doc, "A").unwrap(), vec![1]);
}

#[test]
fn path_at_boundary_index() {
    let doc = parse("* A\n* B\n* C\n");
    // First and last valid indices
    assert_eq!(resolve(&doc, "#1").unwrap(), vec![0]);
    assert_eq!(resolve(&doc, "#3").unwrap(), vec![2]);
    // One past the end
    assert!(resolve(&doc, "#4").is_err());
}

#[test]
fn path_very_deep() {
    let input = "* L1\n** L2\n*** L3\n**** L4\n***** L5\n****** L6\n******* L7\n";
    let doc = parse(input);
    assert_eq!(
        resolve(&doc, "L1/L2/L3/L4/L5/L6/L7").unwrap(),
        vec![0, 0, 0, 0, 0, 0, 0]
    );
    assert_eq!(
        resolve(&doc, "#1/#1/#1/#1/#1/#1/#1").unwrap(),
        vec![0, 0, 0, 0, 0, 0, 0]
    );
}

// ===================================================================
//  Model helper edge cases
// ===================================================================

#[test]
fn walk_flat_siblings_only() {
    let doc = parse("* A\n* B\n* C\n");
    let walked = doc.walk();
    assert_eq!(walked.len(), 3);
    assert_eq!(walked[0].0, vec![0]);
    assert_eq!(walked[1].0, vec![1]);
    assert_eq!(walked[2].0, vec![2]);
}

#[test]
fn walk_deep_chain() {
    let doc = parse("* A\n** B\n*** C\n**** D\n");
    let walked = doc.walk();
    assert_eq!(walked.len(), 4);
    assert_eq!(walked[3].0, vec![0, 0, 0, 0]);
    assert_eq!(walked[3].1.title, "D");
}

#[test]
fn heading_at_mut_deep_modification() {
    let mut doc = parse("* A\n** B\n*** C\n**** D\n");
    doc.heading_at_mut(&[0, 0, 0, 0]).keyword = Some("DONE".into());
    assert_eq!(doc.heading_at(&[0, 0, 0, 0]).keyword.as_deref(), Some("DONE"));
}

#[test]
fn parent_list_mut_deep() {
    let mut doc = parse("* A\n** B\n*** C\n**** D1\n**** D2\n");
    let (list, idx) = doc.parent_list_mut(&[0, 0, 0, 1]);
    assert_eq!(idx, 1);
    assert_eq!(list[idx].title, "D2");
    assert_eq!(list.len(), 2);
}

#[test]
fn format_addr_deep() {
    assert_eq!(Heading::format_addr(&[0, 0, 0, 0, 0]), "#1.1.1.1.1");
    assert_eq!(Heading::format_addr(&[9, 8, 7]), "#10.9.8");
}

#[test]
fn settings_empty_lists() {
    let s = Settings {
        todo_keywords: Vec::new(),
        done_keywords: Vec::new(),
    };
    assert!(!s.is_keyword("TODO"));
    assert!(!s.is_done("DONE"));
    assert!(s.all_keywords().is_empty());
}

// ===================================================================
//  Operations on degenerate documents
// ===================================================================

#[test]
fn insert_into_empty_doc() {
    let mut doc = parse("");
    doc.headings.push(h(1, "First"));
    let text = write(&doc);
    let doc2 = parse(&text);
    assert_eq!(doc2.headings.len(), 1);
    assert_eq!(doc2.headings[0].title, "First");
}

#[test]
fn insert_into_preamble_only_doc() {
    let mut doc = parse("#+TITLE: Test\n");
    doc.headings.push(h(1, "New"));
    let text = write(&doc);
    assert!(text.contains("#+TITLE: Test"));
    assert!(text.contains("* New"));
}

#[test]
fn delete_last_heading() {
    let mut doc = parse("* Only\n");
    doc.headings.remove(0);
    let text = write(&doc);
    // No headings, should produce empty or just trailing newline
    let doc2 = parse(&text);
    assert!(doc2.headings.is_empty());
}

#[test]
fn delete_last_heading_preserves_preamble() {
    let mut doc = parse("#+TITLE: Keep me\n\n* Gone\n");
    doc.headings.remove(0);
    let text = write(&doc);
    assert!(text.contains("#+TITLE: Keep me"));
    assert!(!text.contains("Gone"));
}

#[test]
fn move_to_same_parent() {
    // Move a child to a different position under the same parent
    let mut doc = parse("* P\n** A\n** B\n** C\n");
    let heading = doc.headings[0].children.remove(2); // remove C
    doc.headings[0].children.insert(0, heading); // insert C first
    let text = write(&doc);
    let doc2 = parse(&text);
    assert_eq!(doc2.headings[0].children[0].title, "C");
    assert_eq!(doc2.headings[0].children[1].title, "A");
    assert_eq!(doc2.headings[0].children[2].title, "B");
}

#[test]
fn move_first_to_last() {
    let mut doc = parse("* P\n** A\n** B\n** C\n");
    let heading = doc.headings[0].children.remove(0); // remove A
    doc.headings[0].children.push(heading); // append A
    let text = write(&doc);
    let doc2 = parse(&text);
    assert_eq!(doc2.headings[0].children[0].title, "B");
    assert_eq!(doc2.headings[0].children[1].title, "C");
    assert_eq!(doc2.headings[0].children[2].title, "A");
}

#[test]
fn edit_title_to_empty() {
    let mut doc = parse("* TODO Original\n");
    doc.heading_at_mut(&[0]).title = String::new();
    let text = write(&doc);
    assert!(text.contains("* TODO \n") || text.contains("* TODO"));
    let doc2 = parse(&text);
    assert_eq!(doc2.headings[0].keyword.as_deref(), Some("TODO"));
}

#[test]
fn edit_title_with_special_chars() {
    let mut doc = parse("* Original\n");
    doc.heading_at_mut(&[0]).title = "Title with :colons: and [brackets] and *stars*".into();
    let text = write(&doc);
    let doc2 = parse(&text);
    // Colons might be parsed as tags depending on format
    assert!(doc2.headings[0].title.contains("brackets"));
}

#[test]
fn property_very_long_value() {
    let long_val = "x".repeat(5000);
    let mut doc = parse("* H\n");
    doc.headings[0].properties.push(("LONG".into(), long_val.clone()));
    let text = write(&doc);
    let doc2 = parse(&text);
    assert_eq!(doc2.headings[0].properties[0].1, long_val);
}

#[test]
fn consecutive_inserts_at_position_1() {
    let mut doc = parse("* P\n** Existing\n");
    for i in 0..5 {
        let heading = h(2, &format!("Insert_{}", i));
        doc.headings[0].children.insert(0, heading);
    }
    let text = write(&doc);
    let doc2 = parse(&text);
    // Inserted in reverse order at position 0
    assert_eq!(doc2.headings[0].children[0].title, "Insert_4");
    assert_eq!(doc2.headings[0].children[4].title, "Insert_0");
    assert_eq!(doc2.headings[0].children[5].title, "Existing");
}

#[test]
fn delete_then_reinsert() {
    let mut doc = parse("* P\n** A\n** B\n** C\n");
    let heading = doc.headings[0].children.remove(1); // remove B
    assert_eq!(doc.headings[0].children.len(), 2);
    doc.headings[0].children.insert(1, heading); // put B back
    let text = write(&doc);
    let doc2 = parse(&text);
    assert_eq!(doc2.headings[0].children[0].title, "A");
    assert_eq!(doc2.headings[0].children[1].title, "B");
    assert_eq!(doc2.headings[0].children[2].title, "C");
}

#[test]
fn modify_heading_siblings_unchanged() {
    let mut doc = parse("* A\nBody A\n* B\nBody B\n* C\nBody C\n");
    doc.heading_at_mut(&[1]).title = "B Modified".into();
    doc.heading_at_mut(&[1]).keyword = Some("DONE".into());
    // A and C should be untouched
    let text = write(&doc);
    let doc2 = parse(&text);
    assert_eq!(doc2.headings[0].title, "A");
    assert_eq!(doc2.headings[0].body, "Body A\n");
    assert_eq!(doc2.headings[1].title, "B Modified");
    assert_eq!(doc2.headings[1].keyword.as_deref(), Some("DONE"));
    assert_eq!(doc2.headings[2].title, "C");
    assert_eq!(doc2.headings[2].body, "Body C\n");
}

#[test]
fn walk_after_mutation() {
    let mut doc = parse("* A\n** A1\n* B\n");
    doc.headings[0].children.push(h(2, "A2"));
    let walked = doc.walk();
    let titles: Vec<&str> = walked.iter().map(|(_, h)| h.title.as_str()).collect();
    assert_eq!(titles, vec!["A", "A1", "A2", "B"]);
}

#[test]
fn multiple_tag_add_remove_simultaneously() {
    let mut doc = parse("* H :a:b:c:\n");
    let h = doc.heading_at_mut(&[0]);
    // Add d and e
    h.tags.push("d".into());
    h.tags.push("e".into());
    // Remove b and c
    h.tags.retain(|t| t != "b" && t != "c");
    let text = write(&doc);
    let doc2 = parse(&text);
    assert_eq!(doc2.headings[0].tags, vec!["a", "d", "e"]);
}

#[test]
fn clear_then_add_tags() {
    let mut doc = parse("* H :old1:old2:\n");
    let h = doc.heading_at_mut(&[0]);
    h.tags.clear();
    h.tags.push("new".into());
    let text = write(&doc);
    let doc2 = parse(&text);
    assert_eq!(doc2.headings[0].tags, vec!["new"]);
}

#[test]
fn body_with_org_directives() {
    let input = "\
* H
#+BEGIN_QUOTE
Some quoted text
#+END_QUOTE
#+RESULTS:
: output line
";
    assert_roundtrip(input);
}

#[test]
fn body_with_org_links() {
    let input = "* H\nSee [[https://example.com][Example]] and [[file:./other.org]].\n";
    assert_roundtrip(input);
}

#[test]
fn body_with_list_items() {
    let input = "\
* H
- item 1
- item 2
  - sub item
- item 3
  1. numbered
  2. also numbered
";
    assert_roundtrip(input);
}

#[test]
fn body_with_tables() {
    let input = "\
* H
| Name  | Age |
|-------+-----|
| Alice |  30 |
| Bob   |  25 |
";
    assert_roundtrip(input);
}

// ===================================================================
//  Serialization edge cases (JSON output)
// ===================================================================

#[test]
fn json_serialization_empty_doc() {
    let doc = parse("");
    let json = serde_json::to_value(&doc).unwrap();
    assert!(json["headings"].as_array().unwrap().is_empty());
}

#[test]
fn json_serialization_omits_empty_fields() {
    let doc = parse("* Title\n");
    let json = serde_json::to_value(&doc.headings[0]).unwrap();
    // Optional fields should be absent (skip_serializing_if)
    assert!(json.get("keyword").is_none());
    assert!(json.get("priority").is_none());
    assert!(json.get("planning").is_none());
    assert!(json.get("tags").is_none());
    assert!(json.get("properties").is_none());
    assert!(json.get("body").is_none());
    assert!(json.get("children").is_none());
    // But title and level are always present
    assert_eq!(json["title"], "Title");
    assert_eq!(json["level"], 1);
}

#[test]
fn json_serialization_all_fields_present() {
    let doc = parse("\
* TODO [#A] Task :work:
SCHEDULED: <2024-01-01>
:PROPERTIES:
:ID: x
:END:
Body
** Child
");
    let json = serde_json::to_value(&doc.headings[0]).unwrap();
    assert_eq!(json["keyword"], "TODO");
    assert_eq!(json["priority"], "A");
    assert_eq!(json["tags"][0], "work");
    assert!(json["planning"].as_str().unwrap().contains("SCHEDULED"));
    assert_eq!(json["properties"][0][0], "ID");
    assert!(json["body"].as_str().unwrap().contains("Body"));
    assert_eq!(json["children"][0]["title"], "Child");
}

// ===================================================================
//  Combined stress tests
// ===================================================================

#[test]
fn stress_many_mutations_stability() {
    let mut doc = parse("* Root\n");

    // Insert 20 children
    for i in 0..20 {
        doc.headings[0].children.push(Heading {
            level: 2,
            keyword: if i % 3 == 0 { Some("TODO".into()) } else { None },
            priority: if i % 5 == 0 { Some('A') } else { None },
            title: format!("Child {}", i),
            tags: if i % 2 == 0 { vec!["even".into()] } else { Vec::new() },
            planning: None,
            properties: if i % 4 == 0 {
                vec![("N".into(), i.to_string())]
            } else {
                Vec::new()
            },
            body: if i % 3 == 1 { format!("Body of child {}\n", i) } else { String::new() },
            children: Vec::new(),
        });
    }

    // Delete every third child (in reverse to maintain indices)
    for i in (0..20).rev() {
        if i % 3 == 2 {
            doc.headings[0].children.remove(i);
        }
    }

    // Modify some
    for child in &mut doc.headings[0].children {
        if child.keyword.is_some() {
            child.keyword = Some("DONE".into());
        }
    }

    // Round-trip stability
    let text = write(&doc);
    let doc2 = parse(&text);
    let text2 = write(&doc2);
    assert_eq!(text, text2);

    // Verify structure
    let remaining: Vec<&str> = doc2.headings[0]
        .children
        .iter()
        .map(|h| h.title.as_str())
        .collect();
    // Children 2, 5, 8, 11, 14, 17 were deleted (i % 3 == 2)
    assert!(!remaining.contains(&"Child 2"));
    assert!(!remaining.contains(&"Child 5"));
    assert!(remaining.contains(&"Child 0"));
    assert!(remaining.contains(&"Child 1"));
}

#[test]
fn stress_deep_nesting_roundtrip() {
    let mut input = String::new();
    for i in 1..=20 {
        input.push_str(&"*".repeat(i));
        input.push_str(&format!(" Level {}\n", i));
    }
    assert_roundtrip(&input);
}

#[test]
fn stress_wide_tree_roundtrip() {
    let mut input = String::new();
    for i in 0..50 {
        input.push_str(&format!("* Heading {}\n", i));
        for j in 0..10 {
            input.push_str(&format!("** Child {}_{}\n", i, j));
        }
    }
    assert_roundtrip(&input);
    let doc = parse(&input);
    assert_eq!(doc.headings.len(), 50);
    assert_eq!(doc.walk().len(), 550);
}

#[test]
fn stress_path_resolution_on_wide_tree() {
    let mut input = String::new();
    for i in 0..100 {
        input.push_str(&format!("* H{}\n", i));
    }
    let doc = parse(&input);
    assert_eq!(resolve(&doc, "#1").unwrap(), vec![0]);
    assert_eq!(resolve(&doc, "#50").unwrap(), vec![49]);
    assert_eq!(resolve(&doc, "#100").unwrap(), vec![99]);
    assert!(resolve(&doc, "#101").is_err());
    assert_eq!(resolve(&doc, "H99").unwrap(), vec![99]);
}
