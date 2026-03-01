//! End-to-end tests that invoke the `chorg` binary via `std::process::Command`.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Create a temporary org file with the given content; returns its path.
/// Each call gets a unique filename so tests can run in parallel.
fn tmp_org(content: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("cli_test_{}.org", n));
    fs::write(&path, content).unwrap();
    path
}

fn chorg() -> Command {
    Command::new(env!("CARGO_BIN_EXE_chorg"))
}

/// Run chorg with args, assert success, return stdout.
fn run(args: &[&str]) -> String {
    let output = chorg()
        .args(args)
        .output()
        .expect("failed to execute chorg");
    assert!(
        output.status.success(),
        "chorg {:?} failed.\nstdout: {}\nstderr: {}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

/// Run chorg expecting failure, return stderr.
fn run_err(args: &[&str]) -> String {
    let output = chorg()
        .args(args)
        .output()
        .expect("failed to execute chorg");
    assert!(
        !output.status.success(),
        "expected failure but chorg {:?} succeeded.\nstdout: {}",
        args,
        String::from_utf8_lossy(&output.stdout)
    );
    String::from_utf8(output.stderr).unwrap()
}

const FIXTURE: &str = "\
#+TITLE: CLI Test
#+TODO: TODO NEXT | DONE

* TODO [#A] Task one :work:
:PROPERTIES:
:EFFORT: 2h
:END:
Body of task one.
** Subtask A
** Subtask B
* DONE Task two :personal:
* Projects
** TODO Project Alpha
** NEXT Project Beta :code:
* Inbox
** Review docs
** TODO Call plumber
";

// ===========================================================================
// show
// ===========================================================================

#[test]
fn cli_show_basic() {
    let f = tmp_org(FIXTURE);
    let out = run(&["show", f.to_str().unwrap()]);
    assert!(out.contains("#1"));
    assert!(out.contains("Task one"));
    assert!(out.contains("#3.2"));
    assert!(out.contains("Project Beta"));
    // Should show all headings
    assert!(out.contains("Inbox"));
    assert!(out.contains("Call plumber"));
}

#[test]
fn cli_show_with_path() {
    let f = tmp_org(FIXTURE);
    let out = run(&["show", f.to_str().unwrap(), "-p", "Projects"]);
    assert!(out.contains("Projects"));
    assert!(out.contains("Project Alpha"));
    assert!(out.contains("Project Beta"));
    // Should NOT show unrelated headings
    assert!(!out.contains("Task one"));
    assert!(!out.contains("Inbox"));
}

#[test]
fn cli_show_with_depth() {
    let f = tmp_org(FIXTURE);
    let out = run(&["show", f.to_str().unwrap(), "-d", "0"]);
    // Depth 0 = only top-level headings, no children
    assert!(out.contains("Task one"));
    assert!(!out.contains("Subtask A"));
    assert!(!out.contains("Project Alpha"));
}

#[test]
fn cli_show_with_depth_1() {
    let f = tmp_org(FIXTURE);
    let out = run(&["show", f.to_str().unwrap(), "-d", "1"]);
    // Depth 1 = top-level headings + their direct children
    assert!(out.contains("Task one"));
    assert!(out.contains("Subtask A"));
    assert!(out.contains("Project Alpha"));
}

#[test]
fn cli_show_depth_with_path() {
    let f = tmp_org(FIXTURE);
    let out = run(&["show", f.to_str().unwrap(), "-p", "Task one", "-d", "0"]);
    // Depth 0 relative to the target — show target only, no children
    assert!(out.contains("Task one"));
    assert!(!out.contains("Subtask A"));
    assert!(!out.contains("Subtask B"));
}

#[test]
fn cli_show_with_content() {
    let f = tmp_org(FIXTURE);
    let out = run(&["show", f.to_str().unwrap(), "--content", "-p", "Task one"]);
    assert!(out.contains("Body of task one"));
    assert!(out.contains("|")); // content marker
}

#[test]
fn cli_show_json() {
    let f = tmp_org(FIXTURE);
    let out = run(&["show", f.to_str().unwrap(), "--json"]);
    let json: serde_json::Value = serde_json::from_str(&out).expect("invalid JSON");
    assert!(json["headings"].is_array());
    let headings = json["headings"].as_array().unwrap();
    assert_eq!(headings.len(), 4);
    assert_eq!(headings[0]["title"], "Task one");
    assert_eq!(headings[0]["keyword"], "TODO");
    assert_eq!(headings[0]["priority"], "A");
    assert_eq!(headings[0]["addr"], "#1");
}

#[test]
fn cli_show_json_subtree() {
    let f = tmp_org(FIXTURE);
    let out = run(&["show", f.to_str().unwrap(), "--json", "-p", "Projects"]);
    let json: serde_json::Value = serde_json::from_str(&out).expect("invalid JSON");
    let headings = json["headings"].as_array().unwrap();
    assert_eq!(headings.len(), 1);
    assert_eq!(headings[0]["title"], "Projects");
    let children = headings[0]["children"].as_array().unwrap();
    assert_eq!(children.len(), 2);
}

// ===========================================================================
// find
// ===========================================================================

#[test]
fn cli_find_by_keyword() {
    let f = tmp_org(FIXTURE);
    let out = run(&["find", f.to_str().unwrap(), "--keyword", "TODO"]);
    assert!(out.contains("Task one"));
    assert!(out.contains("Project Alpha"));
    assert!(out.contains("Call plumber"));
    assert!(!out.contains("Task two")); // DONE
    assert!(!out.contains("Project Beta")); // NEXT
}

#[test]
fn cli_find_by_tag() {
    let f = tmp_org(FIXTURE);
    let out = run(&["find", f.to_str().unwrap(), "--tag", "work"]);
    assert!(out.contains("Task one"));
    assert!(!out.contains("Task two")); // personal, not work
}

#[test]
fn cli_find_by_tag_case_insensitive() {
    let f = tmp_org(FIXTURE);
    let out = run(&["find", f.to_str().unwrap(), "--tag", "WORK"]);
    assert!(out.contains("Task one"));
}

#[test]
fn cli_find_by_tag_multi_tag_heading() {
    // Project Beta has :code: — only one tag, but let's test a heading with multiple tags
    let multi = "* TODO Task :alpha:beta:gamma:\n";
    let f = tmp_org(multi);
    let out = run(&["find", f.to_str().unwrap(), "--tag", "beta"]);
    assert!(out.contains("Task"));
}

#[test]
fn cli_find_by_multiple_tags() {
    let fixture = "\
* A :work:urgent:
* B :work:
* C :urgent:
* D :work:urgent:code:
";
    let f = tmp_org(fixture);
    // Require BOTH tags — only A and D have both work AND urgent
    let out = run(&["find", f.to_str().unwrap(), "--tag", "work", "--tag", "urgent"]);
    assert!(out.contains("A"));
    assert!(!out.contains("#2")); // B has work but not urgent
    assert!(!out.contains("#3")); // C has urgent but not work
    assert!(out.contains("D"));
}

#[test]
fn cli_find_by_three_tags() {
    let fixture = "\
* A :work:urgent:code:
* B :work:urgent:
";
    let f = tmp_org(fixture);
    let out = run(&["find", f.to_str().unwrap(), "--tag", "work", "--tag", "urgent", "--tag", "code"]);
    assert!(out.contains("A"));
    assert!(!out.contains("B"));
}

#[test]
fn cli_find_by_tag_no_match() {
    let f = tmp_org(FIXTURE);
    let out = run(&["find", f.to_str().unwrap(), "--tag", "nonexistent"]);
    // No matches — stdout should be empty (message goes to stderr)
    assert!(out.is_empty() || !out.contains("*"));
}

#[test]
fn cli_find_by_title() {
    let f = tmp_org(FIXTURE);
    let out = run(&["find", f.to_str().unwrap(), "--title", "Project"]);
    assert!(out.contains("Projects"));
    assert!(out.contains("Project Alpha"));
    assert!(out.contains("Project Beta"));
    assert!(!out.contains("Task one"));
}

#[test]
fn cli_find_by_property() {
    let f = tmp_org(FIXTURE);
    let out = run(&["find", f.to_str().unwrap(), "--property", "EFFORT"]);
    assert!(out.contains("Task one"));
    assert!(!out.contains("Task two"));
}

#[test]
fn cli_find_by_body() {
    let f = tmp_org(FIXTURE);
    let out = run(&["find", f.to_str().unwrap(), "--body", "task one"]);
    assert!(out.contains("Task one"));
    assert!(!out.contains("Task two"));
    assert!(!out.contains("Projects"));
}

#[test]
fn cli_find_by_body_case_insensitive() {
    let f = tmp_org(FIXTURE);
    let out = run(&["find", f.to_str().unwrap(), "--body", "BODY OF TASK"]);
    assert!(out.contains("Task one"));
}

#[test]
fn cli_find_by_body_no_match() {
    let f = tmp_org(FIXTURE);
    let out = run(&["find", f.to_str().unwrap(), "--body", "xyzzy"]);
    assert!(out.is_empty() || !out.contains("#"));
}

#[test]
fn cli_find_body_combined_with_keyword() {
    // Body AND keyword must both match
    let fixture = "\
* TODO A
Has the magic word
* DONE B
Has the magic word
* TODO C
No magic here
";
    let f = tmp_org(fixture);
    let out = run(&["find", f.to_str().unwrap(), "--body", "magic word", "--keyword", "TODO"]);
    assert!(out.contains("A"));
    assert!(!out.contains("B")); // DONE, not TODO
    assert!(!out.contains("C")); // no "magic word" in body
}

#[test]
fn cli_find_scoped_under() {
    let f = tmp_org(FIXTURE);
    // Only search under "Projects"
    let out = run(&["find", f.to_str().unwrap(), "--under", "Projects", "--keyword", "TODO"]);
    assert!(out.contains("Project Alpha"));
    // Task one is TODO but not under Projects
    assert!(!out.contains("Task one"));
    // Call plumber is TODO but under Inbox, not Projects
    assert!(!out.contains("Call plumber"));
}

#[test]
fn cli_find_scoped_under_positional() {
    let f = tmp_org(FIXTURE);
    let out = run(&["find", f.to_str().unwrap(), "--under", "#4"]);
    // #4 is Inbox — should return its children
    assert!(out.contains("Review docs"));
    assert!(out.contains("Call plumber"));
    // Should NOT include the parent itself
    assert!(!out.contains("#4 "));
    // Should NOT include headings outside Inbox
    assert!(!out.contains("Task one"));
    assert!(!out.contains("Project"));
}

#[test]
fn cli_find_scoped_under_with_body() {
    let fixture = "\
* Projects
** Alpha
Alpha body text
** Beta
Beta body text
* Notes
** Gamma
Gamma body text
";
    let f = tmp_org(fixture);
    let out = run(&["find", f.to_str().unwrap(), "--under", "Projects", "--body", "body text"]);
    assert!(out.contains("Alpha"));
    assert!(out.contains("Beta"));
    // Gamma has "body text" too, but it's under Notes, not Projects
    assert!(!out.contains("Gamma"));
}

#[test]
fn cli_find_scoped_under_deep() {
    let fixture = "\
* A
** B
*** TODO C1
*** DONE C2
** TODO D
* E
** TODO F
";
    let f = tmp_org(fixture);
    // Search only under A/B
    let out = run(&["find", f.to_str().unwrap(), "--under", "A/B", "--keyword", "TODO"]);
    assert!(out.contains("C1"));
    assert!(!out.contains("C2")); // DONE
    assert!(!out.contains("** TODO D")); // sibling of B, not under B
    assert!(!out.contains("F"));  // under E
}

#[test]
fn cli_find_scoped_under_no_match() {
    let f = tmp_org(FIXTURE);
    let out = run(&["find", f.to_str().unwrap(), "--under", "Projects", "--keyword", "DONE"]);
    // No DONE headings under Projects
    assert!(out.is_empty() || !out.contains("#"));
}

// ===========================================================================
// find: negation
// ===========================================================================

#[test]
fn cli_find_no_keyword() {
    let f = tmp_org(FIXTURE);
    // Exclude DONE headings
    let out = run(&["find", f.to_str().unwrap(), "--no-keyword", "DONE"]);
    assert!(out.contains("Task one"));       // TODO — not excluded
    assert!(!out.contains("Task two"));      // DONE — excluded
    assert!(out.contains("Project Alpha"));  // TODO
    assert!(out.contains("Project Beta"));   // NEXT
    assert!(out.contains("Review docs"));    // no keyword
}

#[test]
fn cli_find_no_keyword_combined() {
    let f = tmp_org(FIXTURE);
    // TODO headings that are NOT DONE (redundant here, but tests the AND)
    let out = run(&["find", f.to_str().unwrap(), "--keyword", "TODO", "--no-keyword", "DONE"]);
    assert!(out.contains("Task one"));
    assert!(!out.contains("Task two"));
}

#[test]
fn cli_find_no_tag() {
    let f = tmp_org(FIXTURE);
    // Exclude headings tagged :personal:
    let out = run(&["find", f.to_str().unwrap(), "--no-tag", "personal"]);
    assert!(out.contains("Task one"));
    assert!(out.contains("Project Alpha"));
    assert!(!out.contains("Task two")); // has :personal:
    assert!(out.contains("Call plumber")); // no tags at all — not excluded
}

#[test]
fn cli_find_no_tag_combined_with_tag() {
    let fixture = "\
* A :work:urgent:
* B :work:
* C :urgent:
* D :work:urgent:old:
";
    let f = tmp_org(fixture);
    // Has :work: but NOT :old:
    let out = run(&["find", f.to_str().unwrap(), "--tag", "work", "--no-tag", "old"]);
    assert!(out.contains("A"));
    assert!(out.contains("B"));
    assert!(!out.contains("C")); // no :work:
    assert!(!out.contains("#4")); // D has :old:
}

// ===========================================================================
// find: level filter
// ===========================================================================

#[test]
fn cli_find_min_level() {
    let f = tmp_org(FIXTURE);
    // Only level 2+ headings (children)
    let out = run(&["find", f.to_str().unwrap(), "--min-level", "2"]);
    assert!(out.contains("Subtask A"));
    assert!(out.contains("Project Alpha"));
    assert!(!out.contains("Task one"));   // level 1
    assert!(!out.contains("Projects"));   // level 1
}

#[test]
fn cli_find_max_level() {
    let f = tmp_org(FIXTURE);
    // Only level 1 headings (top-level)
    let out = run(&["find", f.to_str().unwrap(), "--max-level", "1"]);
    assert!(out.contains("Task one"));
    assert!(out.contains("Projects"));
    assert!(!out.contains("Subtask A"));     // level 2
    assert!(!out.contains("Project Alpha")); // level 2
}

#[test]
fn cli_find_min_and_max_level() {
    let fixture = "\
* L1
** L2a
*** L3
** L2b
";
    let f = tmp_org(fixture);
    let out = run(&["find", f.to_str().unwrap(), "--min-level", "2", "--max-level", "2"]);
    assert!(out.contains("L2a"));
    assert!(out.contains("L2b"));
    assert!(!out.contains("L1"));
    assert!(!out.contains("L3"));
}

#[test]
fn cli_find_level_combined_with_keyword() {
    let f = tmp_org(FIXTURE);
    // Top-level TODO headings only
    let out = run(&["find", f.to_str().unwrap(), "--keyword", "TODO", "--max-level", "1"]);
    assert!(out.contains("Task one"));
    assert!(!out.contains("Project Alpha")); // TODO but level 2
    assert!(!out.contains("Call plumber"));  // TODO but level 2
}

// ===========================================================================
// find: planning presence
// ===========================================================================

#[test]
fn cli_find_scheduled() {
    let fixture = "\
* TODO A
SCHEDULED: <2024-06-15 Sat>
Task A body
* TODO B
Task B body
* TODO C
DEADLINE: <2024-07-01 Mon>
Task C body
";
    let f = tmp_org(fixture);
    let out = run(&["find", f.to_str().unwrap(), "--scheduled"]);
    assert!(out.contains("A"));
    assert!(!out.contains("B")); // no planning at all
    assert!(!out.contains("C")); // has DEADLINE not SCHEDULED
}

#[test]
fn cli_find_deadline() {
    let fixture = "\
* TODO A
SCHEDULED: <2024-06-15 Sat>
* TODO B
DEADLINE: <2024-07-01 Mon>
* C
";
    let f = tmp_org(fixture);
    let out = run(&["find", f.to_str().unwrap(), "--deadline"]);
    assert!(!out.contains("#1")); // A has SCHEDULED not DEADLINE
    assert!(out.contains("B"));
    assert!(!out.contains("#3")); // C has no planning
}

#[test]
fn cli_find_scheduled_and_deadline_on_same_heading() {
    let fixture = "\
* TODO A
SCHEDULED: <2024-06-15> DEADLINE: <2024-07-01>
* TODO B
SCHEDULED: <2024-06-15>
";
    let f = tmp_org(fixture);
    // --deadline: only A has DEADLINE
    let out = run(&["find", f.to_str().unwrap(), "--deadline"]);
    assert!(out.contains("A"));
    assert!(!out.contains("B"));
    // --scheduled: both have SCHEDULED
    let out = run(&["find", f.to_str().unwrap(), "--scheduled"]);
    assert!(out.contains("A"));
    assert!(out.contains("B"));
}

#[test]
fn cli_find_scheduled_combined_with_keyword() {
    let fixture = "\
* TODO A
SCHEDULED: <2024-06-15>
* DONE B
SCHEDULED: <2024-06-10>
* TODO C
";
    let f = tmp_org(fixture);
    let out = run(&["find", f.to_str().unwrap(), "--scheduled", "--keyword", "TODO"]);
    assert!(out.contains("A"));
    assert!(!out.contains("B")); // DONE
    assert!(!out.contains("C")); // no SCHEDULED
}

#[test]
fn cli_find_combined_filters() {
    let f = tmp_org(FIXTURE);
    let out = run(&[
        "find",
        f.to_str().unwrap(),
        "--keyword",
        "TODO",
        "--tag",
        "work",
    ]);
    // Only headings matching BOTH filters
    assert!(out.contains("Task one"));
    assert!(!out.contains("Call plumber")); // TODO but no work tag
}

#[test]
fn cli_find_json() {
    let f = tmp_org(FIXTURE);
    let out = run(&["find", f.to_str().unwrap(), "--keyword", "DONE", "--json"]);
    let json: serde_json::Value = serde_json::from_str(&out).expect("invalid JSON");
    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["title"], "Task two");
}

// ===========================================================================
// todo
// ===========================================================================

#[test]
fn cli_todo_stdout() {
    let f = tmp_org(FIXTURE);
    let out = run(&["todo", f.to_str().unwrap(), "-p", "Task one", "DONE"]);
    assert!(out.contains("* DONE"));
    assert!(out.contains("Task one"));
    // Original file unchanged
    let original = fs::read_to_string(&f).unwrap();
    assert!(original.contains("* TODO"));
}

#[test]
fn cli_todo_in_place() {
    let f = tmp_org(FIXTURE);
    run(&["todo", f.to_str().unwrap(), "-p", "Task one", "DONE", "-i"]);
    let content = fs::read_to_string(&f).unwrap();
    assert!(content.contains("* DONE [#A] Task one"));
    assert!(!content.contains("* TODO [#A] Task one"));
}

#[test]
fn cli_todo_clear() {
    let f = tmp_org(FIXTURE);
    // Empty string clears the keyword
    let out = run(&["todo", f.to_str().unwrap(), "-p", "Task one", ""]);
    // Should have `* [#A] Task one` (no keyword)
    assert!(out.contains("* [#A] Task one"));
}

// ===========================================================================
// insert
// ===========================================================================

#[test]
fn cli_insert_basic() {
    let f = tmp_org(FIXTURE);
    let out = run(&[
        "insert",
        f.to_str().unwrap(),
        "-p",
        "Projects",
        "--title",
        "New project",
        "--keyword",
        "TODO",
    ]);
    assert!(out.contains("** TODO New project"));
    // Default is append — should appear after existing children
    let alpha_pos = out.find("Project Alpha").unwrap();
    let beta_pos = out.find("Project Beta").unwrap();
    let new_pos = out.find("New project").unwrap();
    assert!(new_pos > alpha_pos, "inserted heading should come after existing children");
    assert!(new_pos > beta_pos, "inserted heading should come after existing children");
}

#[test]
fn cli_insert_top_level() {
    let f = tmp_org(FIXTURE);
    let out = run(&[
        "insert",
        f.to_str().unwrap(),
        "-p",
        "/",
        "--title",
        "Archive",
    ]);
    assert!(out.contains("* Archive"));
}

#[test]
fn cli_insert_with_tags() {
    let f = tmp_org(FIXTURE);
    let out = run(&[
        "insert",
        f.to_str().unwrap(),
        "-p",
        "Projects",
        "--title",
        "Tagged",
        "--tags",
        "work:urgent",
    ]);
    assert!(out.contains("** Tagged :work:urgent:"));
}

#[test]
fn cli_insert_with_position() {
    let f = tmp_org(FIXTURE);
    let out = run(&[
        "insert",
        f.to_str().unwrap(),
        "-p",
        "Projects",
        "--title",
        "First child",
        "-n",
        "1",
    ]);
    // Should appear before "Project Alpha"
    let first_pos = out.find("First child").unwrap();
    let alpha_pos = out.find("Project Alpha").unwrap();
    assert!(first_pos < alpha_pos, "inserted heading should come first");
}

#[test]
fn cli_insert_in_place() {
    let f = tmp_org(FIXTURE);
    run(&[
        "insert",
        f.to_str().unwrap(),
        "-p",
        "Inbox",
        "--title",
        "New item",
        "--keyword",
        "TODO",
        "-i",
    ]);
    let content = fs::read_to_string(&f).unwrap();
    assert!(content.contains("** TODO New item"));
}

// ===========================================================================
// delete
// ===========================================================================

#[test]
fn cli_delete_basic() {
    let f = tmp_org(FIXTURE);
    let out = run(&["delete", f.to_str().unwrap(), "-p", "Task two"]);
    assert!(!out.contains("Task two"));
    // Other headings should remain
    assert!(out.contains("Task one"));
    assert!(out.contains("Projects"));
}

#[test]
fn cli_delete_subtree() {
    let f = tmp_org(FIXTURE);
    let out = run(&["delete", f.to_str().unwrap(), "-p", "Projects"]);
    assert!(!out.contains("Projects"));
    assert!(!out.contains("Project Alpha"));
    assert!(!out.contains("Project Beta"));
}

#[test]
fn cli_delete_in_place() {
    let f = tmp_org(FIXTURE);
    run(&["delete", f.to_str().unwrap(), "-p", "Task two", "-i"]);
    let content = fs::read_to_string(&f).unwrap();
    assert!(!content.contains("Task two"));
    assert!(content.contains("Task one"));
}

// ===========================================================================
// move
// ===========================================================================

#[test]
fn cli_move_basic() {
    let f = tmp_org(FIXTURE);
    let out = run(&[
        "move",
        f.to_str().unwrap(),
        "-p",
        "Inbox/Review docs",
        "--under",
        "Projects",
    ]);
    // Should appear under Projects
    assert!(out.contains("** Review docs"));
    // Count occurrences — should only appear once
    assert_eq!(out.matches("Review docs").count(), 1);
}

#[test]
fn cli_move_with_level_change() {
    // Move a level-2 heading to top level — stars should change
    let f = tmp_org(FIXTURE);
    let out = run(&[
        "move",
        f.to_str().unwrap(),
        "-p",
        "Projects/Project Alpha",
        "--under",
        "/",
    ]);
    // Was ** (level 2), now should be * (level 1)
    assert!(out.contains("* TODO Project Alpha"));
    assert!(!out.contains("** TODO Project Alpha"));
}

#[test]
fn cli_move_deep_level_change() {
    // Move a top-level heading under a level-2 heading — child becomes level 3
    let fixture_with_deep = "\
* Parent
** Child
* Orphan
** Grandchild
";
    let f = tmp_org(fixture_with_deep);
    let out = run(&[
        "move",
        f.to_str().unwrap(),
        "-p",
        "Orphan",
        "--under",
        "Parent/Child",
    ]);
    // Orphan was level 1, now level 3 (under Child which is level 2)
    assert!(out.contains("*** Orphan"));
    // Its child was level 2, now level 4
    assert!(out.contains("**** Grandchild"));
}

#[test]
fn cli_move_to_top_level() {
    let f = tmp_org(FIXTURE);
    let out = run(&[
        "move",
        f.to_str().unwrap(),
        "-p",
        "Projects/Project Alpha",
        "--under",
        "/",
    ]);
    assert!(out.contains("* TODO Project Alpha"));
}

#[test]
fn cli_move_in_place() {
    let f = tmp_org(FIXTURE);
    run(&[
        "move",
        f.to_str().unwrap(),
        "-p",
        "Inbox/Review docs",
        "--under",
        "Projects",
        "-i",
    ]);
    let content = fs::read_to_string(&f).unwrap();
    // "Review docs" should be under Projects now
    let proj_pos = content.find("* Projects").unwrap();
    let review_pos = content.find("** Review docs").unwrap();
    assert!(review_pos > proj_pos);
    // And not under Inbox
    let inbox_pos = content.find("* Inbox").unwrap();
    assert!(review_pos < inbox_pos);
}

// ===========================================================================
// edit
// ===========================================================================

#[test]
fn cli_edit_title() {
    let f = tmp_org(FIXTURE);
    let out = run(&[
        "edit",
        f.to_str().unwrap(),
        "-p",
        "Task one",
        "--title",
        "Task ONE (renamed)",
    ]);
    assert!(out.contains("Task ONE (renamed)"));
    assert!(!out.contains("Task one :work:"));
}

#[test]
fn cli_edit_priority() {
    let f = tmp_org(FIXTURE);
    let out = run(&[
        "edit",
        f.to_str().unwrap(),
        "-p",
        "Task one",
        "--priority",
        "C",
    ]);
    assert!(out.contains("[#C]"));
}

#[test]
fn cli_edit_priority_clear() {
    let f = tmp_org(FIXTURE);
    let out = run(&[
        "edit",
        f.to_str().unwrap(),
        "-p",
        "Task one",
        "--priority",
        "",
    ]);
    assert!(!out.contains("[#A]"));
    assert!(!out.contains("[#"));
}

#[test]
fn cli_edit_append() {
    let f = tmp_org(FIXTURE);
    let out = run(&[
        "edit",
        f.to_str().unwrap(),
        "-p",
        "Task one",
        "--append",
        "Additional note.",
    ]);
    assert!(out.contains("Body of task one"));
    assert!(out.contains("Additional note."));
    // Appended text must come after original
    let orig_pos = out.find("Body of task one").unwrap();
    let new_pos = out.find("Additional note.").unwrap();
    assert!(new_pos > orig_pos, "appended text should follow original body");
}

// ===========================================================================
// prop
// ===========================================================================

#[test]
fn cli_prop_get() {
    let f = tmp_org(FIXTURE);
    let out = run(&["prop", f.to_str().unwrap(), "-p", "Task one", "EFFORT"]);
    assert_eq!(out.trim(), "2h");
}

#[test]
fn cli_prop_set() {
    let f = tmp_org(FIXTURE);
    let out = run(&[
        "prop",
        f.to_str().unwrap(),
        "-p",
        "Task one",
        "EFFORT",
        "4h",
    ]);
    assert!(out.contains(":EFFORT: 4h"));
}

#[test]
fn cli_prop_set_new() {
    let f = tmp_org(FIXTURE);
    let out = run(&[
        "prop",
        f.to_str().unwrap(),
        "-p",
        "Task one",
        "CATEGORY",
        "errands",
    ]);
    assert!(out.contains(":CATEGORY: errands"));
}

#[test]
fn cli_prop_delete() {
    let f = tmp_org(FIXTURE);
    let out = run(&[
        "prop",
        f.to_str().unwrap(),
        "-p",
        "Task one",
        "EFFORT",
        "--delete",
    ]);
    assert!(!out.contains(":EFFORT:"));
    // Without any properties, no drawer
    assert!(!out.contains(":PROPERTIES:"));
}

#[test]
fn cli_prop_delete_one_of_many() {
    // Heading with multiple properties — delete one, others remain
    let multi_prop = "\
* Heading
:PROPERTIES:
:ID: abc
:EFFORT: 2h
:CATEGORY: work
:END:
";
    let f = tmp_org(multi_prop);
    let out = run(&[
        "prop",
        f.to_str().unwrap(),
        "-p",
        "Heading",
        "EFFORT",
        "--delete",
    ]);
    assert!(!out.contains(":EFFORT:"));
    // Drawer should still exist with the remaining properties
    assert!(out.contains(":PROPERTIES:"));
    assert!(out.contains(":ID: abc"));
    assert!(out.contains(":CATEGORY: work"));
    assert!(out.contains(":END:"));
}

#[test]
fn cli_prop_get_missing() {
    let f = tmp_org(FIXTURE);
    let stderr = run_err(&["prop", f.to_str().unwrap(), "-p", "Task one", "NONEXISTENT"]);
    assert!(stderr.contains("not found"));
}

// ===========================================================================
// tag
// ===========================================================================

#[test]
fn cli_tag_display() {
    let f = tmp_org(FIXTURE);
    let out = run(&["tag", f.to_str().unwrap(), "-p", "Task one"]);
    assert_eq!(out.trim(), ":work:");
}

#[test]
fn cli_tag_add() {
    let f = tmp_org(FIXTURE);
    let out = run(&[
        "tag",
        f.to_str().unwrap(),
        "-p",
        "Task one",
        "--add",
        "urgent",
    ]);
    assert!(out.contains(":work:urgent:"));
}

#[test]
fn cli_tag_remove() {
    let f = tmp_org(FIXTURE);
    let out = run(&[
        "tag",
        f.to_str().unwrap(),
        "-p",
        "Task one",
        "--remove",
        "work",
    ]);
    // Tag removed — heading should have no tags now
    assert!(!out.contains(":work:"));
}

#[test]
fn cli_tag_set() {
    let f = tmp_org(FIXTURE);
    let out = run(&[
        "tag",
        f.to_str().unwrap(),
        "-p",
        "Task one",
        "--set",
        "new1:new2",
    ]);
    assert!(out.contains(":new1:new2:"));
    assert!(!out.contains(":work:"));
}

#[test]
fn cli_tag_clear() {
    let f = tmp_org(FIXTURE);
    let out = run(&[
        "tag",
        f.to_str().unwrap(),
        "-p",
        "Task one",
        "--clear",
    ]);
    // No tags on the heading line
    let line = out.lines().find(|l| l.contains("Task one")).unwrap();
    assert!(!line.contains(':'));
}

// ===========================================================================
// promote / demote
// ===========================================================================

#[test]
fn cli_promote() {
    let f = tmp_org(FIXTURE);
    let out = run(&[
        "promote",
        f.to_str().unwrap(),
        "-p",
        "Projects/Project Alpha",
    ]);
    assert!(out.contains("* TODO Project Alpha"));
}

#[test]
fn cli_demote() {
    let f = tmp_org(FIXTURE);
    let out = run(&["demote", f.to_str().unwrap(), "-p", "Projects"]);
    assert!(out.contains("** Projects"));
    // Children should also be demoted
    assert!(out.contains("*** TODO Project Alpha"));
}

#[test]
fn cli_promote_at_level_1() {
    let f = tmp_org(FIXTURE);
    let stderr = run_err(&["promote", f.to_str().unwrap(), "-p", "Task one"]);
    assert!(stderr.contains("already at top level"));
}

// ===========================================================================
// Error handling
// ===========================================================================

#[test]
fn cli_nonexistent_file() {
    let stderr = run_err(&["show", "/nonexistent/file.org"]);
    assert!(stderr.contains("reading") || stderr.contains("No such file"));
}

#[test]
fn cli_bad_path() {
    let f = tmp_org(FIXTURE);
    let stderr = run_err(&[
        "todo",
        f.to_str().unwrap(),
        "-p",
        "Nonexistent heading",
        "DONE",
    ]);
    assert!(
        stderr.contains("no heading matches"),
        "stderr was: {}",
        stderr
    );
}

#[test]
fn cli_ambiguous_path() {
    let f = tmp_org(FIXTURE);
    let stderr = run_err(&[
        "todo",
        f.to_str().unwrap(),
        "-p",
        "Inbox/e", // matches "Review docs" and neither uniquely
        "DONE",
    ]);
    // Should give an error (either ambiguous or not found)
    assert!(!stderr.is_empty());
}

// ===========================================================================
// Round-trip: file written by -i should parse identically
// ===========================================================================

#[test]
fn cli_in_place_roundtrip() {
    let f = tmp_org(FIXTURE);

    // Apply a series of changes in-place
    run(&["todo", f.to_str().unwrap(), "-p", "Task one", "DONE", "-i"]);
    run(&[
        "insert",
        f.to_str().unwrap(),
        "-p",
        "Projects",
        "--title",
        "New project",
        "-i",
    ]);
    run(&["delete", f.to_str().unwrap(), "-p", "Task two", "-i"]);

    // Read the file, parse it, write it back, and compare
    let content = fs::read_to_string(&f).unwrap();
    let doc = chorg::parser::parse(&content);
    let rewritten = chorg::writer::write(&doc);
    assert_eq!(
        content, rewritten,
        "in-place file is not round-trip stable"
    );
}

#[test]
fn cli_stdout_matches_in_place() {
    let f1 = tmp_org(FIXTURE);
    let f2 = tmp_org(FIXTURE);

    // Get stdout output
    let stdout = run(&["todo", f1.to_str().unwrap(), "-p", "Task one", "DONE"]);

    // Get in-place output
    run(&["todo", f2.to_str().unwrap(), "-p", "Task one", "DONE", "-i"]);
    let in_place = fs::read_to_string(&f2).unwrap();

    assert_eq!(stdout, in_place, "stdout and in-place outputs differ");
}
