use async_graphql::Context;
use neo4rs::query;

use crate::graphql::error::gql_err;
use crate::graphql::nodes::{Interview, Statement, StatementNode, Transcript};
use crate::neo4j::Db;

// ---------------------------------------------------------------------------
// Resolver functions---called from QueryRoot in queries/root.rs.
// ---------------------------------------------------------------------------

pub async fn list_interviews(ctx: &Context<'_>) -> async_graphql::Result<Vec<Interview>> {
    let db = ctx.data::<Db>()?;

    let mut stream = db
        .execute(query(
            "MATCH (interview:Interview)
             RETURN interview
             ORDER BY interview.date ASC",
        ))
        .await
        .map_err(gql_err)?;

    let mut interviews = Vec::new();

    while let Some(row) = stream.next().await.map_err(gql_err)? {
        let node: neo4rs::Node = row.get("interview").map_err(gql_err)?;
        interviews.push(node.to().map_err(gql_err)?);
    }

    Ok(interviews)
}

pub async fn get_transcript(ctx: &Context<'_>, number: i64) -> async_graphql::Result<Transcript> {
    let db = ctx.data::<Db>()?;
    let mut stream = db
        .execute(
            query(
                "MATCH (interview:Interview {number: $number})
                       -[:HAS_TRANSCRIPT]->(transcript:Transcript)
                       -[contains:CONTAINS]->(statement:Statement)
                       <-[:SAYS]-(person:Person)
                 OPTIONAL MATCH (interview)-[:INTERVIEWED_BY]->(interviewer:Person)
                   WHERE interviewer = person

                 RETURN interview, transcript, statement, person, contains
                 ORDER BY contains.startTime",
            )
            .param("number", number),
        )
        .await
        .map_err(gql_err)?;

    let mut interview_opt: Option<Interview> = None;
    let mut transcript_uid = String::new();
    let mut statements: Vec<Statement> = Vec::new();

    while let Some(row) = stream.next().await.map_err(gql_err)? {
        if interview_opt.is_none() {
            let node: neo4rs::Node = row.get("interview").map_err(gql_err)?;
            let t_node: neo4rs::Node = row.get("transcript").map_err(gql_err)?;
            interview_opt = Some(node.to().map_err(gql_err)?);
            transcript_uid = t_node.get("uid").map_err(gql_err)?;
        }

        let statement: neo4rs::Node = row.get("statement").map_err(gql_err)?;
        let person: neo4rs::Node = row.get("person").map_err(gql_err)?;
        let contains: neo4rs::Relation = row.get("contains").map_err(gql_err)?;
        let sn: StatementNode = statement.to().map_err(gql_err)?;

        statements.push(Statement {
            uid: sn.uid,
            text: sn.text,
            // Person can be deserialized directly---its fields match the
            // node properties exactly (uid, name).
            person: person.to().map_err(gql_err)?,
            // Timing lives on the :CONTAINS relationship, not the Statement
            // node. Relation::get() works just like Node::get().
            start_time: contains.get("startTime").map_err(gql_err)?,
            end_time: contains.get("endTime").map_err(gql_err)?,
            words: sn.words,
        });
    }

    let interview = interview_opt
        .ok_or_else(|| async_graphql::Error::new(format!("interview #{number} not found")))?;

    Ok(Transcript {
        uid: transcript_uid,
        interview,
        statements,
    })
}
