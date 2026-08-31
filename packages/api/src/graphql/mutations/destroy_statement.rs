use crate::{graphql::nodes::StatementNode, neo4j::Db};
use async_graphql::{Context, InputObject, SimpleObject};
use neo4rs::query;

#[derive(SimpleObject)]
pub struct DestroyStatementPayload {
    pub ok: bool,
    pub statement: StatementNode,
}

pub async fn destroy_statement(
    ctx: &Context<'_>,
    uid: String,
) -> async_graphql::Result<DestroyStatementPayload> {
    let db = ctx.data::<Db>()?;

    let mut destroy_stream = db
        .execute(
            query(
                r#"
                MATCH (statement:Statement {uid: $uid})
                DETACH DELETE statement
                RETURN statement
            "#,
            )
            .param("uid", uid),
        )
        .await?;

    let statement = match destroy_stream.next().await?.into_iter().next() {
        Some(row) => row.get::<StatementNode>("statement")?,
        None => {
            return Err(async_graphql::Error {
                message: "Missing statement".into(),
                source: None,
                extensions: None,
            });
        }
    };

    Ok(DestroyStatementPayload {
        ok: true,
        statement,
    })
}
