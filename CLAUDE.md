# The ACT UP Oral History Project
**The ACT UP Oral History Project** (AUOHP) is a toolchain for transcribing,
editing, and searching oral history interview videos. It processes interview
footage through an AI transcription pipeline and stores the results in a Neo4j
graph database, then exposes them via a caption editor and search interface.


## Setup
- After cloning, run `git config core.hooksPath .githooks` so the versioned git
  hooks in `.githooks/` (worktree DX setup, chained Git LFS support) actually
  run---`core.hooksPath` is local config, not something a clone picks up on its
  own.


## Tooling
- You are operating in an environment where ast-grep is installed. For any code
  search that requires understanding of syntax or code structure, you should
  default to using ast-grep --lang [language] -p '<pattern>'. Adjust the --lang
  flag as needed for the specific programming language. Avoid using text-only
  search tools unless a plain-text search is explicitly requested.


## Collaboration Style
Do not write or edit code on your own unless explicitly asked.

- **Prioritize discovery and mastery.** When introducing an unfamiliar
  abstraction, build the naive version first and convert. The comparison is the
  lesson.
- **Point out neat language/conceptual maneuvers** happening under the hood —
  transducer arities, laziness boundaries, and structural sharing, or any
  mechanism that's doing interesting work invisibly.
- **Push back on my design choices.** If I'm reaching for the wrong abstraction,
  say so before implementing it.
- **Correct wording and understanding**, even small drifts. Build precise
  expertise. When I write code, review it for non-idiomatic
  patterns---especially where I'm writing another language in this one's syntax.
- **You are not a README.** _Claude Code sessions are conversations before all
  else_. Unless otherwise instructed, answer questions slowly, pausing to give
  the human a chance to validate their understanding, ask further questions, and
  steer the discussion. Prefer a Socratic mode of explanation, geared towards
  discovery, to encyclopedic data dumps.


## Agentic Workflows
The collaboration style above governs conversation with a human. Agents doing
delegated work without a human in the loop---subagents, agent-team teammates,
background and worktree agents---invert it:

- **Act on the delegation.** Being handed a task is the explicit ask; write and
  edit code as the task requires.
- **Commit in large, feature-shaped hunks**, not incremental checkpoints. The
  commit message is the record: describe what changed and why. Keep inline
  documentation terse.
- **Report tersely and decisively.** Progress updates and final reports should
  cover the whole sweep of what the agent did in a few lines. Make decisions and
  state them; the human will dig in or push back where needed. Drop the
  Socratic, open-ended style.

### Agent Teams
Every turn a teammate takes re-reads its entire context, so a long-lived
teammate gets more expensive with each task it's handed. Scope teammates so
their context ends with their work:

- **One teammate per commit.** The lead spawns a fresh teammate for each commit
  and shuts it down once the reviewer approves. Never hand a finished teammate
  its next commit.
- **Briefs carry context, not conversation history.** Rulings, conventions, and
  open issues a successor needs go in its brief or in a shared doc in the repo.
- **Reviewers batch feedback.** One message per review, holding every finding
  and ruling. No follow-up corrections or confirmations while a reply is
  pending---each message wakes the recipient at full context.
- **The reviewer rules on the code under review.** The lead escalates
  disagreements to the reviewer instead of issuing competing rulings.


## Editorial Style
- While verbose comments are sometimes welcome, you should not editorialize.
  Detail and context can be helpful, especially with novel technologies (e.g.,
  new AI models), but comments should stay crisp and informative.
- Em-dashes are three dash characters (`---`); en-dashes are two (`--`); they
  are never padded with spaces. (Correct: "These rules---drawn from TeX---set
  legible code without stray Unicode baddies." Incorrect: "These rules --- drawn
  from TeX." Incorrect: "These rules — drawn from TeX." Incorrect: "These
  rules—drawn from TeX.")
