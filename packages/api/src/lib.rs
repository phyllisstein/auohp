//! Library target for the AUOHP API.
//!
//! Added by the Loco migration spike. Loco's CLI entrypoint
//! (`loco_rs::cli::main::<App>()`) needs the `Hooks` implementation to be
//! importable from a binary, and this crate had no library target --- `main.rs`
//! declared its modules privately with `mod`.
//!
//! This is itself a small finding: Loco expects the standard generated layout
//! (`src/lib.rs` exporting `app`, `controllers`, `models`, `views`, `workers`,
//! `tasks`, with a thin `src/bin/main.rs`). Adopting Loco means adopting that
//! split, which is a structural change to the crate independent of any
//! behavioral one.

pub mod captions;
pub mod graphql;
pub mod loco_app;
pub mod neo4j;
pub mod uid;
