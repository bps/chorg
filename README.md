# chorg

Structured org-mode document editor for humans and LLM agents.

`chorg` parses org files into a heading tree, lets you address any heading by
title path or positional index, perform structural operations, and write the
result back — either to stdout or in-place. Output is human-readable by
default; pass `--json` for machine-consumable JSON.

## Install

```bash
cargo install --path .
```

## Quick start

```bash
# View the outline with positional addresses
chorg show file.org

# View a subtree with body content
chorg show file.org -p "Projects" --content

# JSON output (great for piping to jq or feeding to an LLM)
chorg show file.org --json

# Search by keyword, tag, title, body, or property
chorg find file.org --keyword TODO
chorg find file.org --tag work --title "redesign"
chorg find file.org --tag work --tag urgent         # multiple tags (AND)
chorg find file.org --body "authentication"
chorg find file.org --property EFFORT=8h
chorg find file.org --under "Projects" --keyword TODO
chorg find file.org --no-keyword DONE               # negation
chorg find file.org --no-tag archive
chorg find file.org --max-level 1                   # top-level only
chorg find file.org --scheduled                     # has SCHEDULED timestamp
chorg find file.org --deadline                      # has DEADLINE timestamp

# Change TODO state
chorg todo file.org -p "Projects/Website redesign" DONE -i

# Insert a new heading under a parent
chorg insert file.org -p "Projects" --title "Mobile app" --keyword TODO --tags "work:mobile" -i

# Delete a heading and its subtree
chorg delete file.org -p "Inbox/Old item" -i

# Move (refile) a heading under a new parent
chorg move file.org -p "Inbox/Review PR" --under "Projects" -i

# Edit heading fields
chorg edit file.org -p "#3.1" --title "New title" --priority A -i
chorg edit file.org -p "Projects/Docs" --append "Added a paragraph." -i

# Get / set / delete properties
chorg prop file.org -p "Projects/Website" EFFORT          # get
chorg prop file.org -p "Projects/Website" EFFORT 4h -i    # set
chorg prop file.org -p "Projects/Website" EFFORT --delete -i

# Modify tags
chorg tag file.org -p "Projects/Website" --add urgent -i
chorg tag file.org -p "Projects/Website" --remove old -i
chorg tag file.org -p "Projects/Website" --set "work:design" -i
chorg tag file.org -p "Projects/Website" --clear -i

# Promote / demote
chorg promote file.org -p "#3.1" -i
chorg demote file.org -p "#2" -i
```

## Addressing headings

Every heading gets a *positional address* shown in `show` output:

```
#1       * TODO [#A] Buy groceries                     :shopping:
#1.1       ** Milk
#1.2       ** Eggs
#2       * Projects
#2.1       ** TODO Website redesign                    :work:
#2.2       ** WAITING Database migration               :work:infra:
```

The `-p` / `--path` flag accepts two addressing styles, which can be mixed
freely:

| Style | Example | Meaning |
|-------|---------|---------|
| Title path | `"Projects/Website redesign"` | Match by heading title (case-insensitive, substring fallback) |
| Positional | `"#2/#1"` | 1-based index at each level |
| Mixed | `"Projects/#2"` | Title for first level, position for second |

If a title segment matches multiple siblings, `chorg` reports an error and
suggests positional addresses to disambiguate.

Use `"/"` as the path for top-level operations (e.g. inserting a new
top-level heading).

## Output modes

- **Default** — compact outline with positional addresses, designed for
  terminal use and easy scanning by LLMs.
- **`--content`** — also shows body text, indented with `|` markers.
- **`--json`** — full structured JSON with addresses, keywords, tags,
  properties, body, and nested children.
- **`-d N` / `--depth N`** — limit display depth.

## In-place editing

Mutation commands (`todo`, `insert`, `delete`, `move`, `edit`, `prop`, `tag`,
`promote`, `demote`) print the modified document to stdout by default.
Pass `-i` / `--in-place` to write changes back to the file.

## Design notes

- *Round-trip safe* — parse → write without modifications reproduces the
  original file byte-for-byte.
- *Structure-aware* — operations work on the heading tree, so you can't
  accidentally break the org hierarchy.
- *LLM-friendly* — the `show` output gives an LLM everything it needs (the
  outline with addresses) to construct follow-up commands. JSON mode gives
  full structured access.
- *No dependencies on Emacs* — pure Rust, no external tools needed.

## Supported org-mode features

- Headlines at any nesting depth
- TODO keywords (parsed from `#+TODO:` or defaults: TODO, NEXT, WAITING,
  HOLD, DONE, CANCELLED)
- Priority cookies (`[#A]`, `[#B]`, etc.)
- Tags (`:tag1:tag2:`)
- Property drawers (`:PROPERTIES:` … `:END:`)
- Planning lines (SCHEDULED, DEADLINE, CLOSED)
- Body text (preserved verbatim)
- Preamble (file-level settings and text before the first heading)
