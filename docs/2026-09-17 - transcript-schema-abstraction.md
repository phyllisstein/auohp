---
status: In Progress
created: 2026-09-17
updated: 2026-09-17
---

# Transcript schema abstraction
## Context
Iterating on editorial and rendering features for transcription data has become increasingly difficult. Friction results from a design problem. Transcriptions serve different purposes, but they're all bound to a single `(:Transcript)-[:CONTAINS]->(:Statement)` tree.

- **Captioning.** View VTT files with timings (`[:CONTAINS {startTime, endTime}]`), formatted text (`(:Statement {text})`), and metadata (`:Statement`, and/or deeper walk).
- **Editing.** Modify the text content of transcriptions (`(:Statement {text})`), reformat it (also `(:Statement {text})`), and adjust timings (`[:CONTAINS {startTime, endTime}]`).
- **Search.** Query the text content of transcriptions (`(:Statement {embedding})`), returning timings (`[:CONTAINS {startTime, endTime}]`) and metadata (deeper walk).

The unified schema was designed with automagical editorial affordances in mind. The reasoning held that a single "editing" pane could harness multiple kinds of edits for the purpose of sanitizing and enriching the overall corpus. A director, seeing "ACT UP" split across two caption segments, would remove the split for readability's sake---improving, at the same time, the search index and the ability to add knowledge graph edges, neither of which could usefully span two statement nodes.

In reality, the perverse outcome prevailed: different use cases created shearing in the schema. Core transcript data can't be tombstoned on edit because presentational caption edits are too fluid and fast-moving. Captions can't be in a drafting state because the core data has to remain live. Transcript text must be split into `:Statement` hunks that are searchable _and_ readable _and_ indexable.

> [!WARNING] TK: Tautology. "Makes itself necessary"?
>
> The magical "ACT UP" edit _wouldn't be necessary_ if the transcript data, the search index, the knowledge graph, and the formatting of a caption were separated at the schema level.


## Decision
Separate "Transcription" and "Captions" in the database schema. `:Transcription` becomes a data-rich singleton; `:Caption` becomes a persisted presentational artifact generated from a `:Transcription`.

### Schema changes
#### `:Transcription`
`:Transcript` subtly implies a source type, inasmuch as "transcript of an image" would make no sense and "transcript of a document" would be a tossup. It is renamed `:Transcription`, and grows an additional label based on source type. _Source type_ is based on the medium, not the container. What the current schema thinks of as the "transcript" of a video is a _transcription_ of the _speech_ contained in a video.

```cypher
(:Video)-[:TRANSCRIBED_AS]->(:Transcription:Speech)
```

Similarly, an image would have a transcription of its text.

```cypher
(:Image)-[:TRANSCRIBED_AS]->(:Transcription:Text)
```

Sources might be transcribed in multiple ways---for instance, the speech in a documentary may be transcribed along with the visible text of posters.

```cypher
(:Video)-[:TRANSCRIBED_AS]->(:Transcription:Speech)
(:Video)-[:TRANSCRIBED_AS]->(:Transcription:Text)
```

This separation allows presentational layers to be derived more flexibly. _Searching_ the text visible in a documentary is useful; _captioning_ it would be redundant.

All `:Transcription`s produce `:Text`.

```cypher
(:Transcription:Speech)-[:TRANSCRIBED_INTO]->(:Text)
```

Timings are still stashed on edges in this iteration.

```cypher
[:TRANSCRIBED_INTO {
    startTime:     FLOAT
    endTime:       FLOAT
}]
```

Mapping transcription text to timestamps in a stable, indexed way is the single problem the MVP is meant to solve, and the simple "big bag of properties" design of the `:INTO` edge reflects a need to commit and ship. It's a hand-wave. "At what timestamp does this speech appear in that interview" is actually a whole class of problem. At what timestamp is this poster visible in that documentary? On what physical page does this broadsheet mention AZT? In what paragraph of text? At which word? A principled, generalized locatability pattern is crucial but out of scope.

`:Transcription` includes metadata fields reflecting the process that generated it.

```cypher
(:Transcription {
    createdAt:  ZONED DATETIME
    updatedAt:  ZONED DATETIME
    model:      STRING
})
```

A `:Caption` has edges to `:Statement` nodes. `:Statement` holds on to `embedding` and `text`, drops `words`. Caption timings remain on `:HAS_STATEMENT` edges.


### Search
<!-- TKTK: Search is presentational -->
<!-- TKTK: Embeddings index both raw transcript text and presentational text? -->
<!-- TKTK: ?
    (:Video)-[:SEARCHABLE_AS]->
    (:Transcription)-[:SEARCHABLE_AS]->
-->

### Schema Changes
- `:Transcription`
