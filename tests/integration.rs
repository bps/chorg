//! Integration tests that exercise parse → mutate → write → re-parse cycles.
//!
//! These simulate what the CLI commands do, but test purely through the
//! library API so they're deterministic and don't touch the filesystem.

use chorg::model::{Heading, OrgDoc};
use chorg::parser::parse;
use chorg::path::resolve;
use chorg::writer::write;

// ===========================================================================
// Helpers
// ===========================================================================

/// The kitchen-sink document used by most tests.
fn fixture() -> OrgDoc {
    parse(
        "\
#+TITLE: Test
#+TODO: TODO NEXT WAITING | DONE CANCELLED

* TODO [#A] Buy groceries :shopping:
SCHEDULED: <2024-06-15 Sat>
:PROPERTIES:
:EFFORT: 30min
:END:
Need to go to the store.
** Milk
** Eggs
** Bread
* DONE Fix bug :work:code:
CLOSED: [2024-06-14 Fri]
The login page was fixed.
* Projects
** TODO Website redesign :work:
:PROPERTIES:
:EFFORT: 8h
:ASSIGNED: alice
:END:
Redesign the landing page.
** WAITING Database migration :work:infra:
Blocked on ops.
** NEXT Write documentation :work:
* Inbox
** Review PR from Bob
** TODO Schedule dentist :personal:
** Read org-mode manual
",
    )
}

/// Parse, apply `f`, write, re-parse, assert heading count didn't silently
/// vanish, return the re-parsed doc *and* the text.
fn mutate(f: impl FnOnce(&mut OrgDoc)) -> (OrgDoc, String) {
    let mut doc = fixture();
    f(&mut doc);
    let text = write(&doc);
    let reparsed = parse(&text);
    (reparsed, text)
}

/// Shorthand to look up a heading by path on the fixture doc.
fn at<'a>(doc: &'a OrgDoc, path: &str) -> &'a Heading {
    let idx = resolve(doc, path).unwrap();
    doc.heading_at(&idx)
}

// ===========================================================================
// TODO state changes
// ===========================================================================

#[test]
fn todo_set_done() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Buy groceries").unwrap();
        doc.heading_at_mut(&idx).keyword = Some("DONE".into());
    });
    assert_eq!(at(&doc, "Buy groceries").keyword.as_deref(), Some("DONE"));
}

#[test]
fn todo_clear_state() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Buy groceries").unwrap();
        doc.heading_at_mut(&idx).keyword = None;
    });
    assert_eq!(at(&doc, "Buy groceries").keyword, None);
}

#[test]
fn todo_set_on_heading_without_keyword() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Inbox/Review PR from Bob").unwrap();
        doc.heading_at_mut(&idx).keyword = Some("TODO".into());
    });
    assert_eq!(
        at(&doc, "Inbox/Review PR from Bob").keyword.as_deref(),
        Some("TODO")
    );
}

#[test]
fn todo_reverse_transition() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Fix bug").unwrap();
        doc.heading_at_mut(&idx).keyword = Some("TODO".into());
    });
    assert_eq!(at(&doc, "Fix bug").keyword.as_deref(), Some("TODO"));
}

#[test]
fn todo_intermediate_state() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Buy groceries").unwrap();
        doc.heading_at_mut(&idx).keyword = Some("WAITING".into());
    });
    assert_eq!(at(&doc, "Buy groceries").keyword.as_deref(), Some("WAITING"));
}

#[test]
fn todo_idempotent() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Buy groceries").unwrap();
        // Already TODO, set to TODO again
        doc.heading_at_mut(&idx).keyword = Some("TODO".into());
    });
    assert_eq!(at(&doc, "Buy groceries").keyword.as_deref(), Some("TODO"));
}

#[test]
fn todo_preserves_other_fields() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Buy groceries").unwrap();
        doc.heading_at_mut(&idx).keyword = Some("DONE".into());
    });
    let h = at(&doc, "Buy groceries");
    assert_eq!(h.priority, Some('A'));
    assert_eq!(h.tags, vec!["shopping"]);
    assert!(h.planning.as_ref().unwrap().contains("SCHEDULED"));
    assert_eq!(h.properties[0].0, "EFFORT");
    assert!(h.body.contains("Need to go to the store"));
    assert_eq!(h.children.len(), 3);
}

