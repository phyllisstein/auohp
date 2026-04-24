//! Shared error helpers for GraphQL resolvers.
//!
//! Every resolver in this crate talks to Neo4j (via neo4rs) and needs to
//! convert its errors into async_graphql::Error. Rather than duplicate a
//! small helper in each module, we put it here once.

/// Convert any `Display`-able error into an async-graphql error.
///
/// This is the simplest bridge between the neo4rs / anyhow world and
/// async-graphql's error type. Usage:
///
/// ```ignore
/// db.execute(query("...")).await.map_err(gql_err)?;
/// ```
///
/// Note on trade-offs: this erases the original error type into a string,
/// which is fine for surfacing messages to GraphQL clients but means you
/// can't programmatically match on specific neo4rs error variants
/// downstream. If you need richer error codes later, extend this to call
/// `.extend_with()` and set a "code" extension field.
pub fn gql_err(e: impl std::fmt::Display) -> async_graphql::Error {
    async_graphql::Error::new(e.to_string())
}
