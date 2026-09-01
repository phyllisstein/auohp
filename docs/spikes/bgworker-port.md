# Spike: porting Loco's bgworker to vanilla axum

**Status:** spike, implemented and passing. Not merged, not load-tested, not run
against a real interview.

**Scope constraint:** in-process queues only. No Neo4j-backed queue, no Postgres,
no Redis, no Loco dependency. The trait boundary must admit a durable backend
later without rewriting callers.

## Recommendation up front

**Keep the hand-rolled port for now.** The whole thing is about 900 lines
including tests and comments, it is comprehensible end to end, and it adds two
small dependencies (`tokio-util`, `futures-util`) that this crate would likely
pull in anyway.

But the honest version of that recommendation has a shelf life, and a specific
expiry condition, which is stated in *Would an existing crate beat this?* below.
The short form: **the moment durability becomes a real requirement, stop and
adopt `apalis` rather than writing a durable backend behind this trait.** The
trait boundary is designed to make that switch cheap; it is not an argument for
eventually implementing every backend ourselves.

## What Loco's bgworker actually does

Loco's `src/bgworker/mod.rs` is ~940 lines. The structure worth studying is:

- **`QueueProvider`** --- an object-safe trait with fourteen methods, implemented
  by Postgres, SQLite, Redis, and a `NoopQueue`.
- **`Queue`** --- a newtype over `Arc<dyn QueueProvider>` that forwards every
  call. This is the type application code holds.
- **`BackgroundWorker<A>`** --- the typed side. Implementors define
  `perform(&self, args: A)`, plus `class_name()`, `queue()`, `tags()`.
- **`erase_worker`** --- turns a `BackgroundWorker<A>` into a `JobHandler`, a
  boxed closure `(JobId, JsonValue) -> Pin<Box<dyn Future>>`. This is the
  central trick, and it is what lets differently-typed workers live in one map.
- **`JobRegistry` + `Driver`** (in `sql.rs`) --- a name-to-handler map plus the
  shared worker loop. `Driver` abstracts `dequeue`/`complete_job`/`fail_job` so
  Postgres and SQLite share one loop body.
- **`JobStatus`** --- queued / processing / completed / failed / cancelled.

### Worth taking

1. **The two-trait split.** A typed `Worker` trait for authors, an erased
   `JobHandler` for the dispatch loop. This is the load-bearing idea and it
   ports directly.
2. **The newtype-over-`Arc<dyn>` facade.** Callers name one concrete type; the
   backend is swappable underneath. This is what makes the durability path
   non-breaking.
3. **`catch_unwind` at the job boundary.** Without it a panicking job silently
   kills a worker task and the pool degrades to zero throughput with nothing in
   the logs. Loco gets this right and it is easy to omit.
4. **Cancellation-token shutdown with `biased` select.** Cancellation must win
   ties against an available job, or a saturated queue never drains.
5. **Failing an unroutable job rather than panicking.** A payload that no
   handler matches is bad data, not a reason to stop the process.

### Loco-framework baggage, left behind

1. **`AppContext`.** Loco's workers receive the entire framework context. Our
   `JobContext` carries three fields, so a worker's dependency surface is
   visible in one place and testable by construction.
2. **`WorkerMode` (`ForegroundBlocking` / `BackgroundAsync` / `BackgroundQueue`).**
   Config-driven execution strategy is a testing affordance we get more cheaply
   by calling `perform` directly in a test.
3. **The operational CLI surface** --- `dump`, `import`, `cancel_jobs`,
   `clear_by_status`, `clear_jobs_older_than`, `retry_failed`, `requeue`,
   `ping`, `describe`. Nine of fourteen trait methods exist to serve Loco's CLI.
   Every one is a method a new backend must implement or stub. We take four.
4. **`class_name()` derived from `std::any::type_name`.** Clever, and wrong for
   anything durable: renaming or moving a struct silently orphans every queued
   job of that name. We require an explicit `const NAME`.
5. **`tags` and multiple named queues.** Real features for a multi-tenant job
   fleet. We have two job types and one process.
6. **`interval` / cron scheduling.** Orthogonal concern, not needed yet.
7. **`async_trait`.** Rust 2024 has native `async fn` in traits. We pay the
   `Pin<Box<dyn Future>>` cost in exactly the one place erasure requires it.

## The trait boundary

Two traits, deliberately narrow.

**`QueueBackend`** --- storage and execution: `enqueue`, `start`, `shutdown`,
`describe`. Object-safe, so `Arc<dyn QueueBackend>` works. `enqueue` and
`shutdown` are written in desugared form (returning `Pin<Box<dyn Future>>`)
because object safety forbids `async fn`.

