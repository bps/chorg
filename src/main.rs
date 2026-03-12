use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use serde::Serialize;
use std::path::PathBuf;

use chorg::model::{Heading, OrgDoc};
use chorg::parser;
use chorg::path as orgpath;
use chorg::writer;

// ===========================================================================
// CLI definition
// ===========================================================================

#[derive(Parser)]
#[command(
    name = "chorg",
    version,
    about = "Structured org-mode document editor for humans and LLM agents"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Display the outline (or a subtree) with positional addresses.
    Show {
        file: PathBuf,
        /// Heading path to show (omit for full outline).
        #[arg(short, long)]
        path: Option<String>,
        /// Maximum depth to display (relative to target).
        #[arg(short, long)]
        depth: Option<usize>,
        /// Output as JSON.
        #[arg(long)]
        json: bool,
        /// Include body text in output.
        #[arg(long)]
        content: bool,
    },

    /// Search headings by keyword, tag, title, or property.
    Find {
        file: PathBuf,
        /// Match TODO keyword (exact, case-insensitive).
        #[arg(long)]
        keyword: Option<String>,
        /// Exclude headings with this keyword.
        #[arg(long)]
        no_keyword: Option<String>,
        /// Match tag (exact, repeatable: all must match).
        #[arg(long)]
        tag: Vec<String>,
        /// Exclude headings with this tag.
        #[arg(long)]
        no_tag: Vec<String>,
        /// Minimum heading level (1 = top-level).
        #[arg(long)]
        min_level: Option<usize>,
        /// Maximum heading level.
        #[arg(long)]
        max_level: Option<usize>,
        /// Only headings with a SCHEDULED timestamp.
        #[arg(long)]
        scheduled: bool,
        /// Only headings with a DEADLINE timestamp.
        #[arg(long)]
        deadline: bool,
        /// Match title (substring, case-insensitive).
        #[arg(long)]
        title: Option<String>,
        /// Match body text (substring, case-insensitive).
        #[arg(long)]
        body: Option<String>,
        /// Match property KEY=VALUE.
        #[arg(long)]
        property: Option<String>,
        /// Limit search to descendants of this heading.
        #[arg(long)]
        under: Option<String>,
        /// Output as JSON.
        #[arg(long)]
        json: bool,
    },

    /// Change the TODO keyword of a heading.
    Todo {
        file: PathBuf,
        /// Heading path.
        #[arg(short, long)]
        path: String,
        /// New keyword (e.g. TODO, DONE). Use empty string to clear.
        state: String,
        /// Write changes back to the file.
        #[arg(short, long)]
        in_place: bool,
        /// Suppress dry-run output; only print the confirmation line.
        #[arg(short, long)]
        quiet: bool,
    },

    /// Insert a new heading.
    Insert {
        file: PathBuf,
        /// Path to the *parent* heading (use "/" for top-level).
        #[arg(short, long)]
        path: String,
        /// Heading title.
        #[arg(long)]
        title: String,
        /// TODO keyword.
        #[arg(long)]
        keyword: Option<String>,
        /// Priority (A, B, C, …).
        #[arg(long)]
        priority: Option<char>,
        /// Tags (colon-separated: `work:urgent`).
        #[arg(long)]
        tags: Option<String>,
        /// Body text.
        #[arg(long)]
        body: Option<String>,
        /// 1-based position among siblings (default: append).
        #[arg(short = 'n', long)]
        position: Option<usize>,
        /// Write changes back to the file.
        #[arg(short, long)]
        in_place: bool,
        /// Suppress dry-run output; only print the confirmation line.
        #[arg(short, long)]
        quiet: bool,
    },

    /// Delete a heading and its entire subtree.
    Delete {
        file: PathBuf,
        /// Heading path.
        #[arg(short, long)]
        path: String,
        /// Write changes back to the file.
        #[arg(short, long)]
        in_place: bool,
        /// Suppress dry-run output; only print the confirmation line.
        #[arg(short, long)]
        quiet: bool,
    },

    /// Move (refile) a heading under a new parent.
    Move {
        file: PathBuf,
        /// Path of the heading to move.
        #[arg(short, long)]
        path: String,
        /// Path of the new parent (use "/" for top-level).
        #[arg(long)]
        under: String,
        /// 1-based position among new siblings (default: append).
        #[arg(short = 'n', long)]
        position: Option<usize>,
        /// Write changes back to the file.
        #[arg(short, long)]
        in_place: bool,
        /// Suppress dry-run output; only print the confirmation line.
        #[arg(short, long)]
        quiet: bool,
    },

    /// Edit heading fields (title, priority, body).
    Edit {
        file: PathBuf,
        /// Heading path.
        #[arg(short, long)]
        path: String,
        /// New title.
        #[arg(long)]
        title: Option<String>,
        /// New priority (letter or "" to clear).
        #[arg(long)]
        priority: Option<String>,
        /// Replace body text.
        #[arg(long)]
        body: Option<String>,
        /// Append text to body.
        #[arg(long)]
        append: Option<String>,
        /// Write changes back to the file.
        #[arg(short, long)]
        in_place: bool,
        /// Suppress dry-run output; only print the confirmation line.
        #[arg(short, long)]
        quiet: bool,
    },

    /// Get, set, or delete a property.
    Prop {
        file: PathBuf,
        /// Heading path.
        #[arg(short, long)]
        path: String,
        /// Property key.
        key: String,
        /// Property value (omit to get; provide to set).
        value: Option<String>,
        /// Delete the property.
        #[arg(long)]
        delete: bool,
        /// Write changes back to the file.
        #[arg(short, long)]
        in_place: bool,
        /// Suppress dry-run output; only print the confirmation line.
        #[arg(short, long)]
        quiet: bool,
    },

    /// Modify tags on a heading.
    Tag {
        file: PathBuf,
        /// Heading path.
        #[arg(short, long)]
        path: String,
        /// Add a tag.
        #[arg(long)]
        add: Vec<String>,
        /// Remove a tag.
        #[arg(long)]
        remove: Vec<String>,
        /// Set tags (colon-separated, replaces all).
        #[arg(long)]
        set: Option<String>,
        /// Remove all tags.
        #[arg(long)]
        clear: bool,
        /// Write changes back to the file.
        #[arg(short, long)]
        in_place: bool,
        /// Suppress dry-run output; only print the confirmation line.
        #[arg(short, long)]
        quiet: bool,
    },

    /// Promote (decrease level) or demote (increase level) a heading and its subtree.
    Promote {
        file: PathBuf,
        /// Heading path.
        #[arg(short, long)]
        path: String,
        /// Write changes back to the file.
        #[arg(short, long)]
        in_place: bool,
        /// Suppress dry-run output; only print the confirmation line.
        #[arg(short, long)]
        quiet: bool,
    },
    /// Demote a heading and its subtree.
    Demote {
        file: PathBuf,
        /// Heading path.
        #[arg(short, long)]
        path: String,
        /// Write changes back to the file.
        #[arg(short, long)]
        in_place: bool,
        /// Suppress dry-run output; only print the confirmation line.
        #[arg(short, long)]
        quiet: bool,
    },
}

