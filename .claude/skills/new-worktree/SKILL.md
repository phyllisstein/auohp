---
name: new-worktree
description: Create a new AUOHP worktree outside the repo (at ../auohp.worktrees/<name>) instead of Claude Code's default .claude/worktrees/ location, wired with a CLAUDE.md back-pointer to the parent repo and copied env files so it's immediately runnable. Use when the user asks to start a worktree, spike, or isolated branch for this project.
---

# New external worktree

Claude Code's built-in `EnterWorktree({name})` always creates worktrees under
`.claude/worktrees/` inside the repo. That nesting has caused confusion in
past sessions — do NOT use `EnterWorktree({name: ...})` for this project.
Instead, create the worktree manually outside the repo, then attach to it
with `EnterWorktree({path: ...})`, which accepts any path already registered
in `git worktree list`.

## Steps

1. Ask for (or infer from context) a short branch/worktree name, e.g. `spike-foo`.

2. Resolve the repo root and its sibling worktrees directory on the fly —
   don't hardcode a path:

   ```
   REPO_ROOT="$(git rev-parse --show-toplevel)"
   WORKTREES_DIR="$(dirname "$REPO_ROOT")/$(basename "$REPO_ROOT").worktrees"
   ```

   Then create the worktree as a sibling of the repo:

   ```
   mkdir -p "$WORKTREES_DIR"
   git worktree add "$WORKTREES_DIR/<name>" -b <name>
   ```

   Adjust the branch ref (`-b <name>` vs. checking out an existing branch) to
   match what the user asked for.

3. Copy untracked env files the project needs to run, since a fresh worktree
   checkout won't have them (they're gitignored):

   ```
   cp "$REPO_ROOT/.env" "$WORKTREES_DIR/<name>/.env"
   cp "$REPO_ROOT/.envrc" "$WORKTREES_DIR/<name>/.envrc"
   ```

   Skip any that don't exist in the source repo.

4. Write `$WORKTREES_DIR/<name>/CLAUDE.md` — do not overwrite if one
   already exists from the checked-out branch; instead prepend this block:

   ```
   # Worktree of auohp

   This directory is a git worktree of the main AUOHP repo at
   `$REPO_ROOT` (branch `<name>`, created <date>).
   It is NOT the primary checkout. Project conventions, architecture notes,
   and collaboration style live in that repo's CLAUDE.md — read it for full
   context if this file alone seems insufficient.

   ---

   ```

   followed by the original content, if any. Resolve `$REPO_ROOT` to its
   actual value when writing the file — don't leave the literal variable name
   in the committed CLAUDE.md.

5. Run `yarn install` inside the new worktree so `node_modules` is populated
   (each worktree gets its own, per `nmHoistingLimits: workspaces` in
   `.yarnrc.yml` — see project memory `project_docker_virtiofs_nested_volume_bug`
   for why that setting exists and what it costs).

6. Copy and rename the VS Code multi-root workspace file so the worktree gets
   its own distinguishable entry in VS Code's window title and recent-
   workspaces picker (each worktree is assumed to open in its own window, not
   share one with the main checkout):

   ```
   cp "$REPO_ROOT/AUOHP.code-workspace" "$WORKTREES_DIR/<name>/AUOHP-<name>.code-workspace"
   ```

   Inside the copy, folder `path` entries are relative to the `.code-workspace`
   file's own location, so they still resolve correctly unmodified — only the
   filename needs to change, not the contents.

7. Attach the current session to the new worktree:

   ```
   EnterWorktree({ path: "$WORKTREES_DIR/<name>" })
   ```

   using the resolved absolute path, not the literal variable name.

## Notes

- This worktree will NOT get its own Claude Code memory directory linked to
  the parent's — that's a deliberate non-goal. Memory paths are derived from
  the working-directory path, and sharing a memory dir across two
  concurrently-writable sessions risks last-write-wins clobbering. The
  CLAUDE.md back-pointer is the intended bridge instead: a fresh session can
  read it and go look at the parent's CLAUDE.md/memory directly if needed.
- `ExitWorktree` will refuse to auto-remove a worktree entered via `path`
  (it only owns worktrees it created via `name`). To remove one when done:
  `git worktree remove "$WORKTREES_DIR/<name>"` from the main repo, then
  `git branch -D <name>` if the branch should go too.
