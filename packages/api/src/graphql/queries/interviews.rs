use async_graphql::Context;
use neo4rs::query;

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
        .await?;

    let mut interviews = Vec::new();

    while let Some(row) = stream.next().await? {
        let node: neo4rs::Node = row.get("interview")?;
        interviews.push(node.to()?);
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
        .await?;

    let mut interview_opt: Option<Interview> = None;
    let mut transcript_uid = String::new();
    let mut statements: Vec<Statement> = Vec::new();

    while let Some(row) = stream.next().await? {
        if interview_opt.is_none() {
            let node: neo4rs::Node = row.get("interview")?;
            let t_node: neo4rs::Node = row.get("transcript")?;
            interview_opt = Some(node.to()?);
            transcript_uid = t_node.get("uid")?;
        }

        let statement: neo4rs::Node = row.get("statement")?;
        let person: neo4rs::Node = row.get("person")?;
        let contains: neo4rs::Relation = row.get("contains")?;
        let sn: StatementNode = statement.to()?;

        statements.push(Statement {
            uid: sn.uid,
            text: sn.text,
            // Person can be deserialized directly---its fields match the
            // node properties exactly (uid, name).
            person: person.to()?,
            // Timing lives on the :CONTAINS relationship, not the Statement
            // node. Relation::get() works just like Node::get().
            start_time: contains.get("startTime")?,
            end_time: contains.get("endTime")?,
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
