use crate::model::{Heading, OrgDoc};

/// Serialise an `OrgDoc` back to org-mode text.
pub fn write(doc: &OrgDoc) -> String {
    let mut out = String::new();

    if !doc.preamble.is_empty() {
        out.push_str(&doc.preamble);
    }

    for h in &doc.headings {
        write_heading(&mut out, h);
    }

    // Ensure trailing newline.
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }

    out
}

fn write_heading(out: &mut String, h: &Heading) {
    // Headline
    out.push_str(&format_headline(h));
    out.push('\n');

    // Planning
    if let Some(ref p) = h.planning {
        out.push_str(p);
        out.push('\n');
    }

    // Property drawer
    if !h.properties.is_empty() {
        out.push_str(":PROPERTIES:\n");
        for (key, value) in &h.properties {
            if value.is_empty() {
                out.push_str(&format!(":{}:\n", key));
            } else {
                out.push_str(&format!(":{}: {}\n", key, value));
            }
        }
        out.push_str(":END:\n");
    }

    // Body (already contains its own newlines)
    if !h.body.is_empty() {
        out.push_str(&h.body);
    }

    // Children
    for child in &h.children {
        write_heading(out, child);
    }
}

/// Format a single headline line (without trailing newline).
pub fn format_headline(h: &Heading) -> String {
    let mut line = "*".repeat(h.level);
    line.push(' ');

    if let Some(ref kw) = h.keyword {
        line.push_str(kw);
        line.push(' ');
    }

    if let Some(p) = h.priority {
        line.push_str(&format!("[#{}] ", p));
    }

    line.push_str(&h.title);

    if !h.tags.is_empty() {
        line.push_str(&format!(" :{}:", h.tags.join(":")));
    }

    line
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Settings;
    use crate::parser::parse;

    /// Helper: parse → write, assert output matches input exactly.
    fn assert_roundtrip(input: &str) {
        let doc = parse(input);
        let output = write(&doc);
        assert_eq!(input, output, "\n--- INPUT ---\n{}\n--- OUTPUT ---\n{}", input, output);
    }

    // -----------------------------------------------------------------------
    // Round-trip identity tests
    // -----------------------------------------------------------------------

    #[test]
    fn roundtrip_single_heading() {
        assert_roundtrip("* Heading\n");
    }

    #[test]
    fn roundtrip_simple() {
        assert_roundtrip("* TODO Buy milk\n** Whole milk\n** Skim milk\n* DONE Laundry\n");
    }

    #[test]
    fn roundtrip_with_body() {
        assert_roundtrip("\
* Heading
Body line 1
Body line 2
** Child
Child body
");
    }

    #[test]
    fn roundtrip_with_properties() {
        assert_roundtrip("\
* Heading
:PROPERTIES:
:ID: abc
:END:
Body
");
    }

    #[test]
    fn roundtrip_with_preamble() {
        assert_roundtrip("\
#+TITLE: Test
#+TODO: TODO | DONE

* First heading
");
    }

    #[test]
    fn roundtrip_tags_priority() {
        assert_roundtrip("* TODO [#A] Urgent :work:urgent:\n");
    }

    #[test]
    fn roundtrip_planning() {
        assert_roundtrip("\
* TODO Task
SCHEDULED: <2024-01-15 Mon>
Body text
");
    }

    #[test]
    fn roundtrip_planning_plus_properties_plus_body() {
        assert_roundtrip("\
* TODO Important
DEADLINE: <2024-06-01 Sat>
:PROPERTIES:
:EFFORT: 4h
:ID: imp-1
:END:
This is the body.

It has multiple paragraphs.
");
    }

    #[test]
    fn roundtrip_deep_nesting() {
        assert_roundtrip("* L1\n** L2\n*** L3\n**** L4\n***** L5\n");
    }

    #[test]
    fn roundtrip_body_with_blank_lines() {
        assert_roundtrip("\
* Heading

Paragraph 1

Paragraph 2

");
    }

    #[test]
    fn roundtrip_multiple_headings_with_bodies() {
        assert_roundtrip("\
* Heading 1
Body 1
** Child 1a
Child 1a body
** Child 1b
* Heading 2
Body 2
");
    }

    #[test]
    fn roundtrip_property_empty_value() {
        assert_roundtrip("\
* Heading
:PROPERTIES:
:EMPTY:
:END:
");
    }

    #[test]
    fn roundtrip_property_mixed_empty_and_nonempty() {
        assert_roundtrip("\
* Heading
:PROPERTIES:
:ID: abc
:MARKER:
:EFFORT: 2h
:END:
");
    }

    #[test]
    fn roundtrip_property_value_with_colons() {
        assert_roundtrip("\
* Heading
:PROPERTIES:
:URL: https://example.com:8080/path
:END:
");
    }

    #[test]
    fn roundtrip_many_properties() {
        assert_roundtrip("\
* Heading
:PROPERTIES:
:ID: abc
:EFFORT: 2h
:CATEGORY: work
:ASSIGNED: alice
:PRIORITY: high
:END:
");
    }

    #[test]
    fn roundtrip_siblings_no_body() {
        assert_roundtrip("* A\n* B\n* C\n* D\n");
    }

    #[test]
    fn roundtrip_complex_tree() {
        assert_roundtrip("\
#+TITLE: Complex
#+TODO: TODO NEXT | DONE CANCELLED

* TODO [#A] Project Alpha :work:
SCHEDULED: <2024-06-15 Sat>
:PROPERTIES:
:ID: alpha
:EFFORT: 40h
:END:
Main project description.

** TODO Design phase :design:
*** NEXT Create wireframes
*** TODO Review with team
** DONE Implementation :code:
CLOSED: [2024-05-01 Wed 10:00]
All done.
** TODO Testing
* Inbox
** Read article
** TODO Call dentist :personal:
* Archive
");
    }

    #[test]
    fn roundtrip_skipped_levels() {
        assert_roundtrip("* A\n*** B\n** C\n");
    }

    #[test]
    fn roundtrip_blank_lines_between_headings() {
        // Blank lines before a heading are part of the previous heading's body
        assert_roundtrip("\
* First
Body

* Second
");
    }

    #[test]
    fn roundtrip_preamble_only() {
        assert_roundtrip("#+TITLE: Just a preamble\nSome text\n");
    }

    // -----------------------------------------------------------------------
    // format_headline unit tests
    // -----------------------------------------------------------------------

    #[test]
    fn format_headline_all_fields() {
        let h = Heading {
            level: 2,
            keyword: Some("TODO".into()),
            priority: Some('A'),
            title: "Task".into(),
            tags: vec!["work".into(), "urgent".into()],
            planning: None,
            properties: Vec::new(),
            body: String::new(),
            children: Vec::new(),
        };
        assert_eq!(format_headline(&h), "** TODO [#A] Task :work:urgent:");
    }

    #[test]
    fn format_headline_title_only() {
        let h = Heading {
            level: 1,
            keyword: None,
            priority: None,
            title: "Simple".into(),
            tags: Vec::new(),
            planning: None,
            properties: Vec::new(),
            body: String::new(),
            children: Vec::new(),
        };
        assert_eq!(format_headline(&h), "* Simple");
    }

    #[test]
    fn format_headline_keyword_only() {
        let h = Heading {
            level: 1,
            keyword: Some("DONE".into()),
            priority: None,
            title: "".into(),
            tags: Vec::new(),
            planning: None,
            properties: Vec::new(),
            body: String::new(),
            children: Vec::new(),
        };
        assert_eq!(format_headline(&h), "* DONE ");
    }

    #[test]
    fn format_headline_keyword_and_title_no_tags() {
        let h = Heading {
            level: 1,
            keyword: Some("TODO".into()),
            priority: None,
            title: "Simple task".into(),
            tags: Vec::new(),
            planning: None,
            properties: Vec::new(),
            body: String::new(),
            children: Vec::new(),
        };
        assert_eq!(format_headline(&h), "* TODO Simple task");
    }

    #[test]
    fn format_headline_single_tag() {
        let h = Heading {
            level: 2,
            keyword: None,
            priority: None,
            title: "Tagged".into(),
            tags: vec!["work".into()],
            planning: None,
            properties: Vec::new(),
            body: String::new(),
            children: Vec::new(),
        };
        assert_eq!(format_headline(&h), "** Tagged :work:");
    }

    #[test]
    fn format_headline_deep_level() {
        let h = Heading {
            level: 5,
            keyword: Some("NEXT".into()),
            priority: None,
            title: "Deep".into(),
            tags: Vec::new(),
            planning: None,
            properties: Vec::new(),
            body: String::new(),
            children: Vec::new(),
        };
        assert_eq!(format_headline(&h), "***** NEXT Deep");
    }

    // -----------------------------------------------------------------------
    // Write from scratch (not round-trip)
    // -----------------------------------------------------------------------

    #[test]
    fn write_empty_doc() {
        let doc = OrgDoc {
            preamble: String::new(),
            headings: Vec::new(),
            settings: Settings::default(),
        };
        assert_eq!(write(&doc), "");
    }

    #[test]
    fn write_preamble_only() {
        let doc = OrgDoc {
            preamble: "#+TITLE: Hello\n".into(),
            headings: Vec::new(),
            settings: Settings::default(),
        };
        assert_eq!(write(&doc), "#+TITLE: Hello\n");
    }

    #[test]
    fn write_ensures_trailing_newline() {
        // Even if body doesn't end with newline, output should
        let doc = OrgDoc {
            preamble: String::new(),
            headings: vec![Heading {
                level: 1,
                keyword: None,
                priority: None,
                title: "Test".into(),
                tags: Vec::new(),
                planning: None,
                properties: Vec::new(),
                body: String::new(),
                children: Vec::new(),
            }],
            settings: Settings::default(),
        };
        let output = write(&doc);
        assert!(output.ends_with('\n'));
    }

    // -----------------------------------------------------------------------
    // Stability: write(parse(write(parse(x)))) == write(parse(x))
    // -----------------------------------------------------------------------

    #[test]
    fn double_roundtrip_stability() {
        let input = "\
#+TITLE: Stability test
#+TODO: TODO | DONE

* TODO [#A] Task :work:
SCHEDULED: <2024-01-01>
:PROPERTIES:
:ID: s1
:END:
Body text.

** Child
Child body
* Plain heading
";
        let first = write(&parse(input));
        let second = write(&parse(&first));
        assert_eq!(first, second, "second round-trip changed the output");
    }
}