// ===========================================================================
// Main
// ===========================================================================

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Show {
            file,
            path,
            depth,
            json,
            content,
        } => cmd_show(&file, path.as_deref(), depth, json, content),

        Commands::Find {
            file,
            keyword,
            no_keyword,
            tag,
            no_tag,
            min_level,
            max_level,
            scheduled,
            deadline,
            title,
            body,
            property,
            under,
            json,
        } => cmd_find(&file, keyword, no_keyword, tag, no_tag, min_level, max_level, scheduled, deadline, title, body, property, under, json),

        Commands::Todo {
            file,
            path,
            state,
            in_place,
            quiet,
        } => cmd_todo(&file, &path, &state, in_place, quiet),

        Commands::Insert {
            file,
            path,
            title,
            keyword,
            priority,
            tags,
            body,
            position,
            in_place,
            quiet,
        } => cmd_insert(
            &file, &path, &title, keyword, priority, tags, body, position, in_place, quiet,
        ),

        Commands::Delete {
            file,
            path,
            in_place,
            quiet,
        } => cmd_delete(&file, &path, in_place, quiet),

        Commands::Move {
            file,
            path,
            under,
            position,
            in_place,
            quiet,
        } => cmd_move(&file, &path, &under, position, in_place, quiet),

        Commands::Edit {
            file,
            path,
            title,
            priority,
            body,
            append,
            in_place,
            quiet,
        } => cmd_edit(&file, &path, title, priority, body, append, in_place, quiet),

        Commands::Prop {
            file,
            path,
            key,
            value,
            delete,
            in_place,
            quiet,
        } => cmd_prop(&file, &path, &key, value, delete, in_place, quiet),

        Commands::Tag {
            file,
            path,
            add,
            remove,
            set,
            clear,
            in_place,
            quiet,
        } => cmd_tag(&file, &path, add, remove, set, clear, in_place, quiet),

        Commands::Promote {
            file,
            path,
            in_place,
            quiet,
        } => cmd_promote(&file, &path, -1, in_place, quiet),

        Commands::Demote {
            file,
            path,
            in_place,
            quiet,
        } => cmd_promote(&file, &path, 1, in_place, quiet),
    }
}

