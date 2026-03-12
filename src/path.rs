use crate::model::{Heading, OrgDoc};
use anyhow::{bail, Context, Result};

/// Resolve a human-friendly path string into index coordinates.
///
/// Path syntax:
///   - Segments separated by `/`
///   - `#N` — positional (1-based) at that level
///   - Bare text — case-insensitive exact match, then substring fallback
///   - Multiple matches → error with suggestions
///   - `#1.2.3` or `1.2.3` — dot-separated numeric address (as output by show/find)
///
/// Examples: `"Projects/Website"`, `"#1/#2"`, `"#3/Website"`, `"#1.2"`, `"1.2"`
pub fn resolve(doc: &OrgDoc, path: &str) -> Result<Vec<usize>> {
    // Check if this is a dot-separated numeric address like "#1.2" or "1.2"
    let trimmed = path.trim();
    let addr_str = trimmed.strip_prefix('#').unwrap_or(trimmed);
    if is_numeric_addr(addr_str) {
        return resolve_numeric_addr(doc, addr_str);
    }

    let segments: Vec<&str> = path.split('/').collect();
    let mut indices: Vec<usize> = Vec::new();
    let mut current_list: &[Heading] = &doc.headings;

    for (depth, seg) in segments.iter().enumerate() {
        let seg = seg.trim();
        if seg.is_empty() {
            bail!("empty path segment at position {}", depth + 1);
        }
        let idx = resolve_segment(current_list, seg, &indices)
            .with_context(|| format!("resolving segment {:?} at depth {}", seg, depth + 1))?;
        indices.push(idx);
        current_list = &current_list[idx].children;
    }

    Ok(indices)
}

/// Check if a string is a dot-separated numeric address like "1.2.3".
fn is_numeric_addr(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    s.split('.').all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
}

/// Resolve a dot-separated numeric address like "1.2.3" into 0-based index coordinates.
fn resolve_numeric_addr(doc: &OrgDoc, addr: &str) -> Result<Vec<usize>> {
    let parts: Vec<&str> = addr.split('.').collect();
    let mut indices: Vec<usize> = Vec::new();
    let mut current_list: &[Heading] = &doc.headings;

    for (depth, part) in parts.iter().enumerate() {
        let n: usize = part
            .parse()
            .with_context(|| format!("invalid numeric address segment {:?}", part))?;
        if n == 0 || n > current_list.len() {
            bail!(
                "positional index {} out of range (1..{}) at depth {}",
                n,
                current_list.len(),
                depth + 1
            );
        }
        let idx = n - 1;
        indices.push(idx);
        current_list = &current_list[idx].children;
    }

    Ok(indices)
}

