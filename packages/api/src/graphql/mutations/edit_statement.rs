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
    pub wrote_embedding: bool,
}

pub async fn edit_statement(
    ctx: &Context<'_>,
    input: EditStatementInput,
) -> async_graphql::Result<EditStatementPayload> {
    let db = ctx.data::<Db>()?;
    let embedder = ctx.data::<Arc<EmbedderHandle>>()?.clone();
    let embedding = match embedder
        .embed(vec![input.text.clone()])
        .await
        .into_iter()
        .next()
    {
        Some(v) => v.into_iter().next(),
        None => {
            tracing::warn!(
                statement_uid = input.uid.clone(),
                "failed to create embeddings for edited statement"
            );
            None
        }
    };

    let wrote_embedding = embedding.is_some();

    let mut tx = db.start_txn().await?;
    let mut edit_stream = tx
        .execute(
            query(
                // FIXME: Scanning all Statements for a UID breaks Neo4j graph
                // idioms. Consider a scan for the Transcript with edges to
                // Statements.
                "
                MATCH (statement:Statement {uid: $uid})
                WITH statement, statement.text AS oldText
                SET statement.text = $text
                RETURN statement, oldText
            ",
            )
            .param("uid", input.uid.clone())
            .param("text", input.text),
        )
        .await?;

    let row = edit_stream
        .next(&mut tx)
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| async_graphql::Error::new("statement not found"))?;

    let statement_node: StatementNode = row.get("statement")?;
    let old_text: String = row.get("oldText")?;

    if let Some(v) = embedding {
        let bolt_vector: Vec<BoltType> = v.iter().map(|&v| BoltType::from(v as f64)).collect();

        tx.run(
            query(
                "
                MATCH (statement:Statement {uid: $uid})
                CALL db.create.setNodeVectorProperty(statement, 'embedding', $vector)
            ",
            )
            .param("uid", input.uid)
            .param("vector", bolt_vector),
        )
        .await?
    }

    tx.commit().await?;

    let old_hash = md5::compute(old_text);
    let new_hash = md5::compute(statement_node.text);

    Ok(EditStatementPayload {
        old_hash: format!("{:x}", old_hash),
        new_hash: format!("{:x}", new_hash),
        uid: statement_node.uid,
        wrote_embedding,
    })
}
