# Spike: migrating `packages/api` to Loco

**Recommendation: don't.** Not because it fails --- it works, and the code in this
worktree compiles and runs --- but because what it delivers is not what the
framework is for.

## The headline, before the reasoning

The migration is **technically viable**. I expected the SeaORM coupling to be the
wall and it is not. Loco 1.1.0 gates its entire SQL story behind a Cargo feature,
and switching that feature off is a supported, first-class configuration rather
than a hack. With `default-features = false, features = ["cli"]`:

- `sea-orm` and `sqlx` do not appear in `Cargo.lock` at all. Verified, not assumed.
- No version conflicts. `axum`, `tower`, `tokio`, `rustls` and `http` all unify to
  single versions shared with the existing tree.
- async-graphql mounts without friction.
- The whole thing typechecks and boots.

So the question is not "can we?" It is "what did we get?" And the answer to that
is: a CLI, a YAML config loader, a middleware stack, and a scheduler --- in
exchange for 165 additional dependencies, a second configuration system running
alongside the one we already have, and the loss of compile-time dependency
injection on every handler.

That is a bad trade *for this service*. The reasoning follows.

## What Loco actually is

Loco bills itself as "the one-person framework for Rust" and is transparently
modeled on Rails. It is not a routing library with extras; it is an application
lifecycle framework that happens to route with Axum. The pieces:

- **`app::Hooks`** --- the trait your application type implements. It owns boot,
  routing, middleware, initializers, workers, tasks, and shutdown.
- **`app::AppContext`** --- the DI container, threaded to every handler as Axum
  state.
- **`boot`** --- the lifecycle: load config, build context, converge migrations,
  register workers, assemble routes, serve.
- **`controller`** --- `AppRoutes` / `Routes`, a thin declarative wrapper over
  Axum's router.
- **`bgworker`** --- a job queue with Postgres, SQLite and Redis backends.
- **`scheduler`** --- cron, via `tokio-cron-scheduler`.
- **`mailer`** --- SMTP via `lettre`, with Tera templates.
- **`storage`** --- object storage via `opendal` (S3, Azure, GCS, fs, memory).
- **`cache`** --- Moka in-memory or Redis.
- **`cli`** --- `start`, `routes`, `middleware`, `task`, `scheduler`, `generate`,
  `doctor`, `version`, `watch`.
- **`loco-gen`** --- code generators, the Rails `scaffold` analogue.

The framing worth holding onto: Loco's value proposition is *coherence across a
lot of subsystems*. It pays off when you are using many of them and want them to
share configuration, conventions and a lifecycle. It is not designed to be, and
does not reward being, a router with a nice CLI.

## The SeaORM / Neo4j impedance mismatch

This was the central question, and the honest answer is more interesting than
"it's incompatible."

### Where the coupling actually lives

The SQL assumption is real but it is **cleanly quarantined behind one Cargo
feature**, `with-db`. Concretely, in Loco 1.1.0:

`loco_rs::app::AppContext` --- the `db` field is conditional:

> `#[cfg(feature = "with-db")] pub db: DatabaseConnection`

With the feature off the field does not exist. `AppContext::builder` has two
signatures, and the no-db one simply drops the `DatabaseConnection` parameter.
This is not a degraded mode; it is a separately compiled shape.

`loco_rs::boot` --- `create_app` is overloaded the same way. The `with-db` version
is `create_app::<H, M: MigratorTrait>`; the `#[cfg(not(feature = "with-db"))]`
version is `create_app::<H>`. That second overload is what makes the entire spike
possible. Without it, adopting Loco would require supplying a
`sea_orm_migration::MigratorTrait` implementation --- a type this project has no
coherent way to produce, because there is no relational schema to migrate.

`loco_rs::app::Hooks` --- three methods are behind the feature: `truncate`, `seed`
and `dump`. All three vanish in a no-db build. This is the one place where
dropping SQL genuinely *reduces* the surface you must implement rather than
leaving an empty stub behind.

`loco_rs::config::Config` --- the `database` field is likewise
`#[cfg(feature = "with-db")]`. The YAML schema changes shape with the feature.

`loco_rs::cli` --- the `db` subcommand tree is gated. The no-db binary's `--help`
does not list it. (Confirmed by running it.)