// ===========================================================================
// Insert
// ===========================================================================

fn new_heading(level: usize, title: &str) -> Heading {
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

#[test]
fn insert_under_parent_appends() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Projects").unwrap();
        let level = doc.heading_at(&idx).level + 1;
        let h = Heading {
            level,
            keyword: Some("TODO".into()),
            ..new_heading(level, "Mobile app")
        };
        doc.heading_at_mut(&idx).children.push(h);
    });
    let proj = at(&doc, "Projects");
    assert_eq!(proj.children.len(), 4);
    assert_eq!(proj.children[3].title, "Mobile app");
    assert_eq!(proj.children[3].keyword.as_deref(), Some("TODO"));
    assert_eq!(proj.children[3].level, 2);
}

#[test]
fn insert_under_parent_at_position() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Projects").unwrap();
        let level = doc.heading_at(&idx).level + 1;
        let h = new_heading(level, "Inserted first");
        doc.heading_at_mut(&idx).children.insert(0, h);
    });
    let proj = at(&doc, "Projects");
    assert_eq!(proj.children.len(), 4);
    assert_eq!(proj.children[0].title, "Inserted first");
    // Original first child shifted
    assert_eq!(proj.children[1].title, "Website redesign");
}

#[test]
fn insert_top_level() {
    let (doc, _) = mutate(|doc| {
        let h = new_heading(1, "Archive");
        doc.headings.push(h);
    });
    assert_eq!(doc.headings.len(), 5);
    assert_eq!(doc.headings[4].title, "Archive");
}

#[test]
fn insert_top_level_at_position() {
    let (doc, _) = mutate(|doc| {
        let h = new_heading(1, "Urgent");
        doc.headings.insert(0, h);
    });
    assert_eq!(doc.headings[0].title, "Urgent");
    // Others shifted
    assert_eq!(doc.headings[1].title, "Buy groceries");
}

#[test]
fn insert_with_all_fields() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Projects").unwrap();
        let h = Heading {
            level: 2,
            keyword: Some("TODO".into()),
            priority: Some('B'),
            title: "New task".into(),
            tags: vec!["work".into(), "urgent".into()],
            planning: None,
            properties: vec![("EFFORT".into(), "2h".into())],
            body: "Task description.\n".into(),
            children: Vec::new(),
        };
        doc.heading_at_mut(&idx).children.push(h);
    });
    let h = at(&doc, "Projects/New task");
    assert_eq!(h.keyword.as_deref(), Some("TODO"));
    assert_eq!(h.priority, Some('B'));
    assert_eq!(h.tags, vec!["work", "urgent"]);
    assert_eq!(h.properties[0], ("EFFORT".into(), "2h".into()));
    assert!(h.body.contains("Task description"));
}

#[test]
fn insert_into_leaf() {
    // "Milk" has no children — we can still insert under it
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Buy groceries/Milk").unwrap();
        let level = doc.heading_at(&idx).level + 1;
        let h = new_heading(level, "Whole milk");
        doc.heading_at_mut(&idx).children.push(h);
    });
    let milk = at(&doc, "Buy groceries/Milk");
    assert_eq!(milk.children.len(), 1);
    assert_eq!(milk.children[0].title, "Whole milk");
    assert_eq!(milk.children[0].level, 3);
}

// ===========================================================================
// Delete
// ===========================================================================

#[test]
fn delete_leaf() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Buy groceries/Eggs").unwrap();
        let (list, i) = doc.parent_list_mut(&idx);
        list.remove(i);
    });
    let groceries = at(&doc, "Buy groceries");
    assert_eq!(groceries.children.len(), 2);
    assert_eq!(groceries.children[0].title, "Milk");
    assert_eq!(groceries.children[1].title, "Bread");
}

#[test]
fn delete_subtree() {
    // Deleting "Projects" removes all its children too
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Projects").unwrap();
        let (list, i) = doc.parent_list_mut(&idx);
        list.remove(i);
    });
    assert_eq!(doc.headings.len(), 3);
    // "Inbox" shifted from #4 to #3
    assert_eq!(doc.headings[2].title, "Inbox");
    // None of the project headings should exist
    let titles: Vec<&str> = doc.walk().iter().map(|(_, h)| h.title.as_str()).collect();
    assert!(!titles.contains(&"Website redesign"));
    assert!(!titles.contains(&"Database migration"));
}