**`StateStore`** --- observable job state: `set_queued`, `set_progress`,
`set_completed`, `set_failed`, `get`. Separate from `QueueBackend` because the
two have genuinely different lifetimes in a durable world: a durable backend
would serve both from the same rows, but an in-process queue paired with a
durable *state* store is also a coherent (and cheap) intermediate step.

### Why this admits a durable backend later

Three properties, each a deliberate cost paid now:

1. **`Envelope` is already the persisted shape.** `(id, name, json_args)` is a
   database row. Nothing serializes it today --- the args are serialized to JSON
   and immediately deserialized, microseconds apart, in the same process. That
   is a real waste of cycles and it is the price of the boundary: a durable
   backend stores the envelope unchanged.
2. **`enqueue` is `async` even though the in-process implementation never
   awaits.** It uses the synchronous `try_send`. The async signature exists
   because a durable enqueue *is* an I/O round trip, and retrofitting async onto
   this method later would break every call site.
3. **Workers name no backend type.** `EmbedInterviewWorker` mentions
   `JobContext` and its own args. Swapping the backend touches `main.rs` and
   `Queue::new`, nothing else.

What a durable backend would additionally need, none of which exists today:
at-least-once delivery with a visibility timeout, a reaper for jobs stranded by
a crashed worker, and a retry policy. Those are the three things that make
durable queues hard, and none of them is stubbed here --- which is exactly why
the recommendation is to adopt `apalis` rather than write them.

## Wiring into the existing architecture

**How a mutation enqueues.** `Queue` is injected into the schema alongside `Db`
and `Arc<EmbedderHandle>`:

```
Schema::build(..).data(db).data(embedder).data(queue).finish()
```

and recovered in a resolver with `ctx.data::<Queue>()?`. Note `.data()` keys by
`TypeId`, which is why `Queue` is a distinct newtype rather than a bare
`Arc<dyn QueueBackend>` --- two dependencies sharing a type would collide, with
the later `.data()` silently overwriting the earlier.

`seed_interview` is converted as the end-to-end demonstration. It previously
did:

```
tokio::spawn(embed_statements(db.clone(), embedder, uids, texts));
```

a fire-and-forget whose `JoinHandle` was dropped. That worked, and bought
nothing back: no id, no status, no backpressure, and a panic in the task
vanished. It now enqueues a job and returns `embeddingJobId` in the payload,
pollable via `query { job(id:) { phase progress message error } }`. The
durability story is unchanged --- a restart still loses queued work, exactly as
before.

**How workers reach `Db` and `EmbedderHandle`.** Constructor injection through
`JobContext`, built once in `main.rs` from the *same* `Arc`s the HTTP side
uses --- one Neo4j pool, one embedder thread, shared rather than duplicated per
subsystem. `embedder` is `Option` purely so tests can build a context without
loading an ONNX model; `JobContext::embedder()` turns absence into an ordinary
job failure, so the testing affordance cannot become a production nil-deref.
This is what lets all 25 tests run with no services up.

**Where job state is observed.** `InMemoryStateStore`: a `Mutex<HashMap>` with
two independent bounds --- a TTL (default 1h) and a capacity cap (default 10k).
Either alone is insufficient; the TTL stops a low-traffic server holding state
forever, the cap stops an enqueue burst growing the map before any TTL expires.
Read by the `job(id:)` resolver.

Its real limitation: `None` conflates "never existed" with "evicted". A client
cannot tell a bad id from a job that finished an hour ago. A durable backend
distinguishes them; this one cannot.

## CPU-bound work, and the two-scheduler problem

This is the part where the obvious design is wrong, and it is specific to this
codebase.

The naive move for CPU-bound work is `spawn_blocking`. For embedding, that would
be wrong. `auohp-core`'s `EmbedderHandle` is **not** an `Embedder` behind a
mutex --- it is a handle to a dedicated OS thread that owns the ONNX session
outright, with its own priority and background queues, and cooperative
sub-batching so interactive search preempts bulk seeding. The blocking work has
already been moved off the async runtime, once, inside core. Wrapping the call
in `spawn_blocking` would add a second thread that does nothing but block on a
channel waiting for the first, burning a blocking-pool slot to no purpose.

So the rule for this job system: **a worker never calls `spawn_blocking` on
something that already owns its own thread. It awaits the handle.**
`EmbedInterviewWorker` calls `embed_background` (not `embed`) to mark itself as
bulk work, so search stays responsive.