### The subsystem-by-subsystem accounting

This is the question the brief asked me to answer from source rather than
assumption. Reading `bgworker/mod.rs`, `cache/`, `storage/`, `scheduler.rs`,
`mailer/` and `controller/middleware/`:

| Subsystem | Survives without SQL? | Notes |
|---|---|---|
| Routing (`controller`) | **Yes**, fully | Pure Axum underneath. No DB reference anywhere. |
| Middleware stack | **Yes**, fully | `default_middleware_stack` has no DB coupling. |
| Config loader | **Yes** | Minus the `database` section. |
| CLI | **Yes** | Minus `db` subcommands. |
| Scheduler | **Yes** | `tokio-cron-scheduler`; runs shell commands or Loco tasks. |
| Tasks | **Yes** | Plain trait, no persistence. |
| Cache | **Yes** | `cache_inmem` (Moka) needs no server. |
| Storage | **Yes** | `opendal`; unrelated to SeaORM. |
| Mailer | **Partly** | Sends fine. But `MailerWorker` is registered through the queue, so deferred sending needs a queue provider. |
| **Background jobs** | **No** | This is the real casualty. See below. |
| ORM / migrations / seeding | **No** | Gone entirely, which is correct --- we don't want them. |

### The one subsystem that genuinely dies

**`bgworker` is unusable.** Not because of SeaORM --- it does not use SeaORM ---
but because every queue *backend* Loco ships is a SQL or Redis server:

`create_queue_provider` in `bgworker/mod.rs` matches on exactly three config
variants: `QueueConfig::Redis`, `QueueConfig::Postgres`, `QueueConfig::Sqlite`.
The Postgres and SQLite providers talk `sqlx` directly (the `worker` feature is
`["dep:sqlx", "dep:ulid"]`, independent of `with-db`). There is no in-memory
provider, and `QueueProvider` is a trait behind `Queue(Arc<dyn QueueProvider>)` ---
so a custom Neo4j-backed provider is *implementable*, but that means writing a
durable job queue on top of Cypher, which is a project, not a migration step.

The available escape is `workers.mode: ForegroundBlocking`, which runs jobs
inline and gives up durability entirely. That is what the spike's config uses.

This matters directly: it is the same problem space the parallel bgworker spike
is investigating. Loco does not solve it. Adopting Loco to get background jobs
and then discovering you must supply the queue yourself is the worst of both
outcomes --- you take the framework's weight and still write the hard part.

### The `AppContext` question, answered

**Can `AppContext` carry `Arc<Graph>` and `Arc<EmbedderHandle>`?** Yes, via
`shared_store`:

> `pub shared_store: Arc<SharedStore>` --- backed by
> `DashMap<TypeId, Box<dyn Any + Send + Sync>>`

This is the same type-keyed heterogeneous-map trick async-graphql's `.data()` and
`http::Extensions` use --- `TypeId` as the key, `Box<dyn Any>` as the value, a
downcast on the way out. Worth noticing: three independent DI mechanisms in this
one process end up implemented identically. That is not a coincidence, it is what
you get when a language without runtime reflection needs heterogeneous storage.

But note the cost, because it is not free:

1. **It is untyped at the boundary.** `insert` and `get_ref` are not checked
   against each other. A handler that reads a type nothing inserted panics at
   runtime. The current design --- `State(schema): State<AppSchema>` --- is a
   compile error in the same situation.

2. **`Initializer::before_run` returns `Result<()>`, not `Result<T>`.** There is
   no channel by which an initializer *returns* a value. The only way to get the
   Neo4j handle out of boot is to mutate `ctx.shared_store` as a side effect. The
   dependency graph of the boot sequence becomes implicit.

3. **Every handler pays a lookup.** A `TypeId` hash into a `DashMap`, a downcast,
   and a clone --- per request, per resource. Cheap in absolute terms, but it
   replaces something that cost nothing and was checked by the compiler.

### The routing question, answered

**Does Loco assume REST-shaped handlers?** No. `Routes::add` takes a bare
`axum::routing::MethodRouter<AppContext>`, and the verb helpers take
`axum::handler::Handler<T, AppContext>`. "Controller" is vocabulary, not
constraint. async-graphql's `GraphQLRequest` / `GraphQLResponse` extractors drop
straight in. The spike mounts `GET,POST /graphql` and it works.