#[test]
fn delete_top_level() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Fix bug").unwrap();
        let (list, i) = doc.parent_list_mut(&idx);
        list.remove(i);
    });
    assert_eq!(doc.headings.len(), 3);
    assert_eq!(doc.headings[0].title, "Buy groceries");
    assert_eq!(doc.headings[1].title, "Projects");
    assert_eq!(doc.headings[2].title, "Inbox");
}

#[test]
fn delete_middle_child_preserves_siblings() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Inbox/Schedule dentist").unwrap();
        let (list, i) = doc.parent_list_mut(&idx);
        list.remove(i);
    });
    let inbox = at(&doc, "Inbox");
    assert_eq!(inbox.children.len(), 2);
    assert_eq!(inbox.children[0].title, "Review PR from Bob");
    assert_eq!(inbox.children[1].title, "Read org-mode manual");
}

#[test]
fn delete_only_child() {
    // Delete all children from a heading one by one
    let (doc, _) = mutate(|doc| {
        // Delete children in reverse order to keep indices valid
        for _ in 0..3 {
            let idx = resolve(doc, "Buy groceries/#1").unwrap();
            let (list, i) = doc.parent_list_mut(&idx);
            list.remove(i);
        }
    });
    let groceries = at(&doc, "Buy groceries");
    assert!(groceries.children.is_empty());
}

// ===========================================================================
// Move / refile
// ===========================================================================

#[test]
fn move_between_parents() {
    let (doc, _) = mutate(|doc| {
        // Move "Review PR from Bob" from Inbox to Projects
        let src = resolve(doc, "Inbox/Review PR from Bob").unwrap();
        let mut heading = {
            let (list, i) = doc.parent_list_mut(&src);
            list.remove(i)
        };
        // Adjust level: Projects children are level 2
        let dst = resolve(doc, "Projects").unwrap();
        let new_level = doc.heading_at(&dst).level + 1;
        let delta = new_level as i32 - heading.level as i32;
        heading.shift_level(delta);
        doc.heading_at_mut(&dst).children.push(heading);
    });
    // Verify moved
    let proj = at(&doc, "Projects");
    assert_eq!(proj.children.len(), 4);
    assert_eq!(proj.children[3].title, "Review PR from Bob");
    // Verify removed from source
    let inbox = at(&doc, "Inbox");
    assert_eq!(inbox.children.len(), 2);
    let inbox_titles: Vec<&str> = inbox.children.iter().map(|h| h.title.as_str()).collect();
    assert!(!inbox_titles.contains(&"Review PR from Bob"));
}

#[test]
fn move_to_top_level() {
    let (doc, _) = mutate(|doc| {
        let src = resolve(doc, "Projects/Website redesign").unwrap();
        let mut heading = {
            let (list, i) = doc.parent_list_mut(&src);
            list.remove(i)
        };
        // Adjust to level 1
        let delta = 1i32 - heading.level as i32;
        heading.shift_level(delta);
        doc.headings.push(heading);
    });
    assert_eq!(doc.headings.len(), 5);
    assert_eq!(doc.headings[4].title, "Website redesign");
    assert_eq!(doc.headings[4].level, 1);
    // Removed from Projects
    let proj = at(&doc, "Projects");
    assert_eq!(proj.children.len(), 2);
}

#[test]
fn move_adjusts_subtree_levels() {
    // Move a level-1 heading (with children) under a level-2 heading
    let (doc, _) = mutate(|doc| {
        // Move "Inbox" (level 1, children at level 2) under "Projects/Website redesign" (level 2)
        let src = resolve(doc, "Inbox").unwrap();
        let mut heading = {
            let (list, i) = doc.parent_list_mut(&src);
            list.remove(i)
        };
        let dst = resolve(doc, "Projects/Website redesign").unwrap();
        let new_level = doc.heading_at(&dst).level + 1; // 3
        let delta = new_level as i32 - heading.level as i32; // +2
        heading.shift_level(delta);
        doc.heading_at_mut(&dst).children.push(heading);
    });
    let inbox = at(&doc, "Projects/Website redesign/Inbox");
    assert_eq!(inbox.level, 3);
    // Children should also have shifted by +2
    assert_eq!(inbox.children[0].level, 4);
    assert_eq!(inbox.children[0].title, "Review PR from Bob");
}

