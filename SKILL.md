---
name: chorg
description: Make structured changes to org-mode files — change TODO state, insert/delete/move headings, edit fields, search. Use when asked to modify .org files.
compatibility: Requires the chorg binary on PATH
---

## What this tool does

`chorg` reads an org file, parses it into a heading tree, and performs a
single structural operation — change a TODO state, insert or delete a
heading, move it, edit its title/body/tags/properties, etc. It writes
valid org back out, either to stdout or in-place with `-i`.

Every heading gets a *positional address* like `#1`, `#3.2`. Use
`chorg show` first to see the outline and addresses, then reference those
addresses in subsequent commands.

## Workflow

Always follow this pattern:

1. **Read** — run `chorg show <file>` to see the outline with addresses.
2. **Address** — identify the heading you need using its address or title path.
3. **Act** — run the mutation command with `-i` to apply the change.
4. **Verify** — run `chorg show` again if needed.

## Addressing headings

The `-p` flag accepts heading paths. Two styles, freely mixed:

| Style | Example | Meaning |
|-------|---------|---------|
| Title | `"Projects/Website"` | Case-insensitive match, substring fallback |
| Positional | `"#3/#1"` | 1-based index at each level |

Use `"/"` for top-level operations (e.g. inserting a root heading).

If a title matches multiple siblings, chorg errors and suggests positional
addresses. Use `#N` to disambiguate.

## Command reference

### See the outline

```bash
chorg show FILE                        # full outline
chorg show FILE -p "Projects"          # subtree only
chorg show FILE -d 1                   # limit depth
chorg show FILE --content              # include body text
chorg show FILE --json                 # structured JSON output
```

### Search

```bash
chorg find FILE --keyword TODO
chorg find FILE --tag work
chorg find FILE --tag work --tag urgent             # require ALL tags
chorg find FILE --title "redesign"
chorg find FILE --body "authentication"             # search body text
chorg find FILE --property EFFORT
chorg find FILE --property EFFORT=8h                # key=value match
chorg find FILE --keyword TODO --tag work           # combine filters (AND)
chorg find FILE --under "Projects" --keyword TODO   # search only under a heading
chorg find FILE --no-keyword DONE                   # exclude by keyword
chorg find FILE --no-tag archive                    # exclude by tag
chorg find FILE --max-level 1                       # top-level headings only
chorg find FILE --min-level 2 --max-level 3         # level range
chorg find FILE --scheduled                         # has SCHEDULED timestamp
chorg find FILE --deadline                          # has DEADLINE timestamp
chorg find FILE --json                              # JSON output
```

### Change TODO state

```bash
chorg todo FILE -p "#1" DONE -i        # set keyword
chorg todo FILE -p "Buy groceries" "" -i  # clear keyword
```

### Insert a heading

```bash
chorg insert FILE -p "Projects" --title "New project" -i
chorg insert FILE -p "Projects" --title "Urgent" --keyword TODO --priority A --tags "work:urgent" -i
chorg insert FILE -p "/" --title "Top-level" -i          # insert at root
chorg insert FILE -p "Projects" --title "First" -n 1 -i  # insert at position
chorg insert FILE -p "#3" --title "Task" --body "Details here" -i
```

### Delete a heading (and its subtree)

```bash
chorg delete FILE -p "Inbox/Old item" -i
chorg delete FILE -p "#4.2" -i
```

### Move (refile) a heading

```bash
chorg move FILE -p "Inbox/Review PR" --under "Projects" -i
chorg move FILE -p "#4.1" --under "#3" -i
chorg move FILE -p "Projects/Alpha" --under "/" -i        # move to top level
chorg move FILE -p "#1" --under "Archive" -n 1 -i        # move to first position
```

Levels adjust automatically. Moving a level-2 heading under a level-3
parent makes it level 4; its children shift accordingly.

### Edit heading fields

```bash
chorg edit FILE -p "#1" --title "New title" -i
chorg edit FILE -p "#1" --priority A -i      # set priority
chorg edit FILE -p "#1" --priority "" -i     # clear priority
chorg edit FILE -p "#1" --body "Replaced body" -i
chorg edit FILE -p "#1" --append "Extra line" -i
```

### Properties

```bash
chorg prop FILE -p "#1" EFFORT              # get value
chorg prop FILE -p "#1" EFFORT 4h -i        # set value
chorg prop FILE -p "#1" EFFORT --delete -i  # delete key
```

### Tags

```bash
chorg tag FILE -p "#1"                       # display tags
chorg tag FILE -p "#1" --add urgent -i
chorg tag FILE -p "#1" --remove old -i
chorg tag FILE -p "#1" --set "a:b:c" -i      # replace all
chorg tag FILE -p "#1" --clear -i
```

### Promote / demote

```bash
chorg promote FILE -p "#3.1" -i   # decrease level by 1
chorg demote FILE -p "#2" -i     # increase level by 1
```

## Worked examples

### Mark a task done

```
$ chorg show todo.org
#1       TODO [#A] Buy groceries :shopping:
#1.1     Milk
#1.2     Eggs
#2       DONE Fix bug :work:
#3       Projects
#3.1     TODO Website redesign :work:
#3.2     NEXT Write docs :work:

$ chorg todo todo.org -p "#3.1" DONE -i

$ chorg show todo.org
#1       TODO [#A] Buy groceries :shopping:
#1.1     Milk
#1.2     Eggs
#2       DONE Fix bug :work:
#3       Projects
#3.1     DONE Website redesign :work:
#3.2     NEXT Write docs :work:
```

### Refile an inbox item

```
$ chorg show tasks.org
#1       Inbox
#1.1     Review security audit
#1.2     TODO Draft blog post
#2       Projects
#2.1     TODO Website :work:
#3       Archive

$ chorg move tasks.org -p "#1.1" --under "#2" -i

$ chorg show tasks.org
#1       Inbox
#1.1     TODO Draft blog post
#2       Projects
#2.1     TODO Website :work:
#2.2     Review security audit
#3       Archive
```

### Add a project with subtasks

```
$ chorg insert tasks.org -p "Projects" --title "API redesign" --keyword TODO --tags work -i
$ chorg insert tasks.org -p "Projects/API redesign" --title "Design doc" --keyword TODO -i
$ chorg insert tasks.org -p "Projects/API redesign" --title "Prototype" --keyword TODO -i

$ chorg show tasks.org -p "Projects/API redesign"
#2.3     TODO API redesign :work:
#2.3.1   TODO Design doc
#2.3.2   TODO Prototype
```

### Find and batch-process

```
$ chorg find tasks.org --keyword TODO --tag work
#2.1     TODO Website :work:
#2.3     TODO API redesign :work:

$ chorg todo tasks.org -p "#2.1" DONE -i
$ chorg todo tasks.org -p "#2.3" WAITING -i
```

## Key behaviors

- **Stdout by default** — mutation commands print the modified file to
  stdout. Pass `-i` to write in place.
- **Round-trip safe** — unmodified headings are preserved byte-for-byte.
- **Addresses shift** — after inserting or deleting, positional addresses
  change. Re-run `show` if you need to issue further commands.
- **Substring matching** — title paths match case-insensitively, with
  exact match preferred over substring. Ambiguity produces an error
  listing the candidates with their positional addresses.
