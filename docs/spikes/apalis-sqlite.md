# Spike: apalis + SQLite for background jobs

Status: complete, compiling, 20 tests passing. Committed on branch
`spike-apalis-sqlite`, based on `main`. Not merged --- this is a spike, and
adopting it is a separate decision.

This is the third of three spikes on the same problem. Each lives on its own
branch off `main`, and each doc is present only on its own branch:

- `spike-bgworker-port` --- `docs/spikes/bgworker-port.md`, a hand-rolled
  in-process queue.
- `spike-loco-migration` --- `docs/spikes/loco-migration.md`, adopting the
  Loco framework wholesale (declined).
- `spike-apalis-sqlite` --- this document.

## Why this spike exists

Reviewing spike 1 turned up an observation about apalis's feature table:

```
limit       = [tower/limit]
retry       = [tower/retry]
timeout     = [tower/timeout]
catch-panic = [dep:futures-util]
```

Every cross-cutting concern delegates to Tower --- which `packages/api`
already depends on at 0.5.3. The hypothesis was that spike 1 hand-rolled a
re-derivation of abstractions already sitting in the dependency tree, and that
apalis would deliver strictly more functionality in dramatically fewer lines.

**That hypothesis is right on both counts.** An earlier revision of this
document concluded the opposite about line count; that conclusion was a
measurement artifact, corrected below.

## Measured outcome

| | spike 1 (hand-rolled) | spike 3 (apalis) |
| --- | --- | --- |
| Job-module code, excl. tests | 641 | **387** |
| Job-module comments, excl. tests | 566 | 523 |
| Comment : code ratio | 0.88 | 1.35 |
| Test code (`jobs/tests.rs`) | 396 | 399 |
| Job-module files | 8 | 6 |
| Total lines added, all `.rs` | 2,094 | 1,738 |
| New crates | 2 | 59 |
| Direct deps added | 2 | 5 |
| Job tests / crate tests | 17 / 25 | 12 / 20 |
| Survives restart | no | yes |
| Retries | no | yes |
| Delayed / scheduled jobs | no | yes |
| Orphan recovery | no | yes |

Code lines are measured with `cloc`, excluding `tests.rs`, over
`packages/api/src/jobs/` plus `graphql/queries/jobs.rs`. Test files are near
identical in size (396 against 399) and cancel.

**The apalis implementation is 40% smaller: 387 code lines against 641.** It
delivers three capabilities spike 1 lacks at any size, and a fourth (orphan
re-enqueue) it did not attempt --- while being the smaller of the two.

### Why an earlier revision got this backwards

The first pass counted *physical* lines and found parity --- 1,602 against
1,972, "a wash." That was wrong, and the way it was wrong is worth recording.

Physical lines bundle two independent variables: implementation size and
comment density. This workspace is commented heavily by instruction, and the
two spikes are not commented at equal density --- 1.35 comment lines per code
line here against 0.88 there. Leaning on an unfamiliar external API requires
more explanation per line than defining your own abstractions does, so the
denser prose sits on precisely the branch with less code to write. The
confound is correlated with the measured quantity and moves against it,
which is exactly the arrangement that cancels a real 40% difference into an
apparent tie.

The general lesson: when a comparison lands suspiciously on "no difference,"
check whether the unit of measurement is load-bearing before believing it.

### Where the difference actually lives

The totals understate how lopsided this is, because most of what both spikes
write is work neither gets to skip. Spike 1's four largest files are pure
queue machinery, and apalis has no counterpart to any of them:

| spike 1 file | code | what it is |
| --- | --- | --- |
| `backend.rs` | 167 | the in-process backend and dispatch loop |
| `handle.rs` | 120 | join-handle bookkeeping and shutdown |
| `workers.rs` | 109 | the worker supervisor |
| `registry.rs` | 75 | type-erased job registry |
| | **471** | **machinery that the dependency replaces** |

Spike 3's 387 lines are almost entirely shared work that spike 1 also pays
for: `embed.rs` is 124 lines and mostly Neo4j boilerplate, plus the status
query, the storage config, and the enqueue surface. Subtract the shared work
from both sides and the machinery delta is the whole finding.

The crate delta is the real cost: 807 to 866 resolved packages. That is half
of Loco's +116, but it is not the +2 of the hand-rolled version. `sqlx`
arrives with its macro machinery and a SQLite driver, and this workspace has
never carried a SQL dependency before.