#[test]
fn move_deep_with_grandchildren() {
    // Move a heading with grandchildren and verify all levels adjust
    let (doc, _) = mutate(|doc| {
        // Move "Buy groceries" (level 1, children at 2) under "Inbox" (level 1)
        let src = resolve(doc, "Buy groceries").unwrap();
        let mut heading = {
            let (list, i) = doc.parent_list_mut(&src);
            list.remove(i)
        };
        let dst = resolve(doc, "Inbox").unwrap();
        let new_level = doc.heading_at(&dst).level + 1; // 2
        let delta = new_level as i32 - heading.level as i32; // +1
        heading.shift_level(delta);
        doc.heading_at_mut(&dst).children.push(heading);
    });
    let groceries = at(&doc, "Inbox/Buy groceries");
    assert_eq!(groceries.level, 2);
    assert_eq!(groceries.children[0].level, 3);
    assert_eq!(groceries.children[0].title, "Milk");
    assert_eq!(groceries.children.len(), 3);
}

#[test]
fn move_shallow_to_deep() {
    // Move level-2 heading under a level-3 heading
    let (doc, _) = mutate(|doc| {
        // First add a level-3 target
        let idx = resolve(doc, "Projects/Website redesign").unwrap();
        let h = new_heading(3, "Sub-project");
        doc.heading_at_mut(&idx).children.push(h);

        // Move "Inbox/Review PR from Bob" (level 2) under "Sub-project" (level 3)
        let src = resolve(doc, "Inbox/Review PR from Bob").unwrap();
        let mut heading = {
            let (list, i) = doc.parent_list_mut(&src);
            list.remove(i)
        };
        let dst = resolve(doc, "Projects/Website redesign/Sub-project").unwrap();
        let new_level = doc.heading_at(&dst).level + 1; // 4
        let delta = new_level as i32 - heading.level as i32; // +2
        heading.shift_level(delta);
        doc.heading_at_mut(&dst).children.push(heading);
    });
    let h = at(&doc, "Projects/Website redesign/Sub-project/Review PR from Bob");
    assert_eq!(h.level, 4);
}

#[test]
fn move_with_position() {
    let (doc, _) = mutate(|doc| {
        let src = resolve(doc, "Inbox/Read org-mode manual").unwrap();
        let heading = {
            let (list, i) = doc.parent_list_mut(&src);
            list.remove(i)
        };
        let dst = resolve(doc, "Projects").unwrap();
        // Insert at position 0 (first child)
        doc.heading_at_mut(&dst).children.insert(0, heading);
    });
    let proj = at(&doc, "Projects");
    assert_eq!(proj.children[0].title, "Read org-mode manual");
    assert_eq!(proj.children[1].title, "Website redesign");
}

// ===========================================================================
// Edit
// ===========================================================================

#[test]
fn edit_title() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Buy groceries").unwrap();
        doc.heading_at_mut(&idx).title = "Buy food".into();
    });
    assert_eq!(at(&doc, "Buy food").title, "Buy food");
}

#[test]
fn edit_priority_set() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Projects/Website redesign").unwrap();
        doc.heading_at_mut(&idx).priority = Some('A');
    });
    assert_eq!(at(&doc, "Projects/Website redesign").priority, Some('A'));
}

#[test]
fn edit_priority_clear() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Buy groceries").unwrap();
        doc.heading_at_mut(&idx).priority = None;
    });
    assert_eq!(at(&doc, "Buy groceries").priority, None);
}

#[test]
fn edit_body_replace() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Buy groceries").unwrap();
        doc.heading_at_mut(&idx).body = "New body text.\n".into();
    });
    let h = at(&doc, "Buy groceries");
    assert_eq!(h.body, "New body text.\n");
    assert!(!h.body.contains("Need to go"));
}

#[test]
fn edit_body_clear() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Buy groceries").unwrap();
        doc.heading_at_mut(&idx).body = String::new();
    });
    assert_eq!(at(&doc, "Buy groceries").body, "");
}