// ===========================================================================
// Helpers
// ===========================================================================

fn read_doc(file: &PathBuf) -> Result<(String, OrgDoc)> {
    let text = std::fs::read_to_string(file)
        .with_context(|| format!("reading {}", file.display()))?;
    let doc = parser::parse(&text);
    Ok((text, doc))
}

fn emit(file: &PathBuf, doc: &OrgDoc, in_place: bool, quiet: bool) -> Result<()> {
    let text = writer::write(doc);
    if in_place {
        std::fs::write(file, &text)
            .with_context(|| format!("writing {}", file.display()))?;
        eprintln!("wrote {}", file.display());
    } else if !quiet {
        print!("{}", text);
    }
    Ok(())
}

// ===========================================================================
// Display helpers
// ===========================================================================

fn format_outline(
    headings: &[Heading],
    base_addr: &[usize],
    depth: Option<usize>,
    show_content: bool,
) -> String {
    let mut out = String::new();
    for (i, h) in headings.iter().enumerate() {
        let mut addr = base_addr.to_vec();
        addr.push(i);
        format_heading_line(&mut out, h, &addr, show_content);
        if depth.map_or(true, |d| d > 0) {
            let child_depth = depth.map(|d| d.saturating_sub(1));
            out.push_str(&format_outline(&h.children, &addr, child_depth, show_content));
        }
    }
    out
}

fn format_heading_line(out: &mut String, h: &Heading, addr: &[usize], show_content: bool) {
    let addr_str = Heading::format_addr(addr);

    let mut headline = String::new();
    if let Some(ref kw) = h.keyword {
        headline.push_str(kw);
        headline.push(' ');
    }
    if let Some(p) = h.priority {
        headline.push_str(&format!("[#{}] ", p));
    }
    headline.push_str(&h.title);
    if !h.tags.is_empty() {
        headline.push_str(&format!(" :{}:", h.tags.join(":")));
    }

    out.push_str(&format!("{:<8} {}\n", addr_str, headline));

    if show_content && !h.body.is_empty() {
        for line in h.body.lines() {
            out.push_str(&format!("         | {}\n", line));
        }
    }
}

// ---------------------------------------------------------------------------
// JSON output helpers
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct JsonOutline {
    #[serde(skip_serializing_if = "Option::is_none")]
    preamble: Option<String>,
    headings: Vec<JsonHeading>,
}