## What had to be declared, and why that is a finding

Five direct dependencies for what looks like two:

- `apalis` --- the runtime.
- `apalis-sqlite` --- the storage backend.
- `apalis-core` --- because `apalis` re-exports only a curated `prelude` and
  does *not* re-export `apalis_core`, yet `BoxDynError`, `Status` and `Worker`
  appear in types callers must name.
- `apalis-codec` --- `apalis-sqlite` does not re-export it, and it is required
  to name the codec used for task arguments.
- `sqlx` --- `SqliteStorage::new` takes a `sqlx::SqlitePool` in its public API.

Three of the five exist purely to name types that the primary crates expose but
do not re-export. None adds a crate to the tree (all pin versions already
resolved), so the cost is declaration noise rather than weight. But it is a
symptom worth naming: the public API surface leaks types from crates the
facade does not re-export.

Worse, the versions do not line up. `apalis` is `1.0.0-rc.9`, `apalis-sqlite`
is `1.0.0-rc.8`, and `apalis-codec` is `0.1.0-rc.9` --- three separately
versioned crates from one project on two different release lines. In practice
the API surfaces were compatible and nothing had to be worked around, but this
is a distinct risk from ordinary rc status. This workspace already runs three
rc dependencies in production (`async-graphql 8.0.0-rc.5`, `neo4rs
0.9.0-rc.9`, `ort 2.0.0-rc.13`), so rc alone is not disqualifying; skew across
co-released crates is a different exposure, because a patch to one may require
a coordinated bump of the others.

## Is the Tower integration real leverage?

Yes, with one caveat that matters specifically here.

It is real in the sense that `.retry(RetryPolicy::retries(5))` is a Tower
layer wrapping the handler service, not a bespoke retry loop --- and
`tower 0.5.3` unified, no second copy in the lockfile. Retries, timeouts,
concurrency limits and panic-catching are all layers over a
`Service<Task<Args, Ctx, IdType>>`. Spike 1 hand-wrote `catch_unwind` and a
`biased` select to get two of those four.

The caveat is that **`tower/limit` is the wrong tool for this workload.**
Concurrency here is bounded downstream by the single ONNX thread that
`EmbedderHandle` fronts, not by a Tower semaphore. Adding a concurrency layer
would cap something that is already capped, at a different and less accurate
number. The `limit` feature is left off for that reason.

## What spike 1 has that apalis does not

Thinking about this honestly rather than concluding "nothing":

**Enqueue latency.** Spike 1's enqueue is a `try_send` on an in-process mpsc
channel --- nanoseconds, no I/O, no serialization. Here it is a serialization
plus a SQLite write. For a job enqueued and executed on the same machine, that
is real overhead bought for durability that is only sometimes needed.

**No new datastore.** This deployment is Neo4j-only. A SQLite file is a second
place where state lives, with its own file lifecycle, its own migrations, and
its own failure modes. The counter-argument is that a queue's contents are
transient by construction --- the file is closer to a lockfile than to a
database, and holds no truth about interviews --- but it is still one more
thing in the deployment.

**Dependency weight.** +59 crates against +2.

**Version churn.** Three rc crates on two release lines, versus code that is
ours and changes when we change it.

**Fit with the ONNX constraint.** Neutral rather than negative. apalis's
`Service`-shaped worker model does not fight the dedicated-thread embedder,
but it does not help either --- worker concurrency past ~2 buys nothing in
both designs, because everything funnels through one ONNX thread regardless.

## A real bug found in the ack path

Worth recording because it is a property of apalis, not of the test.

Two tests failed with the job stuck in `Running` rather than reaching `Done`.
The cause: the handler called `worker.stop()` before returning `Ok(())`.

Execution and acknowledgement are **two separate writes**. The handler
returning is not the same event as the result being written back to storage.
Calling `stop()` from inside a handler tears the worker down in the window
between them, and the ack is never written --- the job side effect happens,
and the row stays `Running` forever. That is a stranded job, which is
precisely the condition at-least-once delivery exists to prevent.

Confirmed by elimination. Three plausible hypotheses died first:

1. `.enable_tracing()` --- the most visually obvious difference between the
   passing and failing tests. Added it to the failing test; still failed.