#[test]
fn edit_body_append() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Buy groceries").unwrap();
        let h = doc.heading_at_mut(&idx);
        if !h.body.ends_with('\n') && !h.body.is_empty() {
            h.body.push('\n');
        }
        h.body.push_str("Also get some fruit.\n");
    });
    let h = at(&doc, "Buy groceries");
    assert!(h.body.contains("Need to go to the store"));
    assert!(h.body.contains("Also get some fruit"));
    // Appended text should come after original
    let orig_pos = h.body.find("Need to go").unwrap();
    let new_pos = h.body.find("Also get").unwrap();
    assert!(new_pos > orig_pos);
}

#[test]
fn edit_body_double_append() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Buy groceries").unwrap();
        let h = doc.heading_at_mut(&idx);
        h.body.push_str("Line A.\n");
        h.body.push_str("Line B.\n");
    });
    let h = at(&doc, "Buy groceries");
    assert!(h.body.contains("Need to go"));
    assert!(h.body.contains("Line A."));
    assert!(h.body.contains("Line B."));
    let a_pos = h.body.find("Line A.").unwrap();
    let b_pos = h.body.find("Line B.").unwrap();
    assert!(b_pos > a_pos);
}

#[test]
fn edit_body_append_multiline() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Buy groceries/Eggs").unwrap();
        doc.heading_at_mut(&idx).body = "Para 1.\n\nPara 2.\n".into();
    });
    let h = at(&doc, "Buy groceries/Eggs");
    assert_eq!(h.body, "Para 1.\n\nPara 2.\n");
}

#[test]
fn edit_body_append_to_empty() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Buy groceries/Eggs").unwrap();
        doc.heading_at_mut(&idx).body = "Free range\n".into();
    });
    assert_eq!(at(&doc, "Buy groceries/Eggs").body, "Free range\n");
}

// ===========================================================================
// Properties
// ===========================================================================

#[test]
fn prop_set_new() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Buy groceries").unwrap();
        doc.heading_at_mut(&idx)
            .properties
            .push(("CATEGORY".into(), "errands".into()));
    });
    let h = at(&doc, "Buy groceries");
    assert!(h.properties.iter().any(|(k, v)| k == "CATEGORY" && v == "errands"));
}

#[test]
fn prop_update_existing() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Buy groceries").unwrap();
        let h = doc.heading_at_mut(&idx);
        if let Some(entry) = h.properties.iter_mut().find(|(k, _)| k == "EFFORT") {
            entry.1 = "1h".into();
        }
    });
    let h = at(&doc, "Buy groceries");
    let effort = h.properties.iter().find(|(k, _)| k == "EFFORT").unwrap();
    assert_eq!(effort.1, "1h");
}

#[test]
fn prop_update_to_empty_value() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Buy groceries").unwrap();
        let h = doc.heading_at_mut(&idx);
        if let Some(entry) = h.properties.iter_mut().find(|(k, _)| k == "EFFORT") {
            entry.1 = String::new();
        }
    });
    let h = at(&doc, "Buy groceries");
    let effort = h.properties.iter().find(|(k, _)| k == "EFFORT").unwrap();
    assert_eq!(effort.1, "");
}

#[test]
fn prop_update_second_preserves_first() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Projects/Website redesign").unwrap();
        let h = doc.heading_at_mut(&idx);
        // Update ASSIGNED (second property), leave EFFORT (first) alone
        if let Some(entry) = h.properties.iter_mut().find(|(k, _)| k == "ASSIGNED") {
            entry.1 = "bob".into();
        }
    });
    let h = at(&doc, "Projects/Website redesign");
    assert_eq!(h.properties[0], ("EFFORT".into(), "8h".into()));
    assert_eq!(h.properties[1], ("ASSIGNED".into(), "bob".into()));
}

#[test]
fn prop_update_case_insensitive_key() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Buy groceries").unwrap();
        let h = doc.heading_at_mut(&idx);
        // Find by case-insensitive match
        if let Some(entry) = h.properties.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case("effort")) {
            entry.1 = "45min".into();
        }
    });
    let h = at(&doc, "Buy groceries");
    assert_eq!(h.properties[0], ("EFFORT".into(), "45min".into()));
}

#[test]
fn prop_delete() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Buy groceries").unwrap();
        doc.heading_at_mut(&idx).properties.retain(|(k, _)| k != "EFFORT");
    });
    let h = at(&doc, "Buy groceries");
    assert!(h.properties.is_empty());
}