fn resolve_segment(headings: &[Heading], segment: &str, parent_addr: &[usize]) -> Result<usize> {
    if headings.is_empty() {
        bail!("no headings at this level");
    }

    // Positional: #N (1-based)
    if let Some(num_str) = segment.strip_prefix('#') {
        let n: usize = num_str
            .parse()
            .with_context(|| format!("invalid positional index {:?}", segment))?;
        if n == 0 || n > headings.len() {
            bail!(
                "positional index {} out of range (1..{})",
                n,
                headings.len()
            );
        }
        return Ok(n - 1);
    }

    // Exact match (case-insensitive)
    let exact: Vec<usize> = headings
        .iter()
        .enumerate()
        .filter(|(_, h)| h.title.eq_ignore_ascii_case(segment))
        .map(|(i, _)| i)
        .collect();

    if exact.len() == 1 {
        return Ok(exact[0]);
    }

    // Substring match (case-insensitive) as fallback
    let seg_lower = segment.to_lowercase();
    let substr: Vec<usize> = headings
        .iter()
        .enumerate()
        .filter(|(_, h)| h.title.to_lowercase().contains(&seg_lower))
        .map(|(i, _)| i)
        .collect();

    if substr.len() == 1 {
        return Ok(substr[0]);
    }

    // Build error message
    let matches = if !exact.is_empty() { &exact } else { &substr };
    if matches.is_empty() {
        let available: Vec<String> = headings.iter().map(|h| h.title.clone()).collect();
        bail!(
            "no heading matches {:?}.\nAvailable at this level: {}",
            segment,
            available.join(", ")
        );
    }

    // Ambiguous
    let suggestions: Vec<String> = matches
        .iter()
        .map(|&i| {
            let mut addr = parent_addr.to_vec();
            addr.push(i);
            format!(
                "  {} — {:?}",
                Heading::format_addr(&addr),
                headings[i].title
            )
        })
        .collect();
    bail!(
        "ambiguous path {:?} matches {} headings. Use a positional address to disambiguate:\n{}",
        segment,
        matches.len(),
        suggestions.join("\n")
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    fn sample_doc() -> OrgDoc {
        parse(
            "\
* Projects
** Website
** Database
** Website Beta
* Inbox
** Item one
** Item two
** Item three
* Archive
",
        )
    }

    // -----------------------------------------------------------------------
    // Title-based resolution
    // -----------------------------------------------------------------------

    #[test]
    fn resolve_by_title_top_level() {
        let doc = sample_doc();
        assert_eq!(resolve(&doc, "Projects").unwrap(), vec![0]);
        assert_eq!(resolve(&doc, "Inbox").unwrap(), vec![1]);
        assert_eq!(resolve(&doc, "Archive").unwrap(), vec![2]);
    }

    #[test]
    fn resolve_by_title_nested() {
        let doc = sample_doc();
        assert_eq!(resolve(&doc, "Projects/Database").unwrap(), vec![0, 1]);
        assert_eq!(resolve(&doc, "Inbox/Item two").unwrap(), vec![1, 1]);
    }

    #[test]
    fn resolve_case_insensitive() {
        let doc = sample_doc();
        assert_eq!(resolve(&doc, "projects").unwrap(), vec![0]);
        assert_eq!(resolve(&doc, "PROJECTS/DATABASE").unwrap(), vec![0, 1]);
        assert_eq!(resolve(&doc, "inbox/ITEM ONE").unwrap(), vec![1, 0]);
        // Mixed case
        assert_eq!(resolve(&doc, "pRoJeCtS").unwrap(), vec![0]);
        assert_eq!(resolve(&doc, "InBoX/iTeM tWo").unwrap(), vec![1, 1]);
    }

    #[test]
    fn resolve_case_insensitive_substring() {
        let doc = sample_doc();
        // Substring match should also be case-insensitive
        assert_eq!(resolve(&doc, "Projects/DATA").unwrap(), vec![0, 1]);
        assert_eq!(resolve(&doc, "Projects/data").unwrap(), vec![0, 1]);
    }

    #[test]
    fn resolve_substring_unique() {
        let doc = sample_doc();
        // "Data" uniquely matches "Database"
        assert_eq!(resolve(&doc, "Projects/Data").unwrap(), vec![0, 1]);
    }

    #[test]
    fn resolve_exact_match_preferred_over_substring() {
        let doc = sample_doc();
        // "Website" matches exactly, even though "Website Beta" also contains it
        assert_eq!(resolve(&doc, "Projects/Website").unwrap(), vec![0, 0]);
    }

    // -----------------------------------------------------------------------
    // Positional resolution
    // -----------------------------------------------------------------------

    #[test]
    fn resolve_positional_single() {
        let doc = sample_doc();
        assert_eq!(resolve(&doc, "#1").unwrap(), vec![0]);
        assert_eq!(resolve(&doc, "#2").unwrap(), vec![1]);
        assert_eq!(resolve(&doc, "#3").unwrap(), vec![2]);
    }

    #[test]
    fn resolve_positional_nested() {
        let doc = sample_doc();
        assert_eq!(resolve(&doc, "#1/#1").unwrap(), vec![0, 0]);
        assert_eq!(resolve(&doc, "#1/#2").unwrap(), vec![0, 1]);
        assert_eq!(resolve(&doc, "#2/#3").unwrap(), vec![1, 2]);
    }

    #[test]
    fn resolve_positional_zero_error() {
        let doc = sample_doc();
        let err = resolve(&doc, "#0").unwrap_err();
        assert!(format!("{:#}", err).contains("out of range"));
    }

    #[test]
    fn resolve_positional_overflow_error() {
        let doc = sample_doc();
        let err = resolve(&doc, "#99").unwrap_err();
        assert!(format!("{:#}", err).contains("out of range"));
    }

    #[test]
    fn resolve_positional_not_a_number() {
        let doc = sample_doc();
        let err = resolve(&doc, "#abc").unwrap_err();
        assert!(format!("{:#}", err).contains("invalid"));
    }

    // -----------------------------------------------------------------------
    // Mixed resolution
    // -----------------------------------------------------------------------

    #[test]
    fn resolve_mixed_title_then_positional() {
        let doc = sample_doc();
        assert_eq!(resolve(&doc, "Projects/#2").unwrap(), vec![0, 1]);
    }

    #[test]
    fn resolve_mixed_positional_then_title() {
        let doc = sample_doc();
        assert_eq!(resolve(&doc, "#2/Item one").unwrap(), vec![1, 0]);
    }

    // -----------------------------------------------------------------------
    // Error cases
    // -----------------------------------------------------------------------

    #[test]
    fn resolve_empty_doc() {
        let doc = parse("");
        let err = resolve(&doc, "Anything");
        assert!(err.is_err());
    }

    #[test]
    fn resolve_not_found() {
        let doc = sample_doc();
        let err = resolve(&doc, "Nonexistent").unwrap_err();
        let msg = format!("{:#}", err);
        assert!(msg.contains("no heading matches"), "error was: {}", msg);
        // Should list available headings
        assert!(msg.contains("Projects"), "error was: {}", msg);
    }

    #[test]
    fn resolve_not_found_nested() {
        let doc = sample_doc();
        let err = resolve(&doc, "Projects/Nonexistent").unwrap_err();
        let msg = format!("{:#}", err);
        assert!(msg.contains("no heading matches"), "error was: {}", msg);
    }

    #[test]
    fn resolve_path_too_deep() {
        let doc = sample_doc();
        // "Website" is a leaf — can't go deeper
        let err = resolve(&doc, "Projects/Website/Sub").unwrap_err();
        let msg = format!("{:#}", err);
        assert!(msg.contains("no headings at this level"), "error was: {}", msg);
    }

    #[test]
    fn resolve_ambiguous_substring() {
        let doc = sample_doc();
        // "Item" matches Item one, Item two, Item three
        let err = resolve(&doc, "Inbox/Item").unwrap_err();
        let msg = format!("{:#}", err);
        assert!(msg.contains("ambiguous"), "error was: {}", msg);
        // Should suggest positional addresses
        assert!(msg.contains("#"), "error was: {}", msg);
        // Every candidate should be listed
        assert!(msg.contains("Item one"), "error was: {}", msg);
        assert!(msg.contains("Item two"), "error was: {}", msg);
        assert!(msg.contains("Item three"), "error was: {}", msg);
        // Should show the #N.M — "Title" format
        assert!(msg.contains("#2.1"), "error was: {}", msg);
        assert!(msg.contains("#2.2"), "error was: {}", msg);
        assert!(msg.contains("#2.3"), "error was: {}", msg);
    }

    #[test]
    fn resolve_ambiguous_exact() {
        // Two headings with the exact same title
        let doc = parse("* Dup\n* Dup\n");
        let err = resolve(&doc, "Dup").unwrap_err();
        let msg = format!("{:#}", err);
        assert!(msg.contains("ambiguous"), "error was: {}", msg);
    }

    #[test]
    fn resolve_empty_segment() {
        let doc = sample_doc();
        let err = resolve(&doc, "Projects//Website").unwrap_err();
        let msg = format!("{:#}", err);
        assert!(msg.contains("empty path segment"), "error was: {}", msg);
    }

    // -----------------------------------------------------------------------
    // Deep paths
    // -----------------------------------------------------------------------

    #[test]
    fn resolve_deep_path() {
        let doc = parse("* A\n** B\n*** C\n**** D\n***** E\n");
        assert_eq!(resolve(&doc, "A/B/C/D/E").unwrap(), vec![0, 0, 0, 0, 0]);
        assert_eq!(resolve(&doc, "#1/#1/#1/#1/#1").unwrap(), vec![0, 0, 0, 0, 0]);
    }

    // -----------------------------------------------------------------------
    // Dot-separated numeric address resolution
    // -----------------------------------------------------------------------

    #[test]
    fn resolve_dot_addr_single() {
        let doc = sample_doc();
        assert_eq!(resolve(&doc, "#1").unwrap(), vec![0]);
        assert_eq!(resolve(&doc, "1").unwrap(), vec![0]);
        assert_eq!(resolve(&doc, "#3").unwrap(), vec![2]);
        assert_eq!(resolve(&doc, "3").unwrap(), vec![2]);
    }

    #[test]
    fn resolve_dot_addr_nested() {
        let doc = sample_doc();
        assert_eq!(resolve(&doc, "#1.1").unwrap(), vec![0, 0]);
        assert_eq!(resolve(&doc, "1.1").unwrap(), vec![0, 0]);
        assert_eq!(resolve(&doc, "#1.2").unwrap(), vec![0, 1]);
        assert_eq!(resolve(&doc, "1.2").unwrap(), vec![0, 1]);
        assert_eq!(resolve(&doc, "#2.3").unwrap(), vec![1, 2]);
        assert_eq!(resolve(&doc, "2.3").unwrap(), vec![1, 2]);
    }

    #[test]
    fn resolve_dot_addr_with_whitespace() {
        let doc = sample_doc();
        assert_eq!(resolve(&doc, " #1.2 ").unwrap(), vec![0, 1]);
        assert_eq!(resolve(&doc, " 1.2 ").unwrap(), vec![0, 1]);
    }

    #[test]
    fn resolve_dot_addr_zero_error() {
        let doc = sample_doc();
        let err = resolve(&doc, "#0.1").unwrap_err();
        assert!(format!("{:#}", err).contains("out of range"));
    }

    #[test]
    fn resolve_dot_addr_overflow_error() {
        let doc = sample_doc();
        let err = resolve(&doc, "#99.1").unwrap_err();
        assert!(format!("{:#}", err).contains("out of range"));
    }

    #[test]
    fn resolve_dot_addr_deep() {
        let doc = parse("* A\n** B\n*** C\n**** D\n***** E\n");
        assert_eq!(resolve(&doc, "#1.1.1.1.1").unwrap(), vec![0, 0, 0, 0, 0]);
        assert_eq!(resolve(&doc, "1.1.1.1.1").unwrap(), vec![0, 0, 0, 0, 0]);
    }

    // -----------------------------------------------------------------------
    // Whitespace handling
    // -----------------------------------------------------------------------

    #[test]
    fn resolve_segment_whitespace_trimmed() {
        let doc = sample_doc();
        // Segments are trimmed
        assert_eq!(resolve(&doc, " Projects / Website ").unwrap(), vec![0, 0]);
    }
}
