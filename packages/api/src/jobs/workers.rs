//! Registration and lifecycle for every background worker the process runs.
//!
//! # The problem this solves
//!
//! Running one worker from `main` takes four separated touchpoints: build its
//! dependencies, clone a shutdown receiver, spawn the run-future, and --- a
//! hundred lines later, past the entire HTTP server --- join its handle and
//! match on the outcome. Two of those four are pure ceremony that carry no
//! job-specific information except a name in a log line.
//!
//! A second job kind therefore does not append a block; it threads a second
//! strand through code that is not about jobs at all. This module collapses
//! those four touchpoints to two, so adding a job kind is one [`Workers::add`]
//! call and no other edit anywhere in `main`.
//!
//! # Why the handles collect but the futures do not
//!
//! Each `run_worker_*` returns `impl Future`, which is an *opaque type*: the
//! compiler mints one anonymous type per function, so two workers' futures have
//! two unrelated types and cannot share a `Vec`. The reflex is
//! `Vec<Box<dyn Future<..>>>`.
//!
//! That is unnecessary, because `tokio::spawn` is already an erasure. It
//! accepts any `Future<Output = T> + Send + 'static` and returns
//! `JoinHandle<T>` --- and `JoinHandle<anyhow::Result<()>>` is one concrete,
//! non-generic type no matter which worker produced it. Spawning at
//! registration time rather than storing the future means the heterogeneity is
//! gone before anything needs to be collected.
//!
//! This is the same maneuver as `JobQueue` holding `{pool, config}` rather than
//! a built `SqliteStorage`, one layer up: when a type varies inconveniently,
//! hold the value *downstream* of the step that erases it.

use tokio::sync::watch;
use tokio::task::JoinHandle;
use tracing::{error, info};

/// Owns every background worker's shutdown channel and join handle.
pub struct Workers {
    /// Fires once; every worker observes the same edge.
    ///
    /// `watch` rather than `oneshot` because a `oneshot` receiver can only be
    /// awaited once by one owner, and shutdown has N observers.
    shutdown: watch::Sender<bool>,

    /// Holds the channel open when no worker is registered.
    ///
    /// `watch::Sender::send` fails if every receiver has been dropped. With no
    /// workers --- or with all of them already exited --- that failure is
    /// meaningless rather than informative, and keeping one receiver parked
    /// here means [`shutdown_trigger`](Self::shutdown_trigger) never has to
    /// distinguish the two cases.
    _keepalive: watch::Receiver<bool>,

    /// One entry per spawned worker, paired with the name used in its exit log.
    handles: Vec<(&'static str, JoinHandle<anyhow::Result<()>>)>,
}

impl Default for Workers {
    fn default() -> Self {
        Self::new()
    }
}

impl Workers {
    /// An empty registry with a fresh shutdown channel.
    pub fn new() -> Self {
        let (shutdown, _keepalive) = watch::channel(false);
        Self {
            shutdown,
            _keepalive,
            handles: Vec::new(),
        }
    }

