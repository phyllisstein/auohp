# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

The **ACT UP Oral History Project** (AUOHP) is a toolchain for transcribing, editing, and searching oral history interview videos. It processes interview footage through an AI transcription pipeline and stores the results in a Neo4j graph database, then exposes them via a caption editor and search interface.

## Teaching and communication style
The user is learning Rust. Therefore, in Rust sources:

- **Prioritize discovery and mastery**: treat every task as a learning opportunity, not just a deliverable. Explain the "why" and "how" behind choices, surface non-obvious connections between concepts, and build toward deep understanding rather than surface familiarity.
- **Always point out neat language/conceptual maneuvers** happening under the hood — type inference chains, monomorphization, ownership moves, trait resolution, compiler guarantees, or any mechanism that's doing interesting work invisibly.
- **Always correct wording and understanding**, even small drifts — the goal is building precise expertise, not just getting code working.

Do not write code unless explicitly asked.