#[derive(Serialize)]
struct JsonHeading {
    addr: String,
    path: String,
    level: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    keyword: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    priority: Option<char>,
    title: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    planning: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    properties: Vec<JsonProperty>,
    #[serde(skip_serializing_if = "String::is_empty")]
    body: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    children: Vec<JsonHeading>,
}

#[derive(Serialize)]
struct JsonProperty {
    key: String,
    value: String,
}

fn heading_to_json(h: &Heading, addr: &[usize], depth: Option<usize>, doc: &OrgDoc) -> JsonHeading {
    let children = if depth.map_or(true, |d| d > 0) {
        let child_depth = depth.map(|d| d.saturating_sub(1));
        h.children
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let mut a = addr.to_vec();
                a.push(i);
                heading_to_json(c, &a, child_depth, doc)
            })
            .collect()
    } else {
        Vec::new()
    };

    JsonHeading {
        addr: Heading::format_addr(addr),
        path: Heading::format_title_path(doc, addr),
        level: h.level,
        keyword: h.keyword.clone(),
        priority: h.priority,
        title: h.title.clone(),
        tags: h.tags.clone(),
        planning: h.planning.clone(),
        properties: h.properties.iter().map(|(k, v)| JsonProperty { key: k.clone(), value: v.clone() }).collect(),
        body: h.body.trim_end().to_string(),
        children,
    }
}

// ===========================================================================
// Commands
// ===========================================================================

fn cmd_show(
    file: &PathBuf,
    path: Option<&str>,
    depth: Option<usize>,
    json: bool,
    content: bool,
) -> Result<()> {
    let (_, doc) = read_doc(file)?;

    if json {
        let outline = if let Some(p) = path {
            let indices = orgpath::resolve(&doc, p)?;
            let h = doc.heading_at(&indices);
            JsonOutline {
                preamble: None,
                headings: vec![heading_to_json(h, &indices, depth, &doc)],
            }
        } else {
            JsonOutline {
                preamble: if doc.preamble.is_empty() {
                    None
                } else {
                    Some(doc.preamble.trim_end().to_string())
                },
                headings: doc
                    .headings
                    .iter()
                    .enumerate()
                    .map(|(i, h)| heading_to_json(h, &[i], depth, &doc))
                    .collect(),
            }
        };
        println!("{}", serde_json::to_string_pretty(&outline)?);
    } else {
        let text = if let Some(p) = path {
            let indices = orgpath::resolve(&doc, p)?;
            let h = doc.heading_at(&indices);
            let mut out = String::new();
            format_heading_line(&mut out, h, &indices, content);
            if depth.map_or(true, |d| d > 0) {
                let child_depth = depth.map(|d| d.saturating_sub(1));
                out.push_str(&format_outline(&h.children, &indices, child_depth, content));
            }
            out
        } else {
            format_outline(&doc.headings, &[], depth, content)
        };
        print!("{}", text);
    }
    Ok(())
}

