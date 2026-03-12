---
name: chorg
description: Structured org-mode file editing via the chorg CLI. Use when you need to read, query, or mutate org headings, TODO states, tags, properties, or hierarchy — instead of raw text editing which can break org structure.
---

# chorg

A structured org-mode editor. Prefer `chorg` over raw text editing (`edit`) whenever you need to manipulate org heading structure, TODO keywords, tags, or properties.

## Core workflow

1. **Orient** — run `chorg show <file>` to see the outline and discover heading paths
2. **Find** — use `chorg find` to locate headings by keyword, tag, title, or property
3. **Mutate** — use the appropriate command with `-i` to edit in place

## Path addressing

`--path` accepts either *title-based paths* or *numeric addresses*:

```bash
chorg show file.org -p "Project Alpha/Task two"     # title path
chorg show file.org -p "#1.2"                        # numeric address
chorg show file.org -p "1.2"                         # numeric (# optional)
```

Use `/` alone to refer to the document root (e.g., inserting a top-level heading).

Numeric addresses from `show`/`find` output can be passed directly to mutation commands — no need to reconstruct title paths. JSON output also includes a `"path"` field with the full title path.

## Commands

### Reading

```bash
chorg show file.org                         # full outline
chorg show file.org -p "Section" -d 1       # one level deep
chorg show file.org --content               # include body text
chorg show file.org --json                  # structured output

chorg find file.org --keyword TODO          # all TODOs
chorg find file.org --tag urgent            # by tag
chorg find file.org --title "meeting"       # substring match
chorg find file.org --property "ID=abc"     # by property
chorg find file.org --under "Projects"      # scoped search
chorg find file.org --keyword TODO --json   # JSON output
```

### Mutating (always pass `-i` to write changes back)

```bash
# TODO state
chorg todo -p "Section/Task" file.org DONE -i
chorg todo -p "#1.2" file.org DONE -i              # numeric address
chorg todo -p "#1.2" -p "#3.1" file.org DONE -i    # batch: multiple headings
chorg todo -p "Section/Task" file.org "" -i         # clear keyword

# Insert heading
chorg insert -p "Parent" --title "New item" --keyword TODO file.org -i
chorg insert -p "/" --title "Top-level" file.org -i   # at root

# Edit fields
chorg edit -p "Section/Task" --title "Renamed" file.org -i
chorg edit -p "Section/Task" --body "New body" file.org -i
chorg edit -p "Section/Task" --append "Added text" file.org -i

# Tags
chorg tag -p "Section/Task" --add urgent file.org -i
chorg tag -p "Section/Task" --remove old file.org -i
chorg tag -p "Section/Task" --set "a:b:c" file.org -i

# Properties
chorg prop -p "Section/Task" file.org KEY VALUE -i   # set
chorg prop -p "Section/Task" file.org KEY             # get
chorg prop -p "Section/Task" file.org KEY --delete -i # delete

# Structure
chorg move -p "Section/Task" --under "Other Section" file.org -i
chorg delete -p "Section/Task" file.org -i
chorg promote -p "Section/Task" file.org -i
chorg demote -p "Section/Task" file.org -i
```

## Tips

- **Omitting `-i`** prints the result to stdout without modifying the file — useful for dry runs. Add `-q` to suppress the full output and only see the confirmation on stderr.
- **Numeric addresses** (`#1.2`) from `show`/`find` output work directly in `--path` — prefer these over title paths when you already have them.
- **Batch mutations** — pass multiple `-p` flags to apply the same operation to several headings in one invocation.
- **`find` exits 1** when nothing matches — use this for conditional logic.
- **`find --json`** and **`show --json`** include both `"addr"` and `"path"` fields, so you can use either addressing style.
- **`show --content`** includes body text; without it you only see the heading tree.
- **`insert --position N`** and **`move --position N`** control sibling order (1-based; default: append).

## Preferred agent workflow

```bash
# Find headings, then act on them using the addresses directly
chorg find file.org --keyword TODO --json   # → get addr/path values
chorg todo -p "#1.2" -p "#3.1" file.org DONE -i
```
