use serde::Serialize;

/// Top-level org document.
#[derive(Debug, Clone, Serialize)]
pub struct OrgDoc {
    /// Text before the first heading (settings, comments, etc.).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub preamble: String,
    /// Top-level headings (children are nested).
    pub headings: Vec<Heading>,
    /// Parsed TODO/DONE keyword sets.
    #[serde(skip)]
    pub settings: Settings,
}

/// A single org heading and its subtree.
#[derive(Debug, Clone, Serialize)]
pub struct Heading {
    /// Nesting depth (number of stars: 1, 2, 3, …).
    pub level: usize,
    /// TODO keyword (TODO, DONE, WAITING, etc.) if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyword: Option<String>,
    /// Priority cookie (A, B, C, …) if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<char>,
    /// Heading title text.
    pub title: String,
    /// Tags on this heading.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Raw planning line (SCHEDULED / DEADLINE / CLOSED).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub planning: Option<String>,
    /// Property drawer key-value pairs (insertion order preserved).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub properties: Vec<(String, String)>,
    /// Body text (between property drawer and first child heading).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub body: String,
    /// Child headings.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Heading>,
}

/// Configurable keyword sets parsed from #+TODO: lines.
#[derive(Debug, Clone)]
pub struct Settings {
    pub todo_keywords: Vec<String>,
    pub done_keywords: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            todo_keywords: vec!["TODO".into(), "NEXT".into(), "WAITING".into(), "HOLD".into()],
            done_keywords: vec!["DONE".into(), "CANCELLED".into()],
        }
    }
}

impl Settings {
    /// Return true if `word` is any recognised keyword (active or done).
    pub fn is_keyword(&self, word: &str) -> bool {
        self.todo_keywords.iter().any(|k| k == word)
            || self.done_keywords.iter().any(|k| k == word)
    }

    /// Return true if `word` is a *done* keyword.
    pub fn is_done(&self, word: &str) -> bool {
        self.done_keywords.iter().any(|k| k == word)
    }

    /// All keywords (active first, then done).
    pub fn all_keywords(&self) -> Vec<&str> {
        self.todo_keywords
            .iter()
            .chain(self.done_keywords.iter())
            .map(|s| s.as_str())
            .collect()
    }
}

impl OrgDoc {
    /// Iterate all headings depth-first, yielding (positional address, &Heading).
    pub fn walk(&self) -> Vec<(Vec<usize>, &Heading)> {
        let mut out = Vec::new();
        for (i, h) in self.headings.iter().enumerate() {
            walk_rec(h, &[i], &mut out);
        }
        out
    }

    /// Mutable access to a heading by index path.
    pub fn heading_at_mut(&mut self, indices: &[usize]) -> &mut Heading {
        assert!(!indices.is_empty());
        let mut node = &mut self.headings[indices[0]];
        for &idx in &indices[1..] {
            node = &mut node.children[idx];
        }
        node
    }

    /// Immutable access to a heading by index path.
    pub fn heading_at(&self, indices: &[usize]) -> &Heading {
        assert!(!indices.is_empty());
        let mut node = &self.headings[indices[0]];
        for &idx in &indices[1..] {
            node = &node.children[idx];
        }
        node
    }

    /// Return mutable reference to the Vec<Heading> that *contains* the
    /// heading addressed by `indices`, along with the final index.
    pub fn parent_list_mut(&mut self, indices: &[usize]) -> (&mut Vec<Heading>, usize) {
        assert!(!indices.is_empty());
        let last = *indices.last().unwrap();
        if indices.len() == 1 {
            (&mut self.headings, last)
        } else {
            let parent = self.heading_at_mut(&indices[..indices.len() - 1]);
            (&mut parent.children, last)
        }
    }
}

impl Heading {
    /// Format the positional address as `#1.2.3` (1-based).
    pub fn format_addr(indices: &[usize]) -> String {
        let parts: Vec<String> = indices.iter().map(|i| (i + 1).to_string()).collect();
        format!("#{}", parts.join("."))
    }

    /// Format the title-based path like `"Projects/Website"`.
    pub fn format_title_path(doc: &OrgDoc, indices: &[usize]) -> String {
        let mut parts = Vec::new();
        let mut node_list = &doc.headings;
        for &idx in indices {
            parts.push(node_list[idx].title.clone());
            node_list = &node_list[idx].children;
        }
        parts.join("/")
    }

    /// Shift level of this heading and all descendants by `delta`.
    pub fn shift_level(&mut self, delta: i32) {
        self.level = (self.level as i32 + delta).max(1) as usize;
        for child in &mut self.children {
            child.shift_level(delta);
        }
    }
}