fn cmd_find(
    file: &PathBuf,
    keyword: Option<String>,
    no_keyword: Option<String>,
    tag: Vec<String>,
    no_tag: Vec<String>,
    min_level: Option<usize>,
    max_level: Option<usize>,
    scheduled: bool,
    deadline: bool,
    title: Option<String>,
    body: Option<String>,
    property: Option<String>,
    under: Option<String>,
    json: bool,
) -> Result<()> {
    let (_, doc) = read_doc(file)?;

    // Parse property filter
    let prop_filter: Option<(String, String)> = property.map(|p| {
        if let Some((k, v)) = p.split_once('=') {
            (k.to_string(), v.to_string())
        } else {
            (p, String::new())
        }
    });

    // Resolve scope: if --under is given, only walk descendants of that heading.
    let scope_indices: Option<Vec<usize>> = under
        .as_deref()
        .map(|u| orgpath::resolve(&doc, u))
        .transpose()?;

    let all = doc.walk();
    let matches: Vec<&(Vec<usize>, &Heading)> = all
        .iter()
        .filter(|(addr, h)| {
            // Scope filter: heading must be a strict descendant of the scope.
            if let Some(ref scope) = scope_indices {
                if addr.len() <= scope.len() {
                    return false; // same level or higher — not a descendant
                }
                if &addr[..scope.len()] != scope.as_slice() {
                    return false; // different branch
                }
            }
            if let Some(min) = min_level {
                if h.level < min {
                    return false;
                }
            }
            if let Some(max) = max_level {
                if h.level > max {
                    return false;
                }
            }
            if scheduled {
                match &h.planning {
                    Some(p) if p.contains("SCHEDULED:") => {}
                    _ => return false,
                }
            }
            if deadline {
                match &h.planning {
                    Some(p) if p.contains("DEADLINE:") => {}
                    _ => return false,
                }
            }
            if let Some(ref kw) = keyword {
                match &h.keyword {
                    Some(hkw) if hkw.eq_ignore_ascii_case(kw) => {}
                    _ => return false,
                }
            }
            if let Some(ref nkw) = no_keyword {
                if let Some(ref hkw) = h.keyword {
                    if hkw.eq_ignore_ascii_case(nkw) {
                        return false;
                    }
                }
            }
            for t in &tag {
                if !h.tags.iter().any(|ht| ht.eq_ignore_ascii_case(t)) {
                    return false;
                }
            }
            for nt in &no_tag {
                if h.tags.iter().any(|ht| ht.eq_ignore_ascii_case(nt)) {
                    return false;
                }
            }
            if let Some(ref pat) = title {
                let pat_lower = pat.to_lowercase();
                if !h.title.to_lowercase().contains(&pat_lower) {
                    return false;
                }
            }
            if let Some(ref pat) = body {
                let pat_lower = pat.to_lowercase();
                if !h.body.to_lowercase().contains(&pat_lower) {
                    return false;
                }
            }
            if let Some((ref pk, ref pv)) = prop_filter {
                let found = h.properties.iter().any(|(k, v)| {
                    k.eq_ignore_ascii_case(pk) && (pv.is_empty() || v == pv)
                });
                if !found {
                    return false;
                }
            }
            true
        })
        .collect();

    if json {
        let results: Vec<JsonHeading> = matches
            .iter()
            .map(|(addr, h)| heading_to_json(h, addr, Some(0), &doc))
            .collect();
        println!("{}", serde_json::to_string_pretty(&results)?);
    } else {
        if matches.is_empty() {
            eprintln!("no matches");
        }
        for (addr, h) in &matches {
            let mut out = String::new();
            format_heading_line(&mut out, h, addr, false);
            print!("{}", out);
        }
    }

    if matches.is_empty() {
        std::process::exit(1);
    }

    Ok(())
}

fn cmd_todo(file: &PathBuf, path: &str, state: &str, in_place: bool, quiet: bool) -> Result<()> {
    let (_, mut doc) = read_doc(file)?;
    let indices = orgpath::resolve(&doc, path)?;

    let new_kw = if state.is_empty() {
        None
    } else {
        Some(state.to_string())
    };

    let h = doc.heading_at_mut(&indices);
    let old = h.keyword.clone();
    h.keyword = new_kw.clone();

    eprintln!(
        "{}: {} → {}",
        Heading::format_addr(&indices),
        old.as_deref().unwrap_or("(none)"),
        new_kw.as_deref().unwrap_or("(none)")
    );

    emit(file, &doc, in_place, quiet)
}

