---
description: "Review and simplify the AUOHP Rust transcription stack with a quality-first mindset"
name: "Simplify Rust Transcription Stack"
argument-hint: "Inspect the transcription pipeline, recommend simplifications, and explain crate choices without writing code unless asked"
agent: "agent"
---

If you are reading this outside VS Code, ignore the YAML frontmatter above and treat the rest of this file as a standalone prompt.

Review the AUOHP Rust transcription stack and recommend a lower-complexity design that preserves or improves transcript quality on real AUOHP interview material.

Before proposing changes, read the two development notes in this order:

1. `development/rust-transcription-simplification-plan.md`
2. `development/rust-transcription-crate-greatest-hits.md`

Use the first note for the strategic picture and the second note for crate and tooling judgment.

Focus on these files first:

- `packages/auohp-api/src/transcription/whisper.rs`
- `packages/auohp-api/src/transcription/align.rs`
- `packages/auohp-api/src/transcription/diarize.rs`
- `packages/auohp-api/src/transcription/pipeline.rs`
- `packages/auohp-api/src/transcription/types.rs`
- `packages/auohp-api/src/transcription/line_breaking.rs`
- `packages/auohp-api/Cargo.toml`

Use real AUOHP interview material as the quality reference, especially sections dense with movement vocabulary, ACT UP institutions, drug names, and person names.

Use these constraints:

- Optimize for transcript quality and caption readability, not raw throughput.
- Treat AUOHP domain vocabulary as a primary acceptance criterion.
- Do not assume `whisper-rs` is the destination just because it is simpler; its earlier DTW output was already judged unacceptable.
- Keep custom code only where it is clearly product-specific or where the Rust ecosystem is still too thin.
- Treat `line_breaking.rs` as a downstream aid, not a fix for poor ASR or segmentation.
- Assume the final answer may be read by a Rust learner, not just an experienced systems engineer.

Use this process:

1. Identify which parts of the current pipeline are true product logic versus replaceable ML plumbing.
2. Recommend which crates or tools should stay, which should be treated as controls only, and which new crates are worth evaluating.
3. Propose a small number of architectural seams that would make backend swaps or simplification safer.
4. Explain the tradeoffs in plain language for a Rust learner.
5. If asked for implementation, prefer small, reversible changes and avoid rewriting the whole stack at once.

Response checklist:

- [ ] Separate product logic from ML plumbing.
- [ ] Name crates to keep, crates to evaluate, and crates to deprioritize.
- [ ] State whether Candle should remain the likely center of the ASR path.
- [ ] Explain what `align.rs` and `diarize.rs` have to prove to stay.
- [ ] Mention risks, regressions, and unknowns.
- [ ] Prefer repo-relative file paths over abstract discussion.
- [ ] Do not write code unless explicitly asked.

Tone and format:

- Write like an engineering note, not marketing copy.
- Be concrete and opinionated.
- If you recommend against something, say so directly.
- If a tradeoff is uncertain, say what evidence would change your mind.
