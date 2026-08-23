use crate::graphql::nodes::{
    Interview as GqlInterview, Person, Statement, StatementNode, Transcript,
};
use crate::graphql::row::*;
use crate::neo4j::Db;
use async_graphql::{Context, Object, ScalarType, SimpleObject};
use chrono::NaiveDate;
use neo4rs::query;
use serde::Deserialize;

#[derive(Deserialize, Default, Debug, Clone)]
pub struct Interview {
    number: Option<i64>,
    uid: String,
}

#[Object]
impl Interview {
    async fn transcript(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<Transcript>> {
        let db = ctx.data::<Db>()?;

        tracing::debug!("starting transcript fn");

        // TODO: Each interview should have exactly one transcript, but LIMIT 1
        // is a poor enforcement strategy, swallowing up drift in the schema
        // state and giving not-wholly-deterministic responses based on that
        // state.
        let mut stream = db
            .execute(
                query(
                    "MATCH (interview:Interview {number: $number})
                       -[:HAS_TRANSCRIPT]->(transcript:Transcript)
                       -[contains:CONTAINS]->(statement:Statement)
                       <-[:SAYS]-(person:Person)

                 RETURN interview, transcript, statement, person, contains
                 ORDER BY contains.startTime
                 ",
                )
                .param("number", self.number),
            )
            .await?;

        tracing::debug!("Neo4j query returned");

        let mut statements: Vec<Statement> = Vec::new();
        let mut transcript_uid = String::new();

        while let Some(row) = stream.next().await? {
            let statement_node = row.node_as::<StatementNode>("statement")?;
            let person = row.node_as::<Person>("person")?;
            let start_time = row.rel_prop::<f64>("contains", "startTime")?;
            let end_time = row.rel_prop::<f64>("contains", "endTime")?;

            statements.push(Statement {
                uid: statement_node.uid,
                text: statement_node.text,
                person: Some(person),
                start_time: Some(start_time),
                end_time: Some(end_time),
                words: statement_node.words,
            });

            let transcript_node: neo4rs::Node = row.get("transcript")?;
            let transcript_uid: String = transcript_node.get("uid")?;
        }

        Ok(Some(Transcript {
            uid: "ididid".into(),
            statements,
        }))
    }
}

// pub struct Interviews;
//
// #[Object]
// impl Interviews {}

#[derive(Default)]
pub struct InterviewQuery;

#[Object]
impl InterviewQuery {
    async fn interview(
        &self,
        ctx: &Context<'_>,
        number: i64,
    ) -> async_graphql::Result<Option<Interview>> {
        let db = ctx.data::<Db>()?;

        let mut stream = db
            .execute(query!(
                "
                MATCH (interview:Interview {{number: {number}}}) RETURN interview LIMIT 1
            ",
                number = number
            ))
            .await?;

        Ok(stream.first_as::<Interview>("interview").await?)
    }

    async fn interviews(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<Interview>> {
        let db = ctx.data::<Db>()?;

        let mut stream = db
            .execute(query(
                "
                    MATCH (interview:Interview)
                    RETURN interview
                    ORDER BY interview.date ASC
                    LIMIT 5
                ",
            ))
            .await?;

        let mut interviews = Vec::new();

        while let Some(row) = stream.next().await? {
            let node = row.node_as::<Interview>("interview")?;
            interviews.push(node);
        }

        Ok(interviews)
    }
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

    let mut interview_opt: Option<GqlInterview> = None;
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
            person: person.to()?,
            start_time: contains.get("startTime")?,
            end_time: contains.get("endTime")?,
            words: sn.words,
        });
    }

    let interview = interview_opt
        .ok_or_else(|| async_graphql::Error::new(format!("interview #{number} not found")))?;

    Ok(Transcript {
        uid: transcript_uid,
        statements,
    })
}