fn cmd_insert(
    file: &PathBuf,
    parent_path: &str,
    title: &str,
    keyword: Option<String>,
    priority: Option<char>,
    tags: Option<String>,
    body: Option<String>,
    position: Option<usize>,
    in_place: bool,
    quiet: bool,
) -> Result<()> {
    let (_, mut doc) = read_doc(file)?;

    let tag_list: Vec<String> = tags
        .map(|t| t.split(':').map(|s| s.to_string()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();

    let body_text = body
        .map(|b| if b.ends_with('\n') { b } else { format!("{}\n", b) })
        .unwrap_or_default();

    let new_heading = if parent_path == "/" {
        // Top-level insertion
        let level = 1;
        let h = Heading {
            level,
            keyword,
            priority,
            title: title.to_string(),
            tags: tag_list,
            planning: None,
            properties: Vec::new(),
            body: body_text,
            children: Vec::new(),
        };
        let pos = position.map(|p| p.saturating_sub(1)).unwrap_or(doc.headings.len());
        let pos = pos.min(doc.headings.len());
        doc.headings.insert(pos, h);
        eprintln!("inserted at {}", Heading::format_addr(&[pos]));
        return emit(file, &doc, in_place, quiet);
    } else {
        let indices = orgpath::resolve(&doc, parent_path)?;
        let parent = doc.heading_at(&indices);
        let level = parent.level + 1;
        Heading {
            level,
            keyword,
            priority,
            title: title.to_string(),
            tags: tag_list,
            planning: None,
            properties: Vec::new(),
            body: body_text,
            children: Vec::new(),
        }
    };

    // Re-resolve for mutable access (borrow rules)
    let indices = orgpath::resolve(&doc, parent_path)?;
    let parent = doc.heading_at_mut(&indices);
    let pos = position
        .map(|p| p.saturating_sub(1))
        .unwrap_or(parent.children.len());
    let pos = pos.min(parent.children.len());
    parent.children.insert(pos, new_heading);

    let mut child_addr = indices.clone();
    child_addr.push(pos);
    eprintln!("inserted at {}", Heading::format_addr(&child_addr));

    emit(file, &doc, in_place, quiet)
}

fn cmd_delete(file: &PathBuf, path: &str, in_place: bool, quiet: bool) -> Result<()> {
    let (_, mut doc) = read_doc(file)?;
    let indices = orgpath::resolve(&doc, path)?;

    let title = doc.heading_at(&indices).title.clone();
    let (list, idx) = doc.parent_list_mut(&indices);
    list.remove(idx);

    eprintln!("deleted {} ({:?})", Heading::format_addr(&indices), title);
    emit(file, &doc, in_place, quiet)
}

fn cmd_move(
    file: &PathBuf,
    path: &str,
    under: &str,
    position: Option<usize>,
    in_place: bool,
    quiet: bool,
) -> Result<()> {
    let (_, mut doc) = read_doc(file)?;

    // 1. Remove the heading from its current position.
    let src_indices = orgpath::resolve(&doc, path)?;
    let mut heading = {
        let (list, idx) = doc.parent_list_mut(&src_indices);
        list.remove(idx)
    };

    // 2. Determine new level and insert under the destination.
    if under == "/" {
        let delta = 1i32 - heading.level as i32;
        heading.shift_level(delta);
        let pos = position
            .map(|p| p.saturating_sub(1))
            .unwrap_or(doc.headings.len());
        let pos = pos.min(doc.headings.len());
        doc.headings.insert(pos, heading);
        eprintln!("moved to {}", Heading::format_addr(&[pos]));
    } else {
        let dst_indices = orgpath::resolve(&doc, under)?;
        let new_level = doc.heading_at(&dst_indices).level + 1;
        let delta = new_level as i32 - heading.level as i32;
        heading.shift_level(delta);

        let parent = doc.heading_at_mut(&dst_indices);
        let pos = position
            .map(|p| p.saturating_sub(1))
            .unwrap_or(parent.children.len());
        let pos = pos.min(parent.children.len());
        parent.children.insert(pos, heading);

        let mut new_addr = dst_indices;
        new_addr.push(pos);
        eprintln!("moved to {}", Heading::format_addr(&new_addr));
    }

    emit(file, &doc, in_place, quiet)
}

fn cmd_edit(
    file: &PathBuf,
    path: &str,
    title: Option<String>,
    priority: Option<String>,
    body: Option<String>,
    append: Option<String>,
    in_place: bool,
    quiet: bool,
) -> Result<()> {
    let (_, mut doc) = read_doc(file)?;
    let indices = orgpath::resolve(&doc, path)?;
    let h = doc.heading_at_mut(&indices);

    if let Some(t) = title {
        h.title = t;
    }
    if let Some(p) = priority {
        h.priority = if p.is_empty() {
            None
        } else {
            let ch = p.chars().next().unwrap().to_ascii_uppercase();
            Some(ch)
        };
    }
    if let Some(b) = body {
        h.body = if b.is_empty() {
            String::new()
        } else if b.ends_with('\n') {
            b
        } else {
            format!("{}\n", b)
        };
    }
    if let Some(a) = append {
        if !h.body.ends_with('\n') && !h.body.is_empty() {
            h.body.push('\n');
        }
        h.body.push_str(&a);
        if !h.body.ends_with('\n') {
            h.body.push('\n');
        }
    }

    emit(file, &doc, in_place, quiet)
}

fn cmd_prop(
    file: &PathBuf,
    path: &str,
    key: &str,
    value: Option<String>,
    delete: bool,
    in_place: bool,
    quiet: bool,
) -> Result<()> {
    let (_, mut doc) = read_doc(file)?;
    let indices = orgpath::resolve(&doc, path)?;

    if delete {
        let h = doc.heading_at_mut(&indices);
        let before = h.properties.len();
        h.properties.retain(|(k, _)| !k.eq_ignore_ascii_case(key));
        if h.properties.len() == before {
            bail!("property {:?} not found", key);
        }
        eprintln!("deleted property {:?}", key);
        return emit(file, &doc, in_place, quiet);
    }

    if let Some(val) = value {
        // Set
        let h = doc.heading_at_mut(&indices);
        if let Some(entry) = h.properties.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case(key)) {
            entry.1 = val;
        } else {
            h.properties.push((key.to_string(), val));
        }
        return emit(file, &doc, in_place, quiet);
    }

    // Get
    let h = doc.heading_at(&indices);
    if let Some((_, v)) = h.properties.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)) {
        println!("{}", v);
    } else {
        bail!("property {:?} not found on {}", key, Heading::format_addr(&indices));
    }
    Ok(())
}

