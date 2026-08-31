use crate::graphql::nodes::{Statement, StatementNode};
use crate::neo4j::Db;
use async_graphql::{Context, InputObject, SimpleObject};
use auohp_core::embeddings::EmbedderHandle;
use neo4rs::{BoltType, query};
use std::sync::Arc;

#[derive(Debug, InputObject)]
pub struct EditStatementInput {
    pub uid: String,
    pub text: String,
    pub start_time: f64,
    pub end_time: f64,
}

#[derive(Debug, SimpleObject)]
pub struct EditStatementPayload {
    pub old_hash: String,
    pub new_hash: String,
    pub wrote_embedding: bool,
    pub statement: Statement,
}

pub async fn edit_statement(
    ctx: &Context<'_>,
    input: EditStatementInput,
) -> async_graphql::Result<EditStatementPayload> {
    let db = ctx.data::<Db>()?;
    let mut txn = db.start_txn().await?;

    let mut edit_stream = txn
        .execute(
            query(
                // FIXME: Scanning all Statements for a UID breaks Neo4j graph
                // idioms. Consider a scan for the Transcript with edges to
                // Statements.
                "
                MATCH (statement:Statement {uid: $uid})<-[span:CONTAINS]-()
                LET oldText = statement.text
                SET statement.text = $text, span.startTime = $startTime, span.endTime = $endTime
                RETURN statement, oldText, span
            ",
            )
            .param("uid", input.uid.clone())
            .param("text", input.text.clone())
            .param("startTime", input.start_time)
            .param("endTime", input.end_time),
        )
        .await?;

    let row = edit_stream
        .next(&mut txn)
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| async_graphql::Error::new("statement not found"))?;

    let statement_node: StatementNode = row.get("statement")?;
    let old_text: String = row.get("oldText")?;
    let span: neo4rs::Relation = row.get("span")?;

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

    if let Some(v) = &embedding {
        let bolt_vector: Vec<BoltType> = v.iter().map(|&v| BoltType::from(v as f64)).collect();

        txn.run(
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

    let wrote_embedding = embedding.is_some();

    txn.commit().await?;

    let old_hash = md5::compute(old_text);
    let new_hash = md5::compute(statement_node.text.clone());

    Ok(EditStatementPayload {
        old_hash: format!("{:x}", old_hash),
        new_hash: format!("{:x}", new_hash),
        wrote_embedding,
        statement: Statement {
            uid: statement_node.uid,
            text: statement_node.text,
            person: None,
            start_time: span.get("startTime")?,
            end_time: span.get("endTime")?,
            words: None,
        },
    })
}