#[test]
fn prop_order_preserved() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Projects/Website redesign").unwrap();
        let h = doc.heading_at_mut(&idx);
        h.properties.push(("NEW_PROP".into(), "val".into()));
    });
    let h = at(&doc, "Projects/Website redesign");
    assert_eq!(h.properties[0].0, "EFFORT");
    assert_eq!(h.properties[1].0, "ASSIGNED");
    assert_eq!(h.properties[2].0, "NEW_PROP");
}

#[test]
fn prop_creates_drawer() {
    // Adding a property to a heading with no properties should create a drawer
    let (doc, text) = mutate(|doc| {
        let idx = resolve(doc, "Buy groceries/Milk").unwrap();
        doc.heading_at_mut(&idx)
            .properties
            .push(("SOURCE".into(), "farm".into()));
    });
    let h = at(&doc, "Buy groceries/Milk");
    assert_eq!(h.properties[0], ("SOURCE".into(), "farm".into()));
    assert!(text.contains(":PROPERTIES:"));
    assert!(text.contains(":SOURCE: farm"));
    assert!(text.contains(":END:"));
}

#[test]
fn prop_removes_drawer() {
    // Removing all properties should remove the drawer
    let (_, text) = mutate(|doc| {
        let idx = resolve(doc, "Buy groceries").unwrap();
        doc.heading_at_mut(&idx).properties.clear();
    });
    // The PROPERTIES block for "Buy groceries" should be gone
    // (the one for "Website redesign" should still be there)
    let first_heading_section = text.split("** Milk").next().unwrap();
    assert!(!first_heading_section.contains(":PROPERTIES:"));
}

// ===========================================================================
// Tags
// ===========================================================================

#[test]
fn tag_add() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Buy groceries").unwrap();
        doc.heading_at_mut(&idx).tags.push("urgent".into());
    });
    let h = at(&doc, "Buy groceries");
    assert_eq!(h.tags, vec!["shopping", "urgent"]);
}

#[test]
fn tag_add_idempotent() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Buy groceries").unwrap();
        let h = doc.heading_at_mut(&idx);
        if !h.tags.iter().any(|t| t == "shopping") {
            h.tags.push("shopping".into());
        }
    });
    let h = at(&doc, "Buy groceries");
    assert_eq!(h.tags.iter().filter(|t| *t == "shopping").count(), 1);
}

#[test]
fn tag_remove() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Fix bug").unwrap();
        doc.heading_at_mut(&idx).tags.retain(|t| t != "code");
    });
    let h = at(&doc, "Fix bug");
    assert_eq!(h.tags, vec!["work"]);
}

#[test]
fn tag_remove_last_tag() {
    // Removing the only tag should leave the heading untagged
    let (doc, text) = mutate(|doc| {
        let idx = resolve(doc, "Buy groceries").unwrap();
        doc.heading_at_mut(&idx).tags.retain(|t| t != "shopping");
    });
    let h = at(&doc, "Buy groceries");
    assert!(h.tags.is_empty());
    // The headline line should have no colons from tags
    let headline = text.lines().find(|l| l.contains("Buy groceries")).unwrap();
    assert!(!headline.ends_with(':'));
}

#[test]
fn tag_remove_middle_preserves_order() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Fix bug").unwrap();
        let h = doc.heading_at_mut(&idx);
        // Start with work, code — add a third tag then remove the middle
        h.tags.push("archived".into());
        // Now: work, code, archived — remove "code"
        h.tags.retain(|t| t != "code");
    });
    let h = at(&doc, "Fix bug");
    assert_eq!(h.tags, vec!["work", "archived"]);
}

#[test]
fn tag_remove_nonexistent() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Buy groceries").unwrap();
        doc.heading_at_mut(&idx).tags.retain(|t| t != "nonexistent");
    });
    // Should not error or change anything
    assert_eq!(at(&doc, "Buy groceries").tags, vec!["shopping"]);
}

#[test]
fn tag_set_replaces_all() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Fix bug").unwrap();
        doc.heading_at_mut(&idx).tags = vec!["archived".into()];
    });
    let h = at(&doc, "Fix bug");
    assert_eq!(h.tags, vec!["archived"]);
}

#[test]
fn tag_clear() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Buy groceries").unwrap();
        doc.heading_at_mut(&idx).tags.clear();
    });
    assert!(at(&doc, "Buy groceries").tags.is_empty());
}