fn cmd_tag(
    file: &PathBuf,
    path: &str,
    add: Vec<String>,
    remove: Vec<String>,
    set: Option<String>,
    clear: bool,
    in_place: bool,
    quiet: bool,
) -> Result<()> {
    let (_, mut doc) = read_doc(file)?;
    let indices = orgpath::resolve(&doc, path)?;

    let is_mutation = clear || set.is_some() || !add.is_empty() || !remove.is_empty();

    if !is_mutation {
        // Just display current tags
        let h = doc.heading_at(&indices);
        if h.tags.is_empty() {
            println!("(no tags)");
        } else {
            println!(":{}:", h.tags.join(":"));
        }
        return Ok(());
    }

    let h = doc.heading_at_mut(&indices);

    if clear {
        h.tags.clear();
    }
    if let Some(s) = set {
        h.tags = s
            .split(':')
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
            .collect();
    }
    for t in &add {
        if !h.tags.iter().any(|existing| existing.eq_ignore_ascii_case(t)) {
            h.tags.push(t.clone());
        }
    }
    for t in &remove {
        h.tags.retain(|existing| !existing.eq_ignore_ascii_case(t));
    }

    emit(file, &doc, in_place, quiet)
}

fn cmd_promote(file: &PathBuf, path: &str, delta: i32, in_place: bool, quiet: bool) -> Result<()> {
    let (_, mut doc) = read_doc(file)?;
    let indices = orgpath::resolve(&doc, path)?;

    let h = doc.heading_at_mut(&indices);
    if delta < 0 && h.level == 1 {
        bail!("cannot promote: already at top level");
    }
    h.shift_level(delta);

    emit(file, &doc, in_place, quiet)
}