2. `.retry(RetryPolicy)` --- the more sophisticated guess, that a Tower layer
   must observe completion before the result propagates outward. Added it;
   still failed.
3. "The ack is merely delayed, so poll for it." Waited 10 seconds; still
   `Running`. Nothing was slow --- something was never written.

Removing `stop()` from the handler and stopping the worker from outside made
the test pass in **0.11s**. The speed of that pass is the evidence.

The passing retry test was a red herring twice over: it has both
`.enable_tracing()` and `.retry()`, neither of which mattered. It passes
because its handler fails twice first, so earlier attempts force state writes
before the `stop()` on the third.

**Operational consequence.** Nothing in production should call `stop()` from
inside a handler. Shutdown must be driven from outside the worker, after
`axum::serve` returns --- which is what `main.rs` does here.

## Durability, demonstrated rather than asserted

The SQLite file lives at a configured path, created on first run, with
migrations applied idempotently at startup. Three tests cover this:

- `creates_database_file_on_first_run`
- `setup_is_idempotent` --- running setup twice is safe
- `queued_jobs_survive_a_restart` --- jobs enqueued, pool closed, pool
  reopened, jobs still there

That last one is the capability spike 1 structurally cannot have. Its
`InProcessBackend` drops queued-but-undequeued jobs on shutdown by
construction.

## Neo4j as an apalis backend

This was the open question. The answer is much more favourable than expected,
and my earlier reasoning about it was wrong.

### What a backend must implement

In `1.0.0-rc.9` there is **no `Storage` trait** --- that is a change from
older apalis versions and invalidates most advice written about them. The seam
is `apalis_core::backend::Backend`, and it is small:

```
type Args; type IdType: Clone; type Context: Default; type Error;
type Stream;  // Stream<Item = Result<Option<Task<..>>, Error>>
type Beat;    // Stream<Item = Result<(), Error>>
type Layer;

fn heartbeat(&self, worker: &WorkerContext) -> Self::Beat;
fn middleware(&self) -> Self::Layer;
fn poll(self, worker: &WorkerContext) -> Self::Stream;
```

Three methods. Everything else --- `FetchById`, `Update`, `ListTasks`,
`ListQueues`, `Metrics`, `BackendExt` --- is an **optional** trait implemented
only for the capabilities you want. A minimal backend is genuinely minimal.
`apalis-core` also ships `backend/impls/memory.rs`, an in-process backend, and
a composable `poll_strategy` module (`interval`, `backoff`, `stream`,
`future`), so polling cadence is not something a backend author reinvents.

### The claim query, and why the usual objection does not apply

The standard objection to a graph-database queue is that Neo4j has no
`SELECT ... FOR UPDATE SKIP LOCKED`, so N workers contend on the head of the
queue: one acquires the write lock and the others block rather than skipping
to the next available row.

**apalis-sql does not use `SKIP LOCKED`.** Grepping the whole crate for
`SKIP LOCKED` and `FOR UPDATE` returns nothing. The entire storage contract is
externalized as 24 `.sql` files under `queries/`, and the claim is one
statement:

```sql
UPDATE Jobs
SET status = 'Queued', lock_by = ?1, lock_at = strftime('%s','now')
WHERE ROWID IN (
    SELECT ROWID FROM Jobs
    WHERE job_type = ?2
      AND ((status = 'Pending' AND lock_by IS NULL)
           OR (status = 'Failed' AND attempts < max_attempts))
      AND (run_at IS NULL OR run_at <= strftime('%s','now'))
    ORDER BY priority DESC, run_at ASC, id ASC
    LIMIT ?3
)
RETURNING *
```

A conditional bulk update with a `RETURNING` clause. Cypher expresses this
directly: `MATCH` on label and status predicates, `ORDER BY` / `LIMIT`, `SET`
the lock fields, `RETURN` the claimed nodes. Neo4j write locks are acquired
per-node within the transaction and the whole statement is atomic, so two
workers cannot both claim the same node. The loser blocks briefly, re-reads,
and takes the next batch --- which is the same behaviour the SQLite backend
has, since `UPDATE ... WHERE ROWID IN (SELECT ...)` under SQLite's
single-writer model serializes writers too.

The reaper is equally translatable --- a join from jobs to a `Workers`
heartbeat table, re-enqueueing anything whose worker has not been seen within
a timeout:

```sql
UPDATE Jobs SET status = "Pending", lock_by = NULL, lock_at = NULL,
                attempts = attempts + 1, ...
WHERE id IN (
    SELECT Jobs.id FROM Jobs INNER JOIN Workers ON lock_by = Workers.id
    WHERE (status = "Running" OR status = "Queued")
      AND strftime('%s','now') - Workers.last_seen >= ?1
      AND Workers.worker_type = ?2
);
```

In Cypher that is a `MATCH (j:Job)-[:LOCKED_BY]->(w:Worker)` traversal --- the
one shape a graph database is unambiguously *better* at than SQL.

### Verdict and effort

**Achievable with caveats.** The `Backend` trait is small, the reference
implementation is legible SQL rather than hidden Rust, and the two hard
queries both translate. Estimate: **3--5 days** for a working implementation
with claim, ack, retry, reschedule and reaper; longer to trust it under
concurrency.

Likely failure modes of a hand-rolled version: contention behaviour under many
workers (untested until it hurts), clock skew between the reaper's notion of
"now" and the workers', a retry/attempt-counter race if ack and reaper both
touch a row, and no equivalent of the SQL backends' migration discipline as
the schema evolves.

### Ranked, across the three spikes

1. **Spike 1's `QueueBackend` --- easiest by a wide margin.** Four methods,
   and `start` hands you a `JobContext` and `Registry` and says "go" --- it
   does not dictate polling, so you can shape the loop to Neo4j. Its
   `Envelope { id, name, args: JsonValue }` was deliberately shaped as a
   persistable row and `enqueue` is already async-signatured for exactly this.
   The catch: the trait demands nothing about retries, leases, or reaping, so
   all of that correctness burden is yours, and any bug is yours to find.

2. **apalis `Backend` --- harder, but the burden is bounded.** More to
   implement, because it supports more; but you inherit the worker machinery,
   the Tower layers, and 24 reference queries that document the exact
   semantics you must reproduce. A correctness bug lives in your ~400 lines
   rather than diffused through the system.

3. **Loco --- do not.** `create_queue_provider` is a `match` over three
   hardcoded variants in a crate you do not control. There is no trait to
   implement; you would fork Loco and carry that fork. Its queue is also
   entangled with the `worker` feature's `dep:sqlx`, so "Neo4j-backed Loco
   jobs" still drags SQL in.

### The recommendation on Neo4j specifically

Do not build it. Not because it is infeasible --- it is more feasible than the
usual `SKIP LOCKED` objection suggests --- but because SQLite already works,
and the stated escape hatch for scale is Redis. `apalis-redis` is a
first-party crate on the same release line, so that path is a storage swap at
one wiring site in `main.rs`, with job definitions, retry policies and
resolvers untouched.

A Neo4j backend would be 3--5 days of writing a queue in order to avoid
adding a file to a deployment. The graph already answers "which statements
lack embeddings", which is a cheaper recovery mechanism than a durable queue
for an idempotent job.

### When SQLite runs out

Not throughput. The embedding path is bounded to roughly one job at a time by
the ONNX thread, and SQLite handles orders of magnitude more than that. The
trigger is **multi-process**: SQLite cannot serve workers on a different
machine from the API. If the worker pool moves to the GPU box while the API
runs elsewhere, that is the day Redis is needed --- a deployment-shape change,
not a load threshold.

## Recommendation

Adopt apalis with the SQLite backend.

The case rests on capability, maintenance *and* size, which all point the same
way. Durability, retries, scheduling and orphan recovery are present and tested
here, absent there, and each is a thing we would otherwise write and own ---
and the implementation that has them is the smaller one, 387 code lines against
641, because 471 lines of spike 1 are queue machinery the dependency replaces.

The +59 crates are the only real price, and they are the whole price: the three
extra `Cargo.toml` declarations plus the rc skew are the sharp edges. That is
the trade to weigh --- 59 crates against 471 lines of hand-owned queue
machinery --- not a line count that was never a wash.

Spike 1 is not wasted. It identified the correct seam independently --- a
persistable-shaped `Envelope` behind a swappable backend trait is what apalis
also converged on, which is evidence the design was right. What it could not
supply is maintained implementations on the other side of that seam.