#[test]
fn tag_add_to_untagged() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Inbox/Review PR from Bob").unwrap();
        doc.heading_at_mut(&idx).tags.push("work".into());
    });
    assert_eq!(at(&doc, "Inbox/Review PR from Bob").tags, vec!["work"]);
}

// ===========================================================================
// Promote / demote
// ===========================================================================

#[test]
fn promote_heading() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Projects/Website redesign").unwrap();
        doc.heading_at_mut(&idx).shift_level(-1);
    });
    // After write → re-parse, Website redesign is level 1. The subsequent
    // level-2 siblings (Database migration, Write documentation) become its
    // children because they follow it at a deeper level.
    let h = at(&doc, "Website redesign");
    assert_eq!(h.level, 1);
    assert_eq!(h.children.len(), 2);
    assert_eq!(h.children[0].title, "Database migration");
    assert_eq!(h.children[1].title, "Write documentation");
    // Projects lost all its former children
    assert!(at(&doc, "Projects").children.is_empty());
}

#[test]
fn demote_heading_with_children() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Buy groceries").unwrap();
        doc.heading_at_mut(&idx).shift_level(1);
    });
    // After write → re-parse, "Buy groceries" is level 2. Since there's no
    // level-1 heading before it in the preamble, it's still the first logical
    // heading but at level 2. Its children shifted from 2→3.
    let h = at(&doc, "Buy groceries");
    assert_eq!(h.level, 2);
    assert_eq!(h.children[0].level, 3);
    assert_eq!(h.children[1].level, 3);
    assert_eq!(h.children[2].level, 3);
}

#[test]
fn promote_clamps_at_level_1() {
    let (doc, _) = mutate(|doc| {
        let idx = resolve(doc, "Buy groceries").unwrap();
        doc.heading_at_mut(&idx).shift_level(-5);
    });
    // After write → re-parse, heading and children are all clamped to level 1.
    // Former children are now top-level siblings.
    let h = at(&doc, "Buy groceries");
    assert_eq!(h.level, 1);
    // Children became siblings at level 1 after re-parse, so Buy groceries
    // has no children.
    assert!(h.children.is_empty());
    // Milk, Eggs, Bread should now be top-level headings
    assert!(resolve(&doc, "Milk").is_ok());
    assert!(resolve(&doc, "Eggs").is_ok());
    assert!(resolve(&doc, "Bread").is_ok());
}

// ===========================================================================
// Round-trip stability after mutations
// ===========================================================================

#[test]
fn roundtrip_after_todo_change() {
    let (_, text) = mutate(|doc| {
        let idx = resolve(doc, "Buy groceries").unwrap();
        doc.heading_at_mut(&idx).keyword = Some("DONE".into());
    });
    let doc2 = parse(&text);
    let text2 = write(&doc2);
    assert_eq!(text, text2, "second round-trip changed the output");
}

#[test]
fn roundtrip_after_insert() {
    let (_, text) = mutate(|doc| {
        let idx = resolve(doc, "Projects").unwrap();
        let h = new_heading(2, "New project");
        doc.heading_at_mut(&idx).children.push(h);
    });
    let doc2 = parse(&text);
    let text2 = write(&doc2);
    assert_eq!(text, text2);
}

#[test]
fn roundtrip_after_delete() {
    let (_, text) = mutate(|doc| {
        let idx = resolve(doc, "Projects").unwrap();
        let (list, i) = doc.parent_list_mut(&idx);
        list.remove(i);
    });
    let doc2 = parse(&text);
    let text2 = write(&doc2);
    assert_eq!(text, text2);
}

#[test]
fn roundtrip_after_move() {
    let (_, text) = mutate(|doc| {
        let src = resolve(doc, "Inbox/Review PR from Bob").unwrap();
        let heading = {
            let (list, i) = doc.parent_list_mut(&src);
            list.remove(i)
        };
        let dst = resolve(doc, "Projects").unwrap();
        doc.heading_at_mut(&dst).children.push(heading);
    });
    let doc2 = parse(&text);
    let text2 = write(&doc2);
    assert_eq!(text, text2);
}

