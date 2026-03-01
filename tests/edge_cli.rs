//! CLI edge-case tests — degenerate inputs, boundary conditions, and unusual
//! flag combinations exercised through the actual binary.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(1000);

fn tmp_org(content: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("edge_cli_{}.org", n));
    fs::write(&path, content).unwrap();
    path
}

fn chorg() -> Command {
    Command::new(env!("CARGO_BIN_EXE_chorg"))
}

fn run(args: &[&str]) -> String {
    let output = chorg().args(args).output().expect("failed to run chorg");
    assert!(
        output.status.success(),
        "chorg {:?} failed.\nstdout: {}\nstderr: {}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn run_err(args: &[&str]) -> String {
    let output = chorg().args(args).output().expect("failed to run chorg");
    assert!(!output.status.success(), "expected failure for {:?}", args);
    String::from_utf8(output.stderr).unwrap()
}

// ===================================================================
//  show edge cases
// ===================================================================

#[test]
fn cli_show_empty_file() {
    let f = tmp_org("");
    let out = run(&["show", f.to_str().unwrap()]);
    assert!(out.is_empty());
}

#[test]
fn cli_show_preamble_only() {
    let f = tmp_org("#+TITLE: Nothing here\n");
    let out = run(&["show", f.to_str().unwrap()]);
    assert!(out.is_empty()); // no headings to show
}

#[test]
fn cli_show_json_empty_file() {
    let f = tmp_org("");
    let out = run(&["show", f.to_str().unwrap(), "--json"]);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(v["headings"].as_array().unwrap().is_empty());
}

#[test]
fn cli_show_large_depth_on_flat() {
    let f = tmp_org("* A\n* B\n");
    let out = run(&["show", f.to_str().unwrap(), "-d", "100"]);
    assert!(out.contains("A"));
    assert!(out.contains("B"));
}

#[test]
fn cli_show_content_no_body() {
    // --content on a heading with no body shouldn't crash
    let f = tmp_org("* H\n");
    let out = run(&["show", f.to_str().unwrap(), "--content"]);
    assert!(out.contains("H"));
    assert!(!out.contains("|")); // no body to display
}

// ===================================================================
//  find edge cases
// ===================================================================

#[test]
fn cli_find_no_filters() {
    // No filters → all headings
    let f = tmp_org("* A\n** B\n* C\n");
    let out = run(&["find", f.to_str().unwrap()]);
    assert!(out.contains("A"));
    assert!(out.contains("B"));
    assert!(out.contains("C"));
}

#[test]
fn cli_find_all_filters_no_match() {
    let f = tmp_org("* TODO A :work:\n:PROPERTIES:\n:ID: x\n:END:\n");
    let out = run(&[
        "find", f.to_str().unwrap(),
        "--keyword", "DONE",
        "--tag", "personal",
        "--title", "zzz",
        "--property", "NONEXISTENT",
    ]);
    // No match — stdout empty
    assert!(out.is_empty() || !out.contains("*"));
}

#[test]
fn cli_find_property_with_value() {
    let f = tmp_org("* A\n:PROPERTIES:\n:X: 1\n:END:\n* B\n:PROPERTIES:\n:X: 2\n:END:\n");
    let out = run(&["find", f.to_str().unwrap(), "--property", "X=1"]);
    assert!(out.contains("A"));
    assert!(!out.contains("B"));
}

#[test]
fn cli_find_on_empty_file() {
    let f = tmp_org("");
    let out = run(&["find", f.to_str().unwrap(), "--keyword", "TODO"]);
    assert!(out.is_empty() || !out.contains("*"));
}

// ===================================================================
//  todo edge cases
// ===================================================================

#[test]
fn cli_todo_on_heading_with_priority() {
    let f = tmp_org("* TODO [#A] Task\n");
    let out = run(&["todo", f.to_str().unwrap(), "-p", "#1", "DONE"]);
    assert!(out.contains("* DONE [#A] Task"));
}

#[test]
fn cli_todo_to_custom_keyword() {
    // Setting a keyword that isn't in the configured list — should still work
    let f = tmp_org("* TODO Task\n");
    let out = run(&["todo", f.to_str().unwrap(), "-p", "#1", "CUSTOM_STATE"]);
    assert!(out.contains("CUSTOM_STATE"));
}

#[test]
fn cli_todo_preserves_tags() {
    let f = tmp_org("* TODO Task :a:b:c:\n");
    let out = run(&["todo", f.to_str().unwrap(), "-p", "#1", "DONE"]);
    assert!(out.contains(":a:b:c:"));
}

// ===================================================================
//  insert edge cases
// ===================================================================

#[test]
fn cli_insert_into_empty_file() {
    let f = tmp_org("");
    let out = run(&[
        "insert", f.to_str().unwrap(), "-p", "/", "--title", "First",
    ]);
    assert!(out.contains("* First"));
}

#[test]
fn cli_insert_position_zero() {
    // Position 0 should be treated as 1 (first position)
    let f = tmp_org("* P\n** A\n** B\n");
    let out = run(&[
        "insert", f.to_str().unwrap(), "-p", "P",
        "--title", "New", "-n", "0",
    ]);
    let new_pos = out.find("New").unwrap();
    let a_pos = out.find("** A").unwrap();
    assert!(new_pos < a_pos);
}

#[test]
fn cli_insert_position_beyond_end() {
    // Position larger than children count should append
    let f = tmp_org("* P\n** A\n");
    let out = run(&[
        "insert", f.to_str().unwrap(), "-p", "P",
        "--title", "New", "-n", "999",
    ]);
    let a_pos = out.find("** A").unwrap();
    let new_pos = out.find("** New").unwrap();
    assert!(new_pos > a_pos);
}

#[test]
fn cli_insert_with_body() {
    let f = tmp_org("* P\n");
    let out = run(&[
        "insert", f.to_str().unwrap(), "-p", "P",
        "--title", "New", "--body", "Line 1\nLine 2",
    ]);
    assert!(out.contains("** New"));
    assert!(out.contains("Line 1"));
    assert!(out.contains("Line 2"));
}

#[test]
fn cli_insert_with_priority() {
    let f = tmp_org("* P\n");
    let out = run(&[
        "insert", f.to_str().unwrap(), "-p", "P",
        "--title", "Task", "--keyword", "TODO", "--priority", "B",
    ]);
    assert!(out.contains("** TODO [#B] Task"));
}

#[test]
fn cli_insert_level_inherits_from_parent() {
    // Insert under a level-3 heading — child should be level 4
    let f = tmp_org("* A\n** B\n*** C\n");
    let out = run(&[
        "insert", f.to_str().unwrap(), "-p", "A/B/C",
        "--title", "Deep",
    ]);
    assert!(out.contains("**** Deep"));
}

// ===================================================================
//  delete edge cases
// ===================================================================

#[test]
fn cli_delete_first_child() {
    let f = tmp_org("* P\n** A\n** B\n** C\n");
    let out = run(&["delete", f.to_str().unwrap(), "-p", "P/A"]);
    assert!(!out.contains("** A"));
    assert!(out.contains("** B"));
    assert!(out.contains("** C"));
}

#[test]
fn cli_delete_last_child() {
    let f = tmp_org("* P\n** A\n** B\n** C\n");
    let out = run(&["delete", f.to_str().unwrap(), "-p", "P/C"]);
    assert!(out.contains("** A"));
    assert!(out.contains("** B"));
    assert!(!out.contains("** C"));
}

#[test]
fn cli_delete_all_top_level() {
    let f = tmp_org("#+TITLE: Keep\n\n* A\n* B\n");
    run(&["delete", f.to_str().unwrap(), "-p", "A", "-i"]);
    run(&["delete", f.to_str().unwrap(), "-p", "B", "-i"]);
    let content = fs::read_to_string(&f).unwrap();
    assert!(content.contains("#+TITLE: Keep"));
    assert!(!content.contains("* A"));
    assert!(!content.contains("* B"));
}

// ===================================================================
//  move edge cases
// ===================================================================

#[test]
fn cli_move_with_position() {
    let f = tmp_org("* Src\n* Dst\n** A\n** B\n");
    let out = run(&[
        "move", f.to_str().unwrap(), "-p", "Src", "--under", "Dst", "-n", "1",
    ]);
    // Src should appear before A under Dst
    let src_pos = out.find("** Src").unwrap();
    let a_pos = out.find("** A").unwrap();
    assert!(src_pos < a_pos);
}

#[test]
fn cli_move_preserves_children() {
    let f = tmp_org("* P\n** A\n*** A1\n*** A2\n* Dst\n");
    let out = run(&[
        "move", f.to_str().unwrap(), "-p", "P/A", "--under", "Dst",
    ]);
    assert!(out.contains("** A"));
    assert!(out.contains("*** A1"));
    assert!(out.contains("*** A2"));
    // Parent should lose child
    assert!(!out.contains("* P\n** A")); // A no longer under P
}

#[test]
fn cli_move_preserves_body_and_properties() {
    let f = tmp_org("\
* Src
:PROPERTIES:
:ID: s
:END:
Body here
* Dst
");
    let out = run(&[
        "move", f.to_str().unwrap(), "-p", "Src", "--under", "Dst",
    ]);
    assert!(out.contains(":ID: s"));
    assert!(out.contains("Body here"));
}

// ===================================================================
//  edit edge cases
// ===================================================================

#[test]
fn cli_edit_body_replace_with_empty() {
    let f = tmp_org("* H\nOld body\n");
    let out = run(&[
        "edit", f.to_str().unwrap(), "-p", "#1", "--body", "",
    ]);
    assert!(!out.contains("Old body"));
}

#[test]
fn cli_edit_body_with_org_syntax() {
    let f = tmp_org("* H\n");
    let out = run(&[
        "edit", f.to_str().unwrap(), "-p", "#1",
        "--body", "#+BEGIN_SRC\ncode\n#+END_SRC",
    ]);
    assert!(out.contains("#+BEGIN_SRC"));
    assert!(out.contains("code"));
    assert!(out.contains("#+END_SRC"));
}

#[test]
fn cli_edit_multiple_fields_at_once() {
    let f = tmp_org("* TODO [#A] Old title\nOld body\n");
    let out = run(&[
        "edit", f.to_str().unwrap(), "-p", "#1",
        "--title", "New title",
        "--priority", "C",
        "--body", "New body",
    ]);
    assert!(out.contains("New title"));
    assert!(out.contains("[#C]"));
    assert!(out.contains("New body"));
    assert!(!out.contains("Old title"));
    assert!(!out.contains("[#A]"));
    assert!(!out.contains("Old body"));
}

#[test]
fn cli_edit_priority_lowercase_uppercased() {
    let f = tmp_org("* TODO Task\n");
    let out = run(&[
        "edit", f.to_str().unwrap(), "-p", "#1", "--priority", "b",
    ]);
    assert!(out.contains("[#B]"));
}

// ===================================================================
//  prop edge cases
// ===================================================================

#[test]
fn cli_prop_set_creates_drawer() {
    let f = tmp_org("* H\n");
    let out = run(&[
        "prop", f.to_str().unwrap(), "-p", "#1", "ID", "new-id",
    ]);
    assert!(out.contains(":PROPERTIES:"));
    assert!(out.contains(":ID: new-id"));
    assert!(out.contains(":END:"));
}

#[test]
fn cli_prop_delete_nonexistent() {
    let f = tmp_org("* H\n");
    let stderr = run_err(&[
        "prop", f.to_str().unwrap(), "-p", "#1", "NOPE", "--delete",
    ]);
    assert!(stderr.contains("not found"));
}

#[test]
fn cli_prop_get_case_insensitive() {
    let f = tmp_org("* H\n:PROPERTIES:\n:MyKey: value\n:END:\n");
    let out = run(&["prop", f.to_str().unwrap(), "-p", "#1", "mykey"]);
    assert_eq!(out.trim(), "value");
}

#[test]
fn cli_prop_set_preserves_body() {
    let f = tmp_org("* H\nBody text\n");
    let out = run(&[
        "prop", f.to_str().unwrap(), "-p", "#1", "KEY", "val",
    ]);
    assert!(out.contains("Body text"));
    assert!(out.contains(":KEY: val"));
}

#[test]
fn cli_prop_set_preserves_planning() {
    let f = tmp_org("* TODO H\nSCHEDULED: <2024-01-01>\n");
    let out = run(&[
        "prop", f.to_str().unwrap(), "-p", "#1", "ID", "x",
    ]);
    assert!(out.contains("SCHEDULED:"));
    assert!(out.contains(":ID: x"));
}

// ===================================================================
//  tag edge cases
// ===================================================================

#[test]
fn cli_tag_display_no_tags() {
    let f = tmp_org("* H\n");
    let out = run(&["tag", f.to_str().unwrap(), "-p", "#1"]);
    assert_eq!(out.trim(), "(no tags)");
}

#[test]
fn cli_tag_add_multiple() {
    let f = tmp_org("* H\n");
    let out = run(&[
        "tag", f.to_str().unwrap(), "-p", "#1",
        "--add", "a", "--add", "b", "--add", "c",
    ]);
    assert!(out.contains(":a:b:c:"));
}

#[test]
fn cli_tag_remove_multiple() {
    let f = tmp_org("* H :a:b:c:d:\n");
    let out = run(&[
        "tag", f.to_str().unwrap(), "-p", "#1",
        "--remove", "b", "--remove", "d",
    ]);
    assert!(out.contains(":a:c:"));
    assert!(!out.contains("b"));
    assert!(!out.contains("d"));
}

#[test]
fn cli_tag_add_duplicate_ignored() {
    let f = tmp_org("* H :work:\n");
    let out = run(&[
        "tag", f.to_str().unwrap(), "-p", "#1", "--add", "work",
    ]);
    // Should still only have one :work:
    let line = out.lines().find(|l| l.contains("H")).unwrap();
    assert_eq!(line.matches("work").count(), 1);
}

#[test]
fn cli_tag_add_case_insensitive_dedup() {
    let f = tmp_org("* H :Work:\n");
    let out = run(&[
        "tag", f.to_str().unwrap(), "-p", "#1", "--add", "WORK",
    ]);
    // Case-insensitive dedup — should not add a duplicate
    let line = out.lines().find(|l| l.contains("H")).unwrap();
    let colon_count = line.matches(':').count();
    assert_eq!(colon_count, 2); // :Work: = 2 colons
}

// ===================================================================
//  promote/demote edge cases
// ===================================================================

#[test]
fn cli_demote_deep() {
    let f = tmp_org("* H\n** C\n");
    let out = run(&["demote", f.to_str().unwrap(), "-p", "#1"]);
    assert!(out.contains("** H"));
    assert!(out.contains("*** C"));
}

#[test]
fn cli_promote_child() {
    let f = tmp_org("* P\n** C\n*** GC\n");
    let out = run(&["promote", f.to_str().unwrap(), "-p", "P/C"]);
    // C promoted from 2 to 1, GC from 3 to 2
    assert!(out.contains("* C"));
    assert!(out.contains("** GC"));
}

// ===================================================================
//  Sequential in-place operations
// ===================================================================

#[test]
fn cli_many_sequential_in_place() {
    let f = tmp_org("* A\n* B\n* C\n");

    // Insert under A
    run(&["insert", f.to_str().unwrap(), "-p", "A", "--title", "A1", "-i"]);
    // Change B to DONE
    run(&["todo", f.to_str().unwrap(), "-p", "B", "DONE", "-i"]);
    // Add tag to C
    run(&["tag", f.to_str().unwrap(), "-p", "C", "--add", "tagged", "-i"]);
    // Delete B
    run(&["delete", f.to_str().unwrap(), "-p", "B", "-i"]);
    // Insert another top-level
    run(&["insert", f.to_str().unwrap(), "-p", "/", "--title", "D", "-i"]);

    let content = fs::read_to_string(&f).unwrap();
    let doc = chorg::parser::parse(&content);
    let titles: Vec<&str> = doc.headings.iter().map(|h| h.title.as_str()).collect();
    assert!(titles.contains(&"A"));
    assert!(!titles.contains(&"B")); // deleted
    assert!(titles.contains(&"C"));
    assert!(titles.contains(&"D"));

    // A should have child A1
    let a = doc.headings.iter().find(|h| h.title == "A").unwrap();
    assert_eq!(a.children.len(), 1);
    assert_eq!(a.children[0].title, "A1");

    // C should have tag
    let c = doc.headings.iter().find(|h| h.title == "C").unwrap();
    assert_eq!(c.tags, vec!["tagged"]);

    // Verify round-trip stability
    let rewritten = chorg::writer::write(&doc);
    assert_eq!(content, rewritten);
}

// ===================================================================
//  Error path edge cases
// ===================================================================

#[test]
fn cli_path_on_empty_file() {
    let f = tmp_org("");
    let stderr = run_err(&["show", f.to_str().unwrap(), "-p", "Anything"]);
    assert!(!stderr.is_empty());
}

#[test]
fn cli_delete_nonexistent_heading() {
    let f = tmp_org("* A\n");
    let stderr = run_err(&["delete", f.to_str().unwrap(), "-p", "B"]);
    assert!(stderr.contains("no heading matches"));
}

#[test]
fn cli_move_nonexistent_dest() {
    let f = tmp_org("* A\n* B\n");
    let stderr = run_err(&[
        "move", f.to_str().unwrap(), "-p", "A", "--under", "Z",
    ]);
    // After removing A, Z still doesn't exist among remaining headings
    assert!(
        stderr.contains("no heading matches") || stderr.contains("no headings"),
        "stderr was: {}", stderr
    );
}

#[test]
fn cli_edit_nonexistent_heading() {
    let f = tmp_org("* A\n");
    let stderr = run_err(&[
        "edit", f.to_str().unwrap(), "-p", "Z", "--title", "New",
    ]);
    assert!(stderr.contains("no heading matches"));
}

#[test]
fn cli_positional_on_empty_children() {
    let f = tmp_org("* Leaf\n");
    let stderr = run_err(&["show", f.to_str().unwrap(), "-p", "Leaf/#1"]);
    assert!(stderr.contains("no headings at this level"));
}