fn walk_rec<'a>(h: &'a Heading, addr: &[usize], out: &mut Vec<(Vec<usize>, &'a Heading)>) {
    out.push((addr.to_vec(), h));
    for (i, child) in h.children.iter().enumerate() {
        let mut child_addr = addr.to_vec();
        child_addr.push(i);
        walk_rec(child, &child_addr, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    fn sample_doc() -> OrgDoc {
        parse(
            "\
* A
** A1
** A2
*** A2a
* B
** B1
",
        )
    }

    // -----------------------------------------------------------------------
    // Settings
    // -----------------------------------------------------------------------

    #[test]
    fn settings_default() {
        let s = Settings::default();
        assert!(s.is_keyword("TODO"));
        assert!(s.is_keyword("DONE"));
        assert!(s.is_keyword("NEXT"));
        assert!(s.is_keyword("CANCELLED"));
        assert!(!s.is_keyword("OPEN"));
    }

    #[test]
    fn settings_is_done() {
        let s = Settings::default();
        assert!(s.is_done("DONE"));
        assert!(s.is_done("CANCELLED"));
        assert!(!s.is_done("TODO"));
        assert!(!s.is_done("NEXT"));
        assert!(!s.is_done("WAITING"));
        assert!(!s.is_done("HOLD"));
    }

    #[test]
    fn settings_all_keywords() {
        let s = Settings::default();
        let all = s.all_keywords();
        assert!(all.contains(&"TODO"));
        assert!(all.contains(&"DONE"));
        // Active keywords come before done keywords
        let todo_pos = all.iter().position(|&k| k == "TODO").unwrap();
        let done_pos = all.iter().position(|&k| k == "DONE").unwrap();
        assert!(todo_pos < done_pos);
    }

    // -----------------------------------------------------------------------
    // walk()
    // -----------------------------------------------------------------------

    #[test]
    fn walk_order_depth_first() {
        let doc = sample_doc();
        let walked = doc.walk();
        let titles: Vec<&str> = walked.iter().map(|(_, h)| h.title.as_str()).collect();
        assert_eq!(titles, vec!["A", "A1", "A2", "A2a", "B", "B1"]);
    }

    #[test]
    fn walk_addresses_correct() {
        let doc = sample_doc();
        let walked = doc.walk();
        let addrs: Vec<&[usize]> = walked.iter().map(|(a, _)| a.as_slice()).collect();
        assert_eq!(
            addrs,
            vec![
                &[0][..],     // A
                &[0, 0][..],  // A1
                &[0, 1][..],  // A2
                &[0, 1, 0][..], // A2a
                &[1][..],     // B
                &[1, 0][..],  // B1
            ]
        );
    }

    #[test]
    fn walk_empty_doc() {
        let doc = parse("");
        assert!(doc.walk().is_empty());
    }

    #[test]
    fn walk_single_heading() {
        let doc = parse("* Only\n");
        let walked = doc.walk();
        assert_eq!(walked.len(), 1);
        assert_eq!(walked[0].0, vec![0]);
        assert_eq!(walked[0].1.title, "Only");
    }

    // -----------------------------------------------------------------------
    // heading_at / heading_at_mut
    // -----------------------------------------------------------------------

    #[test]
    fn heading_at_root() {
        let doc = sample_doc();
        assert_eq!(doc.heading_at(&[0]).title, "A");
        assert_eq!(doc.heading_at(&[1]).title, "B");
    }

    #[test]
    fn heading_at_nested() {
        let doc = sample_doc();
        assert_eq!(doc.heading_at(&[0, 0]).title, "A1");
        assert_eq!(doc.heading_at(&[0, 1]).title, "A2");
        assert_eq!(doc.heading_at(&[0, 1, 0]).title, "A2a");
        assert_eq!(doc.heading_at(&[1, 0]).title, "B1");
    }

    #[test]
    fn heading_at_mut_modifies() {
        let mut doc = sample_doc();
        doc.heading_at_mut(&[0, 0]).title = "Modified".into();
        assert_eq!(doc.heading_at(&[0, 0]).title, "Modified");
    }

    // -----------------------------------------------------------------------
    // parent_list_mut
    // -----------------------------------------------------------------------

    #[test]
    fn parent_list_mut_root() {
        let mut doc = sample_doc();
        let (list, idx) = doc.parent_list_mut(&[1]);
        assert_eq!(idx, 1);
        assert_eq!(list[idx].title, "B");
    }

    #[test]
    fn parent_list_mut_nested() {
        let mut doc = sample_doc();
        let (list, idx) = doc.parent_list_mut(&[0, 1]);
        assert_eq!(idx, 1);
        assert_eq!(list[idx].title, "A2");
    }

    #[test]
    fn parent_list_mut_remove() {
        let mut doc = sample_doc();
        let (list, idx) = doc.parent_list_mut(&[0, 0]);
        list.remove(idx);
        // A1 removed, A2 is now the only child of A
        assert_eq!(doc.headings[0].children.len(), 1);
        assert_eq!(doc.headings[0].children[0].title, "A2");
    }

    // -----------------------------------------------------------------------
    // format_addr
    // -----------------------------------------------------------------------

    #[test]
    fn format_addr_single() {
        assert_eq!(Heading::format_addr(&[0]), "#1");
        assert_eq!(Heading::format_addr(&[4]), "#5");
    }

    #[test]
    fn format_addr_nested() {
        assert_eq!(Heading::format_addr(&[0, 0]), "#1.1");
        assert_eq!(Heading::format_addr(&[2, 1, 0]), "#3.2.1");
    }

    // -----------------------------------------------------------------------
    // format_title_path
    // -----------------------------------------------------------------------

    #[test]
    fn format_title_path_basic() {
        let doc = sample_doc();
        assert_eq!(Heading::format_title_path(&doc, &[0]), "A");
        assert_eq!(Heading::format_title_path(&doc, &[0, 1, 0]), "A/A2/A2a");
        assert_eq!(Heading::format_title_path(&doc, &[1, 0]), "B/B1");
    }

    // -----------------------------------------------------------------------
    // shift_level
    // -----------------------------------------------------------------------

    #[test]
    fn shift_level_up() {
        let mut h = Heading {
            level: 3,
            keyword: None,
            priority: None,
            title: "Test".into(),
            tags: Vec::new(),
            planning: None,
            properties: Vec::new(),
            body: String::new(),
            children: vec![Heading {
                level: 4,
                keyword: None,
                priority: None,
                title: "Child".into(),
                tags: Vec::new(),
                planning: None,
                properties: Vec::new(),
                body: String::new(),
                children: Vec::new(),
            }],
        };
        h.shift_level(-1);
        assert_eq!(h.level, 2);
        assert_eq!(h.children[0].level, 3);
    }

    #[test]
    fn shift_level_up_large_delta() {
        let mut h = Heading {
            level: 5,
            keyword: None,
            priority: None,
            title: "Test".into(),
            tags: Vec::new(),
            planning: None,
            properties: Vec::new(),
            body: String::new(),
            children: vec![Heading {
                level: 6,
                keyword: None,
                priority: None,
                title: "Child".into(),
                tags: Vec::new(),
                planning: None,
                properties: Vec::new(),
                body: String::new(),
                children: Vec::new(),
            }],
        };
        h.shift_level(-3);
        assert_eq!(h.level, 2);
        assert_eq!(h.children[0].level, 3);
    }

    #[test]
    fn shift_level_zero_is_noop() {
        let mut h = Heading {
            level: 2,
            keyword: None,
            priority: None,
            title: "Test".into(),
            tags: Vec::new(),
            planning: None,
            properties: Vec::new(),
            body: String::new(),
            children: vec![Heading {
                level: 3,
                keyword: None,
                priority: None,
                title: "Child".into(),
                tags: Vec::new(),
                planning: None,
                properties: Vec::new(),
                body: String::new(),
                children: Vec::new(),
            }],
        };
        h.shift_level(0);
        assert_eq!(h.level, 2);
        assert_eq!(h.children[0].level, 3);
    }

    #[test]
    fn shift_level_multilevel_children() {
        let mut h = Heading {
            level: 2,
            keyword: None,
            priority: None,
            title: "Parent".into(),
            tags: Vec::new(),
            planning: None,
            properties: Vec::new(),
            body: String::new(),
            children: vec![Heading {
                level: 3,
                keyword: None,
                priority: None,
                title: "Child".into(),
                tags: Vec::new(),
                planning: None,
                properties: Vec::new(),
                body: String::new(),
                children: vec![Heading {
                    level: 4,
                    keyword: None,
                    priority: None,
                    title: "Grandchild".into(),
                    tags: Vec::new(),
                    planning: None,
                    properties: Vec::new(),
                    body: String::new(),
                    children: Vec::new(),
                }],
            }],
        };
        h.shift_level(2);
        assert_eq!(h.level, 4);
        assert_eq!(h.children[0].level, 5);
        assert_eq!(h.children[0].children[0].level, 6);
    }

    #[test]
    fn shift_level_down() {
        let mut h = Heading {
            level: 1,
            keyword: None,
            priority: None,
            title: "Test".into(),
            tags: Vec::new(),
            planning: None,
            properties: Vec::new(),
            body: String::new(),
            children: Vec::new(),
        };
        h.shift_level(2);
        assert_eq!(h.level, 3);
    }

    #[test]
    fn shift_level_clamp_at_one() {
        let mut h = Heading {
            level: 1,
            keyword: None,
            priority: None,
            title: "Test".into(),
            tags: Vec::new(),
            planning: None,
            properties: Vec::new(),
            body: String::new(),
            children: vec![Heading {
                level: 2,
                keyword: None,
                priority: None,
                title: "Child".into(),
                tags: Vec::new(),
                planning: None,
                properties: Vec::new(),
                body: String::new(),
                children: Vec::new(),
            }],
        };
        h.shift_level(-5);
        assert_eq!(h.level, 1); // clamped
        assert_eq!(h.children[0].level, 1); // also clamped
    }
}
