# Hold `{pool, config}` in `JobQueue` instead of a built `SqliteStorage`

Status: **implemented**, on branch `spike-apalis-sqlite`. 20 tests passing,
unchanged from before. See "Outcome" at the end for what the plan did not
anticipate.

## Context

`JobQueue` (`packages/api/src/jobs/queue.rs`) is the enqueue-side façade that
GraphQL resolvers hold via async-graphql's `.data()`. Its module doc justifies
its existence like this:

> `SqliteStorage<T, C, F>` is generic over its argument type, its codec, and its
> fetcher. Putting it directly into async-graphql's `.data()` would mean
> resolvers naming that whole three-parameter type, and it would mean a second
> job kind forces a second `.data()` entry with a different concrete type.

That reasoning is correct, and the struct then stores an
`EmbedStorage = SqliteStorage<EmbedInterview, ...>` field anyway. So the façade
does not currently deliver the property its own documentation claims for it: the
generic type is hidden from *resolvers*, but it is still baked into `JobQueue`,
which means a second job kind still forces a second storage handle and
`JobQueue` remains spelled for exactly one job.

`storage.rs:138-143` already states the intended design, on `open_pool`:

> Returns a pool rather than a `SqliteStorage` because the pool is the shared
> resource: the enqueue side and the worker side each build their own
> `SqliteStorage` *view* over this one pool.

This change makes `JobQueue` obey the principle the codebase already wrote down.
It is **not** a generalization of the enqueue API --- `enqueue_embed` stays
concrete. It removes a structural obstacle without spending anything on
generics, so a later generic `enqueue<T>` becomes possible rather than blocked.

### Why this is the right size of change

Investigation established three things that bound the work:

1. **The storage layer is already polymorphic.** `apalis-sqlite`'s
   `queries/backend/fetch_next.sql` filters on `WHERE job_type = ?2`, and
   `fetch_next_shared.sql` accepts a *set* of job types. Distinct job kinds
   already share one file without seeing each other's rows. The monomorphism is
   purely in the Rust handle.
2. **The tests already use this shape.** `probe_storage(&pool, &config)`
   (`jobs/tests.rs:79`) builds storage on demand from exactly these two values,
   and is called at eight sites. Per-call construction is already demonstrated
   as cheap --- `SqlitePool` is internally an `Arc`, so it is a refcount bump.
3. **`JobQueue::pool()` is dead.** `grep -rn "\.pool()" packages/api/src/`
   returns nothing; `main.rs:137` passes `jobs_pool.clone()` directly. The
   accessor can be deleted rather than preserved.

### What it buys

async-graphql's `.data()` is a `TypeId` map (noted at `graphql/schema.rs:25`).
`SqliteStorage<A>` and `SqliteStorage<B>` are different keys, so today N job
kinds would mean N context entries. After this change `JobQueue`'s fields are
two non-generic types, so the context stays at **one entry regardless of how
many job kinds exist.**

## Changes

### 1. `packages/api/src/jobs/queue.rs` --- the substantive change

- Replace the struct fields: `{ storage: EmbedStorage, pool: SqlitePool }`
  becomes `{ pool: SqlitePool, config: StorageConfig }`.
- `JobQueue::new` keeps its current signature (`pool`, `&StorageConfig`) and
  stores a clone of the config instead of building a storage. `StorageConfig` is
  `Clone` and is two small fields (`String` + `Duration`).
- Delete the `pool()` accessor (dead; see above).
- In `enqueue_embed_after`, build the storage locally before the push, mirroring
  `probe_storage` in the tests. This also removes the existing
  `let mut storage = self.storage.clone()` line and the comment explaining why
  the clone was needed --- the `&mut self` constraint is satisfied by a local.
- Keep `EmbedStorage` as a type alias. It is still the right name for the
  worker-side view and for the local built in `enqueue_embed_after`.
- **Update the module doc (lines 3-10).** This is the point of the change, so
  the comment must stop describing a problem the code no longer has. State that
  the queue holds the pool and config and monomorphizes per call, and that this
  is what keeps the context to one entry per N job kinds. Follow the house dash
  convention (`---`, not Unicode).

### 2. Queue name becomes per-job-kind, bound to the job type

`to_apalis_config` currently hardcodes `QUEUE_NAME`, so every storage built from
a `StorageConfig` shares one apalis queue name. Parameterize it --- but *not* as
a bare `&str` threaded through call sites.

**The hazard being designed against:** the enqueue side and the worker side must
pass the same queue name. `fetch_next.sql` filters `WHERE job_type = ?2`, so a
mismatch means the worker polls for a `job_type` no row carries. Jobs sit
`Pending` forever and both sides look healthy --- no error, no type failure.
Today that cannot happen because both call `to_apalis_config()` and get the
constant; a bare parameter would replace that guarantee with a convention.

Bind the name to the job type instead, so the two sides derive it from the same
place and cannot disagree:

- `packages/api/src/jobs/embed.rs` --- add `pub const QUEUE: &str =
  "auohp-embeddings";` to the **existing** `impl EmbedInterview` block (line 59,
  alongside `new`). No new block needed.
- `packages/api/src/jobs/storage.rs` --- `to_apalis_config(&self, queue: &str)
  -> Config`, using the argument in place of `QUEUE_NAME`. Remove the
  `QUEUE_NAME` constant; its doc comment (lines 73-79) explains queue
  partitioning and should be **moved to the new const on `EmbedInterview`**, not
  deleted --- the explanation of `job_type` partitioning is the valuable part and
  is now more accurate where the name actually lives.