The consequence for pool sizing: **worker concurrency is not "number of cores".**
Both real job types funnel into a single ONNX thread, so raising concurrency
buys no inference parallelism --- only the ability to run an unrelated job
alongside one that is embedding. The default is 2. Setting it to `num_cpus`
would create workers that queue behind each other inside `EmbedderHandle` while
each holds a whole interview's transcript in memory.

`spawn_blocking` *is* the right tool for the other pending job type, whisper.cpp
transcription, which has no equivalent handle. That job does not exist yet. When
it does, the honest question is whether it should get its own dedicated thread
in core (matching the embedder's shape) rather than a blocking-pool slot ---
because two jobs contending for the GPU is its own scheduling problem, and the
job queue is the wrong layer to solve it.

## Shutdown semantics and the durability tradeoff

The pool is shut down in `main.rs` *after* `axum::serve(..).await` returns, not
inside `shutdown_signal`. That ordering is load-bearing: `axum::serve` returns
only once graceful shutdown has fired *and* every in-flight HTTP request has
finished, so by then no new job can possibly be enqueued --- the last resolver
that could have called `enqueue` has already returned. Draining then is draining
a queue that provably cannot grow.

Racing the two instead would let a request in flight at Ctrl+C enqueue into a
closing queue and get an error for work the server was still nominally
accepting.

**What shutdown costs:**

| | behaviour |
|---|---|
| In-flight jobs | run to completion; shutdown awaits them |
| Queued, not yet dequeued | **dropped**, silently, not persisted |
| Job state | lost with the process |

Cancelling the token stops workers at their *next* dequeue. It does not
interrupt a running job --- there is no safe way to interrupt a `spawn_blocking`
closure mid-inference --- which is why shutdown waits rather than aborts. That
wait is unbounded: a job three minutes into an interview delays shutdown by up
to three minutes. In a container with a 30s SIGKILL timeout, that means the
process is killed mid-job anyway. **A shutdown timeout is a known gap**, listed
below.

Both drop behaviours are asserted in tests, not merely documented
(`shutdown_waits_for_in_flight_work`, `shutdown_drops_undequeued_jobs`).

**The mitigation, and why it is nearly free here.** Make every job idempotent
and re-derivable from graph state. Embedding already is: the Cypher is
`MATCH ... setNodeVectorProperty`, so re-running is safe, and "which statements
lack embeddings" is a query the graph can answer:

```cypher
MATCH (s:Statement) WHERE s.embedding IS NULL RETURN s.uid, s.text
```

So a startup sweep that re-enqueues unembedded statements converts "lost on
restart" into "delayed until next restart", with no queue table. That sweep is
**not implemented** in this spike, and it is the single highest-value follow-up
--- it is what makes the in-process queue defensible rather than merely cheap.

## Cost estimate

Implemented, `cargo check` and `cargo test` clean:

| Item | Lines |
|---|---|
| `jobs/` module, 7 files excluding tests | 1287 |
| `jobs/tests.rs` (25 tests) | 594 |
| GraphQL status query | 91 |
| Wiring diff (5 files) | +129 / -100 |
| **New code total** | **1972** |

Comments are a large fraction by house style, and `seed_interview.rs` is net
*shorter* (-134/+~40) because the old `embed_statements` helper moved into the
worker. Actual logic across the module is roughly 400 lines.

**Remaining to production-ready, if kept:**

| Work | Estimate |
|---|---|
| Startup re-enqueue sweep for unembedded statements | 0.5 day |
| Shutdown timeout (bound the in-flight wait) | 0.5 day |
| Retry with backoff | 1 day |
| Load-test with a real interview | 0.5 day |
| **Total** | **~2.5 days** |

**Not included, and deliberately so:** durability, at-least-once delivery,
visibility timeouts, a stranded-job reaper. Those are multi-week and are the
point at which the recommendation flips.

## Risks

1. **Silent job loss on restart.** The headline risk. Mitigated by the startup
   sweep, which is not yet written. Until then, `seedInterview` promises
   embeddings it may not deliver, and nothing tells the operator.
2. **Unbounded shutdown wait.** A long job outlives a container's SIGKILL
   timeout and is killed mid-flight anyway. Needs a timeout.
3. **No retries.** A transient Neo4j blip fails the whole job permanently.
   Marked `Failed` and left there.
4. **Backpressure surfaces as a user-visible error.** A full queue (512 slots)
   fails `seedInterview` outright. Correct behaviour --- much better than an
   unbounded channel and an OOM kill that takes the HTTP server with it --- but
   it is a failure mode that did not previously exist, since the old
   `tokio::spawn` had no ceiling at all.
5. **Job args hold whole transcripts in memory.** A queued job pins every
   statement's text. 512 slots is a large memory ceiling. Passing an interview
   uid and re-reading from Neo4j in the worker would be strictly better and is
   worth doing before this ships.
6. **Concurrency is a footgun.** Raising it looks like it should speed things up
   and does not, because of the single ONNX thread. Documented at the
   constructor and in `main.rs`, but it will still be reached for.
7. **State-store `None` is ambiguous.** Unknown id and evicted job are
   indistinguishable.

## Would an existing crate beat this?

Genuinely considered, not a formality.

**`apalis` (0.7.4, ~278k recent downloads, actively maintained --- last release
May 2026).** The strongest candidate, and stronger than expected once its
feature list is read: it is built on **Tower**, which this crate *already
depends on* (`tower`, `tower-http` are in `Cargo.toml`, and `main.rs` already
composes a `ServiceBuilder`). That means retries, timeouts, concurrency limits,
and `catch-panic` are Tower layers rather than bespoke code --- items 2, 3, and
part of 6 on the risk list above are configuration, not implementation. It
injects dependencies through a `Data` extractor, which maps cleanly onto our
`JobContext` fields. It supports Redis/Postgres/SQLite storage for the day
durability matters.

Reasons not to adopt it *today*: its documented centre of gravity is external
storage (the docs lead with Redis), so an in-process-only deployment is not the
path of least resistance; and its Tower-`Service`-per-job model is a second
abstraction to learn on top of the one this codebase is still absorbing. Neither
is disqualifying.

**`tokio` alone (status quo).** Already in use --- a bare `tokio::spawn` in
`seed_interview`. It is genuinely adequate for *one* fire-and-forget job. It
gives no id, no status, no backpressure, no panic isolation, and no shutdown
coordination. Adding a second job type is where it stops being adequate, and the
transcription job is that second type.

**`underway`, `faktory`.** Postgres-backed and external-server respectively.
Both violate the in-process constraint outright.

### The honest conclusion

For *this* spike's constraints --- in-process only, no new infrastructure, a
codebase whose author is learning Rust and values reading the machinery --- the
hand-rolled port wins. It is ~350 lines of logic, it fits the existing
architecture exactly, and the erasure/`Pin<Box<dyn Future>>`/cancellation
mechanics are worth having written once.

**The expiry condition, stated precisely:** the moment any of these becomes
true, adopt `apalis` instead of extending this.

- Job loss across restarts becomes unacceptable (i.e. durability is required)
- Retries with backoff are needed
- A third job type appears with different scheduling needs
- Anyone proposes writing a durable backend behind `QueueBackend`

That last one is the trap this document exists to prevent. The trait boundary is
designed to make *swapping to* a durable implementation cheap. It is **not** an
argument that we should be the ones to write it. Writing a correct durable queue
means at-least-once delivery, visibility timeouts, and a stranded-job reaper ---
weeks of work, and precisely the work `apalis` has already done and debugged.

## Files

| Path | |
|---|---|
| `packages/api/src/jobs/mod.rs` | module root |
| `packages/api/src/jobs/worker.rs` | `Worker` trait, `JobContext` |
| `packages/api/src/jobs/registry.rs` | type erasure, `Registry` |
| `packages/api/src/jobs/backend.rs` | `QueueBackend`, `InProcessBackend` |
| `packages/api/src/jobs/queue.rs` | `Queue` facade, `QueueError` |
| `packages/api/src/jobs/handle.rs` | `JobId`, `JobState`, `StateStore` |
| `packages/api/src/jobs/workers.rs` | `EmbedInterviewWorker` |
| `packages/api/src/jobs/tests.rs` | 25 tests |
| `packages/api/src/graphql/queries/jobs.rs` | `job(id:)` status query |

Modified: `main.rs`, `graphql/schema.rs`, `graphql/queries/mod.rs`,
`graphql/mutations/seed_interview.rs`, `Cargo.toml`.

## Verification

```
cargo check -p auohp-api    # clean, 0 errors
cargo test  -p auohp-api    # 25 passed, 0 failed, 0 ignored
```

The shutdown test was confirmed non-vacuous by deleting `tracker.wait().await`
and observing `shutdown_waits_for_in_flight_work` fail, then restoring it.

Not verified: behaviour against a live Neo4j, a real embedder, or a
full-length interview. No load testing.
