use crate::graphql::error::gql_err;
use crate::graphql::nodes::StatementNode;
use crate::neo4j::Db;
use async_graphql::{Context, InputObject, SimpleObject};
use auohp_core::embeddings::EmbedderHandle;
use neo4rs::{BoltType, query};
use std::sync::Arc;

#[derive(Debug, InputObject)]
pub struct EditStatementInput {
    pub uid: String,
    pub text: String,
}

#[derive(Debug, SimpleObject)]
pub struct EditStatementPayload {
    pub uid: String,
    pub old_hash: String,
    pub new_hash: String,
}

pub async fn edit_statement(
    ctx: &Context<'_>,
    input: EditStatementInput,
) -> async_graphql::Result<EditStatementPayload> {
    let db = ctx.data::<Db>()?;
    let embedder = ctx.data::<Arc<EmbedderHandle>>()?.clone();
    let vector = match embedder.embed_background(vec![input.text.clone()]).await {
        Ok(v) => v[0].clone(),
        Err(e) => {
            tracing::error!(error = %e, uid = input.uid.clone(), "embedding failed on edit_statement");
            vec![]
        }
    };
    let bolt_vector: Vec<BoltType> = vector.iter().map(|&v| BoltType::from(v as f64)).collect();

    let mut edit_stream = db
        .execute(
            query(
                "
                MATCH (statement:Statement {uid: $uid})
                WITH statement, statement.text AS oldText
                SET statement.text = $text
                CALL db.create.setNodeVectorProperty(statement, 'embedding', $vector)
                RETURN statement, oldText
            ",
            )
            .param("uid", input.uid)
            .param("vector", bolt_vector)
            .param("text", input.text),
        )
        .await
        .map_err(gql_err)?;

    let row = edit_stream.single().await.map_err(gql_err)?;
    let statement_node: StatementNode = row.get("statement").map_err(gql_err)?;
    let old_text: String = row.get("oldText").map_err(gql_err)?;

    let old_hash = md5::compute(old_text);
    let new_hash = md5::compute(statement_node.text);

    Ok(EditStatementPayload {
        old_hash: format!("{:x}", old_hash),
        new_hash: format!("{:x}", new_hash),
        uid: statement_node.uid,
    })
}