**However** --- `AppRoutes::to_router` ends in `app.with_state(ctx)`. The Axum
state type is *fixed* to `AppContext`. This is the single largest forced change
to existing code: `main.rs`'s `.with_state(schema)` cannot survive, and every
handler signature changes from `State<AppSchema>` to `State<AppContext>` plus an
in-body lookup. Not difficult, but it is a real regression in type safety applied
uniformly across the handler surface.

### The startup-sequence question, answered

Today `main.rs` is 155 lines that do everything in visible order: install the
crypto provider, init tracing, load dotenv, connect with retry, ensure indexes,
load the embedder, build the schema, build the router, serve. You read it top to
bottom and you know what happens when.

Under Loco that sequence scatters:

- **rustls crypto provider** --- currently the *first* statement in `main`, before
  anything can touch TLS. Loco owns `main` now. The earliest available hook is
  inside `Hooks`/`Initializer`, which runs after Loco has parsed config. Still
  early enough in practice, but the ordering guarantee weakens from structural to
  incidental.
- **dotenv** --- must stay in `main` *before* `cli::main`, because Loco reads
  `LOCO_ENV` during `Environment::resolve` and offers no earlier hook.
- **Neo4j connect with retry** --- moves into `Initializer::before_run`. The
  `tokio-retry` backoff for the Docker startup race survives untouched, which is
  itself telling: that module never knew about the web layer, so nothing about it
  is framework-shaped.
- **Index-ensuring Cypher** --- same initializer. Note the irony: this is
  *exactly* the job Loco's migration system exists to do, and we cannot use it,
  because migrations are `MigratorTrait` and `MigratorTrait` is SeaORM.
- **Graceful shutdown** --- Loco's default `Hooks::serve` already does this, and
  its `shutdown_signal` handles SIGTERM as well as Ctrl-C, which is a genuine
  (small) improvement over the current Ctrl-C-only handler.

### The configuration split --- the finding I did not anticipate

`loco_rs::config::Config` is a closed struct: `logger`, `server`, `queue`,
`workers`, `mailer`, and conditionally `database`. There is **no extension field,
no passthrough map, no user-defined section.**

So the Neo4j URI, username, password and database name cannot live in Loco's
config. They stay in `std::env`, read directly, sitting beside a YAML config
system that does not know they exist.

The result is that adopting Loco does not *replace* this project's configuration
mechanism --- it **adds a second one**, and the single most important piece of
configuration in the service is in the one Loco doesn't own. For a framework
whose pitch is coherence, this is close to a direct hit on the value proposition.

## Dependency analysis

Measured in this worktree, not estimated.

**No conflicts.** Everything unifies:

| Crate | Project wants | Loco 1.1.0 wants | Resolved |
|---|---|---|---|
| `axum` | `0.8` | `0.8.1` | **0.8.9**, single copy |
| `tower` | `0.5.3` | `0.5` | **0.5.3**, single copy |
| `tower-http` | `0.6.8` | `0.6.8` | **0.6.11**, single copy |
| `tokio` | `1` (full) | `1.45` | **1.53.1**, single copy |
| `rustls` | `0.23` + aws-lc-rs | `0.23` + ring (optional, `redis_tls` only) | **0.23.43**; the ring provider is not pulled |
| `http` | `1.4.0` | transitive | **1.5.0**, single copy |
| `chrono` | `0.4` | `0.4` + serde | **0.4**, unified |
| `async-graphql` | `8.0.0-rc.5` | — | untouched; Loco has no GraphQL opinion |
| `neo4rs` | `0.9.0-rc.10` | — | untouched |
| `sea-orm` | — | 2.0 (`with-db` only) | **absent** |
| `sqlx` | — | 0.9 (`with-db`/`worker`) | **absent** |

Two notes on the release candidates. `async-graphql 8.0.0-rc.5` was the risk I
most expected to bite, and it does not: Loco has no GraphQL integration at all,
so there is nothing to conflict with. It talks to Axum, and async-graphql-axum
talks to the same Axum. Same for `neo4rs 0.9.0-rc.10`. Being on RCs is a real
project risk, but it is **orthogonal** to this decision --- Loco neither worsens
nor improves it.