    /// Spawn one worker and keep its handle.
    ///
    /// `name` appears in the worker's exit log line and nowhere else; it does
    /// not need to match the apalis queue name or the `WorkerBuilder` id.
    ///
    /// # Why this takes a closure rather than the worker's arguments
    ///
    /// Every `run_worker_*` has its own arity and its own dependency struct, so
    /// there is no argument list this method could name that would fit all of
    /// them. It takes the *partial application* instead: the caller closes over
    /// whatever that particular worker needs, and this method supplies the one
    /// argument they genuinely have in common.
    ///
    /// ```ignore
    /// workers.add("embedding", |shutdown| {
    ///     jobs::queue::run_worker(pool.clone(), config.clone(), deps, shutdown)
    /// });
    /// ```
    ///
    /// `FnOnce` rather than `Fn` is deliberate and load-bearing: it is invoked
    /// exactly once, so the closure may *move* its captures. `EmbedDeps` is
    /// consumed rather than cloned at the call site.
    pub fn add<F, Fut>(&mut self, name: &'static str, spawn: F)
    where
        F: FnOnce(ShutdownSignal) -> Fut,
        Fut: std::future::Future<Output = anyhow::Result<()>> + Send + 'static,
    {
        let signal = ShutdownSignal(self.shutdown.subscribe());
        self.handles.push((name, tokio::spawn(spawn(signal))));
    }

    /// How many workers are registered.
    pub fn len(&self) -> usize {
        self.handles.len()
    }

    /// Whether any worker is registered.
    pub fn is_empty(&self) -> bool {
        self.handles.is_empty()
    }

    /// A one-shot callback that tells every worker to stop.
    ///
    /// Returned as a closure rather than exposed as a `stop(&self)` method
    /// because axum's `with_graceful_shutdown` wants to own the trigger and
    /// fire it at its own moment, inside a future that has already moved.
    ///
    /// # Why the `'static` bound is load-bearing
    ///
    /// Without it this does not compile, and the reason is a genuine trap:
    /// return-position `impl Trait` captures the lifetimes of all input
    /// parameters by default, so `-> impl FnOnce()` is implicitly
    /// `-> impl FnOnce() + '_` and borrows `self` --- even though the body only
    /// uses an owned clone of the sender. That borrow then outlives this call
    /// and collides with [`join_all`](Self::join_all), which takes `self` by
    /// value.
    ///
    /// `+ 'static` asserts what is actually true here --- the closure captures
    /// nothing borrowed --- and severs the tie. Rust 2024's `use<>` syntax
    /// spells the same thing more precisely ("capture no generic parameters or
    /// lifetimes"); `'static` is the more familiar spelling of the same
    /// guarantee.
    pub fn shutdown_trigger(&self) -> impl FnOnce() + 'static {
        let tx = self.shutdown.clone();
        move || {
            // Fails only when every receiver is gone, which itself means every
            // worker has already exited. Not a condition worth reporting.
            let _ = tx.send(true);
        }
    }

    /// Wait for every worker to drain, logging each outcome.
    ///
    /// Takes `self` by value: joining consumes the registry, which makes
    /// "spawn more workers after shutdown" a compile error rather than a
    /// runtime surprise.
    ///
    /// # The double unwrap
    ///
    /// `JoinHandle` yields `Result<T, JoinError>` --- did the task panic? ---
    /// wrapping the task's own `anyhow::Result<()>` --- did the work fail? Two
    /// independent failure modes, so two layers, and the three-arm match names
    /// all three outcomes rather than flattening them.
    ///
    /// # Why sequential rather than `join_all`
    ///
    /// Every worker is already draining concurrently the moment the shutdown
    /// signal fires; awaiting them in registration order costs no wall-clock
    /// time and keeps the shutdown log in a stable, readable order.
    pub async fn join_all(self) {
        for (name, handle) in self.handles {
            match handle.await {
                Ok(Ok(())) => info!(worker = name, "background worker stopped cleanly"),
                Ok(Err(e)) => error!(worker = name, error = %e, "background worker failed"),
                Err(e) => error!(worker = name, error = %e, "background worker panicked"),
            }
        }
    }
}

/// A worker's view of the shutdown channel.
///
/// A newtype rather than a bare `watch::Receiver<bool>` so that `run_worker`
/// signatures keep taking a plain `Future<Output = ()>`. No worker should have
/// to know that a `watch` channel drives it, or repeat the
/// `let _ = rx.changed().await` incantation --- that is this module's business,
/// and hiding it here is what keeps the four-lines-per-worker duplication from
/// coming back in a different place.
pub struct ShutdownSignal(watch::Receiver<bool>);

impl std::future::IntoFuture for ShutdownSignal {
    type Output = ();
    type IntoFuture = std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>;

    /// # Why `IntoFuture` and not `Future`
    ///
    /// Implementing `Future` directly would mean writing `poll` by hand, since
    /// the natural body --- `self.0.changed().await` --- is an async block whose
    /// type cannot be named. `IntoFuture` provides a place to put the
    /// `Box::pin` that erases it, and callers still just write
    /// `shutdown.await`: the `.await` operator desugars through `IntoFuture`,
    /// so the boxing is invisible at every use site.
    ///
    /// The cost is one heap allocation per worker, paid once at startup.
    fn into_future(mut self) -> Self::IntoFuture {
        Box::pin(async move {
            // `changed()` errors only when every sender has been dropped, which
            // also means shutdown --- so either arm of this is "stop".
            let _ = self.0.changed().await;
        })
    }
}
