use crate::graphql::nodes::{Statement, StatementNode};
use crate::neo4j::Db;
use crate::uid;
use async_graphql::{Context, InputObject, SimpleObject};
use auohp_core::embeddings::EmbedderHandle;
use neo4rs::{BoltType, query};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, InputObject)]
pub struct CreateStatementInput {
    pub text: String,
    pub start_time: f64,
    pub end_time: f64,
}

#[derive(SimpleObject)]
pub struct CreateStatementPayload {
    pub statement: Statement,
}

pub async fn create_statement(
    ctx: &Context<'_>,
    statement: CreateStatementInput,
    interview_uid: String,
) -> async_graphql::Result<CreateStatementPayload> {
    let db = ctx.data::<Db>()?;
    let mut txn = db.start_txn().await?;

    let statement_uid = uid::generate();
    let statement_params: HashMap<&str, BoltType> = HashMap::from([
        ("text", BoltType::from(statement.text.clone())),
        ("uid", BoltType::from(statement_uid.clone())),
    ]);
    let span_params: HashMap<&str, BoltType> = HashMap::from([
        ("startTime", BoltType::from(statement.start_time)),
        ("endTime", BoltType::from(statement.end_time)),
    ]);

    let mut create_stream = txn
        .execute(
            query(
                r#"
                MATCH (interview:Interview {uid: $interviewUid})-[:HAS_TRANSCRIPT]->(transcript:Transcript)
                MATCH (interview)-[:INTERVIEWS]->(interviewee:Person)
                CREATE (statement:Statement $statementParams)
                MERGE (statement)<-[span:CONTAINS]-(transcript)
                MERGE (statement)<-[:SAYS]-(interviewee)
                SET span = $spanParams
                RETURN statement, span
            "#,
            )
            .param("interviewUid", interview_uid.clone())
            .param("statementParams", BoltType::from(statement_params))
            .param("spanParams", BoltType::from(span_params)),
        )
        .await?;

    let row = create_stream
        .next(&mut txn)
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| async_graphql::Error::new("create statement failed"))?;

    let statement_node = row.get::<StatementNode>("statement")?;
    let span: neo4rs::Relation = row.get("span")?;

    let embedder = ctx.data::<Arc<EmbedderHandle>>()?.clone();
    let embedding = embedder
        .embed(vec![statement.text.clone()])
        .await?
        .into_iter()
        .next();

    if let Some(v) = embedding {
        let bolt_vector: Vec<BoltType> = v.iter().map(|&v| BoltType::from(v as f64)).collect();
        txn.run(
            query(
                r#"
                    MATCH (statement:Statement {uid: $uid})
                    CALL db.create.setNodeVectorProperty(statement, 'embedding', $vector)
                "#,
            )
            .param("uid", statement_uid.clone())
            .param("vector", bolt_vector),
        )
        .await?;
    }

    txn.commit().await?;

    Ok(CreateStatementPayload {
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
