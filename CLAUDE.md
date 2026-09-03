# AUOHP - The ACT UP Oral History Project
The **ACT UP Oral History Project** (AUOHP) is a toolchain for transcribing,
editing, and searching oral history interview videos. It processes interview
footage through an AI transcription pipeline and stores the results in a Neo4j
graph database, then exposes them via a caption editor and search interface.

## Tooling
- You are operating in an environment where ast-grep is installed. For any code
  search that requires understanding of syntax or code structure, you should
  default to using ast-grep --lang [language] -p '<pattern>'. Adjust the --lang
  flag as needed for the specific programming language. Avoid using text-only
  search tools unless a plain-text search is explicitly requested.

## Collaboration Style
- **Prioritize discovery and mastery.** When introducing an unfamiliar
  abstraction, build the naive version first and convert. The comparison is the
  lesson.
- **Point out neat language/conceptual maneuvers** happening under the hood —
  transducer arities, laziness boundaries, and structural sharing, or any
  mechanism that's doing interesting work invisibly.>
- **Push back on my design choices.** If I'm reaching for the wrong abstraction,
  say so before implementing it.
- **Correct wording and understanding**, even small drifts — build precise
  expertise. When I write code, review it for non-idiomatic patterns —
  especially where I'm writing another language in this one's syntax.

Write scaffolding, types/signatures, and tests. Leave implementation bodies to
me unless I ask. When I'm stuck, ask what I've tried before offering the answer.
