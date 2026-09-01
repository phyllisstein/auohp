//! Background jobs --- a spike port of Loco's `bgworker` design to vanilla axum.
//!
//! See `docs/spikes/bgworker-port.md` for the design rationale. This module is
//! scaffolding: types and signatures are real, bodies are `todo!()`.
//!
//! The shape, in one paragraph. A [`Queue`] is a cloneable handle that callers
//! (GraphQL resolvers) use to enqueue work. It is a newtype over
//! `Arc<dyn QueueBackend>`, so the caller's code is written against one concrete
//! type while the actual storage --- today an in-process channel, tomorrow
//! possibly something durable --- lives behind a trait object. A [`Worker`] is
//! the other half: a typed unit of work that knows how to deserialize its own
//! arguments and `perform` them. The two never mention each other's types; they
//! meet through a [`JobHandler`], a type-erased boxed closure stored in the
//! [`Registry`] under the worker's name.
//!
//! What we deliberately did *not* take from Loco: the `AppContext` god-object,
//! the config-driven `WorkerMode` enum, the CLI surface (`dump`/`import`/
//! `cancel`/`retry_failed`), and cron/`interval` scheduling. Each is discussed
//! in the spike doc.

pub mod backend;
pub mod handle;
pub mod queue;
pub mod registry;
pub mod worker;
pub mod workers;

#[cfg(test)]
mod tests;

// Re-exported for the wiring in `main.rs`. Everything else is reached through
// its module path, so the surface here stays as small as the call sites need
// rather than mirroring the whole module.
pub use backend::InProcessBackend;
pub use handle::StateStore;
pub use queue::Queue;
pub use registry::Registry;
pub use worker::JobContext;