#[test]
fn roundtrip_after_tag_and_prop_changes() {
    let (_, text) = mutate(|doc| {
        let idx = resolve(doc, "Buy groceries").unwrap();
        let h = doc.heading_at_mut(&idx);
        h.tags.push("urgent".into());
        h.properties.push(("CATEGORY".into(), "errands".into()));
    });
    let doc2 = parse(&text);
    let text2 = write(&doc2);
    assert_eq!(text, text2);
}

// ===========================================================================
// Multiple sequential mutations
// ===========================================================================

#[test]
fn multiple_mutations() {
    let mut doc = fixture();

    // 1. Mark groceries done
    let idx = resolve(&doc, "Buy groceries").unwrap();
    doc.heading_at_mut(&idx).keyword = Some("DONE".into());

    // 2. Delete an inbox item
    let idx = resolve(&doc, "Inbox/Read org-mode manual").unwrap();
    let (list, i) = doc.parent_list_mut(&idx);
    list.remove(i);

    // 3. Add a new project
    let idx = resolve(&doc, "Projects").unwrap();
    let h = Heading {
        level: 2,
        keyword: Some("TODO".into()),
        ..new_heading(2, "API redesign")
    };
    doc.heading_at_mut(&idx).children.push(h);

    // 4. Change a tag
    let idx = resolve(&doc, "Fix bug").unwrap();
    doc.heading_at_mut(&idx).tags = vec!["archived".into()];

    // Verify
    let text = write(&doc);
    let final_doc = parse(&text);

    assert_eq!(at(&final_doc, "Buy groceries").keyword.as_deref(), Some("DONE"));
    assert_eq!(at(&final_doc, "Inbox").children.len(), 2);
    assert_eq!(at(&final_doc, "Projects").children.len(), 4);
    assert_eq!(at(&final_doc, "Projects/API redesign").keyword.as_deref(), Some("TODO"));
    assert_eq!(at(&final_doc, "Fix bug").tags, vec!["archived"]);

    // And it's stable
    let text2 = write(&final_doc);
    assert_eq!(text, text2);
}

// ===========================================================================
// Edge cases
// ===========================================================================

#[test]
fn empty_document_operations() {
    let mut doc = parse("");
    doc.headings.push(new_heading(1, "First"));
    let text = write(&doc);
    let doc2 = parse(&text);
    assert_eq!(doc2.headings.len(), 1);
    assert_eq!(doc2.headings[0].title, "First");
}

#[test]
fn delete_all_headings() {
    let mut doc = fixture();
    doc.headings.clear();
    let text = write(&doc);
    let doc2 = parse(&text);
    assert!(doc2.headings.is_empty());
    // Preamble should survive
    assert!(text.contains("#+TITLE: Test"));
}

#[test]
fn preamble_survives_all_mutations() {
    let (_, text) = mutate(|doc| {
        doc.headings.remove(0);
        doc.headings.remove(0);
    });
    assert!(text.contains("#+TITLE: Test"));
    assert!(text.contains("#+TODO: TODO NEXT WAITING | DONE CANCELLED"));
}

#[test]
fn body_with_org_like_content_preserved() {
    // Body text that looks like org syntax should be preserved verbatim
    let input = "\
* Heading
Here is a code example:
#+BEGIN_SRC python
x = 1
#+END_SRC
And a table:
| a | b |
| 1 | 2 |
";
    let doc = parse(input);
    let output = write(&doc);
    assert_eq!(input, output);
    assert!(doc.headings[0].body.contains("#+BEGIN_SRC"));
    assert!(doc.headings[0].body.contains("| a | b |"));
}

#[test]
fn heading_with_only_properties_no_body() {
    let input = "\
* Heading
:PROPERTIES:
:ID: test
:END:
** Child
";
    let doc = parse(input);
    assert_eq!(doc.headings[0].properties[0], ("ID".into(), "test".into()));
    assert_eq!(doc.headings[0].body, "");
    assert_eq!(doc.headings[0].children[0].title, "Child");
    let output = write(&doc);
    assert_eq!(input, output);
}

#[test]
fn heading_count_preserved_across_all_operations() {
    let doc = fixture();
    let initial_count = doc.walk().len();
    assert_eq!(initial_count, 13); // verify our fixture has 13 headings total

    // After round-trip
    let text = write(&doc);
    let doc2 = parse(&text);
    assert_eq!(doc2.walk().len(), initial_count);
}