**MSRV**: Loco 1.1.0 declares `rust-version = "1.94"` (raised for SeaORM 2.0 +
sqlx 0.9 --- amusingly, a floor set by dependencies we don't compile). This
project is on `nightly-2026-05-02` / rustc 1.97.0-nightly. Clears it.

**The cost, measured:**

- **+116 crates** added to `Cargo.lock`.
- **491 → 656** unique resolved dependencies for `auohp-api` (+34%).
- **Debug binary 92M → 138M** (+50%).
- **Duplicate `tera`**: both 1.20.1 and 2.3.0 land in the tree, from Loco's own
  subtree. Two copies of a template engine, for a service that renders no
  templates.

Some of what arrives: `lettre` (SMTP), `opendal` (object storage), `argon2`
(password hashing), `jsonwebtoken` is off but `validator`, `tera` ×2,
`tokio-cron-scheduler`, `english-to-cron`, `axum-client-ip`, `cargo-lock`,
`notify`, `zstd`, `moka`, `scraper`-adjacent bits. This service sends no mail,
stores no objects, hashes no passwords, and renders no templates.

## The migration in phases

If it were to happen, this is the actual shape. The spike executed phases 1--4.

**Phase 1 --- crate restructuring.** Loco's CLI entrypoint needs the `Hooks` impl
importable from a binary, so the crate needs a library target. `packages/api` is
currently binary-only with private `mod` declarations in `main.rs`. Add `lib.rs`,
make the modules public, repoint the binary at the lib. Structural, mechanical,
and independent of any behavioral change --- but it is a change to how the crate
is organized that exists purely to satisfy the framework.

**Phase 2 --- the `Hooks` implementation.** Boot, routes, initializers,
after_routes, plus the inert `connect_workers` and `register_tasks` the trait
requires whether or not you have workers or tasks.

**Phase 3 --- resource relocation.** Neo4j connect + retry, index Cypher, embedder
load, schema build all move into an `Initializer`, depositing results on
`shared_store`.

**Phase 4 --- handler rewrite.** Every handler changes from typed `State<T>` to
`State<AppContext>` + runtime lookup. Config YAML per environment.

**Phase 5 --- not attempted.** Move CORS and body limits from code into YAML;
decide the queue story (there isn't one without new infrastructure); port the
Dockerfile to Loco's config-directory expectation; adopt or reject `loco-gen`
scaffolding conventions.

**Honest cost estimate:** phases 1--4 are roughly a day of focused work, most of
it mechanical. This spike got there in one session. Phase 5 plus operational
shakeout (Docker, environments, the config split, deciding what to do about jobs)
is realistically another two to three days. **Call it 3--4 days total.**

The cost is not the problem. The cost is low. That is *why* the recommendation
needs to rest on something other than difficulty.

## The tension with "many sharp tools"

The project has a documented principle: prefer many sharp purpose-built tools over
one blunt abstraction; quarantine each behind a thin adapter; unify at the output
rather than the runtime. A framework migration is in obvious tension with it, and
the brief rightly asks me not to treat that as automatically decisive. So, fairly:

**The case that Loco is compatible with the principle.** Loco is unusually
unblunt for a framework. Its subsystems are Cargo features, and turning them off
genuinely removes them --- `with-db` off means SeaORM is not in the binary, not
merely unused. `Routes::add` hands you raw Axum. `after_routes` hands you the
router. The escape hatches are real and load-bearing, not decorative. One could
argue Loco *is* a bundle of sharp tools with a shared lifecycle, which is closer
to the principle than to its opposite.

**The case that it isn't.** The principle's second clause is "quarantine each
behind a thin adapter," and Loco inverts that: it is the thing doing the
quarantining, and your code adapts to *it*. Concretely --- `AppContext` is the
state type whether you want it or not; `Hooks` demands implementations for
subsystems you don't use; the config schema is closed to your most important
configuration. And the principle's third clause, "unify at the output, not the
runtime," is precisely what Loco does not do: it unifies at the runtime. That is
its entire premise.

**Where I land.** The principle is not the reason to decline. The reason to
decline is narrower and more concrete: **of Loco's ten subsystems, this service
would use routing, middleware, config and CLI --- and the config one is broken for
us**, because the Neo4j connection details can't live there. The two subsystems
that would most justify the framework's weight, ORM/migrations and background
jobs, are exactly the two unavailable. You would be adopting an application
framework and using its router.

That is not an argument from principle. It is an argument from the ledger.

## Recommendation

**Do not migrate.** Reasoning, in order of weight:

1. **The surviving value is small and separately obtainable.** The real wins are
   a CLI, SIGTERM handling, a config file, and a declarative middleware stack.
   Each is available standalone --- `clap`, ten lines of `tokio::signal`,
   `figment` or `config`, and `tower-http` (already a direct dependency). None
   requires 116 crates.

2. **The subsystems that would justify a framework are unavailable.** Migrations
   are SeaORM, so the index-ensuring Cypher --- the one piece of genuine migration
   logic here --- cannot use them. Jobs need a SQL or Redis server this project
   does not run. See also: the parallel bgworker spike, which Loco does not help.

3. **It adds a configuration system rather than replacing one.** `Config` is
   closed, so `NEO4J_URI` and friends stay in `std::env`. Two mechanisms where
   there was one, with the most important values in the one Loco doesn't manage.

4. **It costs compile-time DI.** `State<AppSchema>` becomes `State<AppContext>`
   plus a `TypeId` downcast that panics if boot ordering is wrong. Trading a
   compile error for a runtime panic, on every handler, is a regression the
   framework does not compensate for here.

5. **Startup legibility drops.** 155 explicit, ordered lines become a sequence
   distributed across `Hooks` methods and framework internals. For a service whose
   startup is genuinely subtle --- crypto provider ordering, connection retry,
   idempotent index creation --- that legibility is worth something.

None of these is fatal alone. Together they describe adopting a framework and
using a tenth of it.

**What I'd do instead**, if the motivating pain is real: take the pieces. Add
`clap` for a subcommand or two. Add SIGTERM to the existing shutdown handler
(genuine bug-adjacent gap, ~5 lines). If configuration is sprawling, add `figment`
--- and get the Neo4j settings in the config file, which Loco cannot do. Keep
`main.rs` as the readable, ordered thing it is.

## What would change this answer

Revisit if any of these become true:

- **A SQL database enters the stack.** If a relational store lands next to Neo4j
  --- users, sessions, audit, billing --- `with-db` switches on, SeaORM becomes an
  asset, migrations become usable, and the ledger flips. This is the single most
  likely trigger.
- **Postgres or Redis is deployed for any reason.** That unlocks `bgworker`
  wholesale, which is the largest subsystem currently forfeited and the one with
  real demand behind it.
- **The service grows the subsystems Loco bundles.** Transactional email, object
  storage for media, scheduled reprocessing, auth with sessions. Two or three of
  these together, and Loco's coherence starts earning its weight.
- **Loco's `Config` gains a user-extension section.** This would eliminate finding
  #3 outright. Worth watching; it is a plausible upstream change.
- **A `QueueProvider` implementation for Neo4j** (or an in-memory one upstream)
  appears. `Queue` is already `Arc<dyn QueueProvider>`, so the seam exists.
- **Team size grows past one or two.** Loco's real pitch is convention over
  configuration, and conventions pay off across people, not within one person's
  head. That argument is weak at current size and gets stronger with headcount.

Absent those, the current architecture --- Axum plus purpose-built libraries,
unified at the output --- is the better fit, and it is better for reasons specific
to this service rather than by appeal to principle.

## Appendix: what's in this worktree

The spike code is left in place, compiling, for side-by-side comparison.

- `packages/api/src/loco_app.rs` --- the `Hooks` implementation. Heavily commented
  with the mechanism notes.
- `packages/api/src/bin/loco_spike.rs` --- Loco entrypoint. Compare against
  `main.rs`.
- `packages/api/src/lib.rs` --- library target added for Phase 1.
- `packages/api/config/development.yaml` --- minimum viable Loco config.
- `packages/api/src/main.rs` --- modified only to consume the lib rather than
  redeclare its modules. The original architecture is otherwise intact and still
  builds as the default binary.

Both binaries build. `cargo check -p auohp-api --all-targets` is clean.
