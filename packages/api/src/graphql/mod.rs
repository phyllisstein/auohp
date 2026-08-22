mod mutations;
// pub(crate): node projections are shared crate-wide (graphql resolvers *and*
// the transport-agnostic `captions` module read them). See the FIXME in
// nodes.rs --- their real home is auohp-core, which would make this a true pub.
pub(crate) mod nodes;
mod queries;
pub(crate) mod row;
mod schema;

pub use schema::{AppSchema, build_schema};