- Update the three call sites to pass `EmbedInterview::QUEUE`:
  `queue.rs:68` (disappears with this refactor --- `JobQueue::new` no longer
  builds a storage), `queue.rs:220` (`run_worker`), and `tests.rs:80`
  (`probe_storage`).

Note `tests.rs:80` is the enqueue/worker correspondence in miniature: the same
helper serves both sides in tests, which is why the tests pass today and why
they will keep passing.

### 3. Call sites --- expected to be zero-change

`main.rs:105` (`JobQueue::new(jobs_pool.clone(), &jobs_config)`) and
`tests.rs:564` are the only constructors, and the signature is unchanged. The
two resolver sites (`mutations/seed_interview.rs:415`,
`queries/jobs.rs:115`) call `ctx.data::<JobQueue>()` and then `enqueue_*`, none
of which change. Verify rather than assume.

## Verification

1. `cargo test -p auohp-api --no-run` --- confirm it compiles. Expect the
   previously-recorded dead-code warning on `JobQueue::pool` to **disappear**;
   its removal is part of the change.
2. `cargo test -p auohp-api` --- expect **20 passed; 0 failed**, the same count
   as before. This is a pure refactor with no behavioural change, so any
   movement in that number is a regression.
3. The four capability tests are the ones that would catch a mistake in storage
   construction, since they exercise the real fetch path:
   `queued_jobs_survive_a_restart`,
   `failed_jobs_are_retried_until_they_succeed`,
   `scheduled_jobs_do_not_run_before_they_are_due`,
   `permanently_failing_jobs_stop_at_the_attempt_ceiling`.
4. `grep -rn "\.pool()" packages/api/src/` --- still no hits.
5. Confirm the enqueue path end-to-end via the one test that goes through
   `JobQueue` rather than `probe_storage` (`tests.rs:564`).
6. `grep -rn "QUEUE_NAME" packages/api/src/` --- expect **no hits**; the constant
   moves to `EmbedInterview::QUEUE`. A lingering hit means a call site was
   missed.
7. The queue-name change is the one part of this that could strand jobs silently
   rather than fail loudly, so `queued_jobs_survive_a_restart` is the specific
   test to watch: it pushes on one side and drains on the other, which is exactly
   the correspondence a name mismatch would break.

## Out of scope

Deliberately not included, per the discussion that produced this plan:

- **A generic `enqueue<T>`.** The only near-term second enqueue-from-resolver
  job is NER, which is speculative. The cron sweeps (tombstone reaping,
  embedding refresh) do not touch `JobQueue` at all --- they need recurrence,
  not enqueue ergonomics. Note that `EmbedInterview::QUEUE` is an inherent
  associated const, not a trait: if a generic `enqueue<T>` does arrive later, it
  will want a `QueuedJob` trait with `const QUEUE: &str` to bound on, and the
  inherent const converts into a trait impl with no call-site churn. That
  conversion is the natural moment to introduce the trait --- when there is a
  second implementor to validate its shape.
- **A worker-registration seam.** This is the axis that will actually accumulate
  copy-paste (each job kind adds a `run_worker_*`, a `tokio::spawn`, and a
  shutdown fan-out in `main.rs`), and it serves all four candidate jobs rather
  than two. Worth doing, but separately and later.
- **`apalis-cron`.** Not vendored in the local registry; it would be a sixth
  direct dependency on a third release line, against a spike whose known sharp
  edge is rc version skew.

## Outcome

Implemented as planned. `cargo test -p auohp-api` reports **20 passed; 0
failed**, the same count as before --- this was a pure refactor, so an unchanged
count is the result we wanted. The dead-code warning on `JobQueue::pool`
disappeared, as predicted, because the accessor was deleted rather than kept.

Two things the plan did not anticipate, both worth recording.

### The tests were not exercising queue partitioning at all

The plan treated updating `probe_storage` (`jobs/tests.rs`) as a mechanical call
site. It is not. The test suite defines its own `Probe` job type, but
`probe_storage` called `to_apalis_config()` with no argument and therefore got
the one hardcoded `QUEUE_NAME` --- so **the probe jobs were running under the
production embedding queue's name.**

That was harmless (each test gets a temp-dir database) but it meant the suite
could never have caught a `job_type` partitioning bug, because there was only
ever one partition in play. `Probe` now declares its own
`const QUEUE = "auohp-test-probe"`, so the tests genuinely exercise two job
kinds sharing one file --- which is the multi-job scenario this whole refactor
is meant to unblock. The 20 passing tests are now weak-but-real evidence that
partitioning works, where before they were evidence of nothing.

This is a small instance of a general trap: a helper that defaults its way to
the same value as production code looks like it is testing isolation while
testing nothing of the kind.

### The queue name wanted to be a `const`, not a parameter

The plan's second section originally proposed
`to_apalis_config(&self, queue: &str)` with callers passing the name. That
signature is correct but the ergonomics are a hazard: the enqueue side and the
worker side must pass the *same* string, `fetch_next.sql` filters
`WHERE job_type = ?2`, and a mismatch strands jobs `Pending` forever with no
error on either side. Nothing in the type system compares two string literals.

Binding the name to the job type (`EmbedInterview::QUEUE`, `Probe::QUEUE`)
converts that from a convention into a correspondence: both sides name the
const, so they cannot drift. `grep -rn "auohp-embeddings"` now returns exactly
one hit --- there is no second literal to get wrong.

### Follow-up left open

`graphql/row.rs` has an unrelated uncommitted edit that removes two import
lines. It deletes `auohp_core::eval::Op`, `DeError` and `Version`, which are
genuinely unused (the compiler warns on all three), but takes `BoltType` and
`Row` with them, which are still used at nine sites. Not touched here.
