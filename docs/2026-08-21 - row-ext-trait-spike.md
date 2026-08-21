# Experimental trait spike: `RowExt`

## Before you start

Two things that will bite early, both expanded on below.

**`gql_err` is redundant --- expect to notice this immediately.** The moment you
write `?` in the new module it will just work, with no `.map_err` in sight, and
that will look wrong given the surrounding code. It isn't. async-graphql's blanket
`From` impl already covers it, and `search_interview` in
`graphql/queries/search.rs` has been relying on that all along. Resist the pull to
"fix" the new module into consistency with the old one --- the new module is the
correct version. Cleaning up the other ~47 sites is a separate change and out of
scope here. See *A finding that redirects the obvious version of this exercise*.

**The bound choice is the whole exercise.** `Deserialize<'this>` versus
`DeserializeOwned` is where the actual learning is. `neo4rs` itself chose the
former for `Row::get`; picking the latter is a real narrowing of what the trait
can ever accept. Make that call on purpose, not by reaching for whichever one
compiles first. See *Phase 1*.

## Context

Every trait in this codebase arrives via `#[derive(...)]` or async-graphql's
`#[Object]` macro --- none are hand-declared. This spike is a learning exercise
with a real payload: write one trait by hand, in a place where a trait is the
*only* available mechanism, and see whether it earns its keep.

The target is row destructuring. Every resolver that reads Neo4j repeats the same
three-step dance --- pull a `neo4rs::Node` out of a `Row` by column alias, call
`.to::<T>()` on it, then pull relationship properties off a `neo4rs::Relation`
one at a time. `graphql/queries/interviews.rs` does this eight times in one
function; `graphql/queries/search.rs` does it twice more.

**Why a trait is mandatory, not stylistic:** `neo4rs::Row` is a foreign type.
Rust forbids inherent impls on types from other crates (`E0116`), so
`impl neo4rs::Row { ... }` will not compile. A trait plus a single impl is the
only way to hang a method off it. This is the extension-trait pattern in its
purest form, the same shape as `anyhow::Context` and `itertools::Itertools` ---
including the part where callers must `use` the trait to see the methods.

### A finding that redirects the obvious version of this exercise

The tempting first trait in this codebase is an error-conversion helper to replace
the ~47 `.map_err(gql_err)` sites. **Don't build it.** async-graphql already ships
`impl<T: Display + Send + Sync + 'static> From<T> for Error`, and `?` desugars to
`From::from` on the error path --- so bare `?` already does what `gql_err` does.
The codebase demonstrates this against itself: `search_statements` and
`search_interview` (`graphql/queries/search.rs`) have identical return types, but
the first uses `.map_err(gql_err)` twelve times and the second uses bare `?`.
Both compile.

That makes `gql_err` (`graphql/error.rs`) redundant ceremony around a blanket impl.
Deleting it is a separate, larger change --- noted here so it doesn't get
rediscovered, explicitly **out of scope** for this spike.

## Scope

Convert **`packages/api/src/graphql/queries/captions.rs` only**. The other four
call sites stay unconverted as a control group, so the before/after reads
side-by-side in the same repo. `git checkout` reverts the whole experiment.

## Phase 1 --- the `RowExt` trait

New module: `packages/api/src/graphql/row.rs`, registered in
`packages/api/src/graphql/mod.rs`.

One trait, two methods, exactly one impl --- for `neo4rs::Row`:

- **`node_as`** --- take a column alias, fetch it as a `neo4rs::Node`, deserialize
  into a caller-chosen type. Collapses the current two-line
  `row.get::<Node>(k)?` + `node.to::<T>()?` pair into one call.
- **`rel_prop`** --- take a column alias and a property name, return a typed
  property off the `neo4rs::Relation` at that column. Covers the `startTime` /
  `endTime` reads.

**The one design decision that matters** is the lifetime on the type parameter.
`Row::get` is declared `fn get<'this, T>(&'this self, key: &str) -> Result<T, DeError>
where T: Deserialize<'this>` --- borrowed deserialization, tied to the row's own
lifetime. The trait can mirror that, or it can require `DeserializeOwned`, which is
strictly stronger (`DeserializeOwned` is sugar for `for<'de> Deserialize<'de>`).
The looser bound preserves zero-copy for future borrowing types; the stricter one
reads better and costs nothing today, since every projection in
`graphql/nodes.rs` owns its data. Choose deliberately --- this is the spike's real
question.

Error type: return `neo4rs::DeError` unchanged. Do not convert to
`async_graphql::Error` inside the trait --- the module has no business knowing
about the transport, and `?` at the call site already bridges it via the blanket
`From` impl described above.

## Phase 2 --- `Statement` assembly, built on Phase 1

A constructor on `Statement` (`graphql/nodes.rs`) taking a row plus the
relationship alias, returning an assembled `Statement`, using `RowExt` internally.

Deliberately **not** a trait. With exactly one implementor, a trait is a function
in a costume --- an inherent `impl Statement` is the honest shape. Phase 2 exists
to test whether Phase 1 *composes*: if the assembly code isn't visibly shorter and
clearer than the block it replaces in `captions.rs`, that is evidence against the
whole abstraction, and the correct response is to delete both phases.

Reuse the existing `StatementNode` projection (`graphql/nodes.rs`) rather than
introducing a new intermediate type.

## Verification

Tests live inline as `#[cfg(test)] mod tests` --- `auohp-api` is a binary crate
(`src/main.rs`, no `lib.rs`), so there is no integration-test surface. Follow the
existing pattern in `packages/api/src/captions.rs`.

**These tests need no database.** `Row::new`, `Node::new`, `Relation::new`, and
`BoltNode` / `BoltList` / `BoltMap` / `BoltString` are all public and re-exported
from the neo4rs crate root, so rows can be built by hand in-process.

Cases to cover:

1. `node_as` deserializes a hand-built node row into `StatementNode`.
2. `node_as` on a missing column alias returns `Err`, not a panic.
3. `rel_prop` reads an `f64` property off a hand-built relationship.
4. `Statement::from_row` assembles correctly, including `person: None` and the
   `#[serde(default)]` path where `words` is absent from the node.

Then:

- `cargo check --package auohp-api --tests` --- must pass. Plain `cargo build`
  never compiles `#[cfg(test)]` modules, so check with `--tests`.
- `cargo test --package auohp-api row::` --- new tests pass.
- `cargo test --package auohp-api` --- the two existing `captions::` tests still pass.
- Live smoke test: run the server and issue
  `query { captions(interviewNumber: N) { spanAtTime(timestamp: T) { uid text startTime endTime } } }`
  against a known interview. Compare against the same query on `main` --- the
  response must be byte-identical, since this is a pure refactor.

## Kill criteria

This is an experiment, so name the failure condition up front: if the converted
`span_at_time` body is not meaningfully shorter *and* easier to read than what it
replaced, revert. A trait that only relocates complexity is worse than the
duplication, because it adds an import requirement and a layer of indirection for
every future reader.
