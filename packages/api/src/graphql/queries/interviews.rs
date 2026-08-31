use crate::graphql::nodes::{Person, Statement, StatementNode, Transcript};
use crate::neo4j::Db;
use async_graphql::{Context, Error, Object};
use chrono::NaiveDate;
use neo4rs::query;
use serde::Deserialize;

#[derive(Deserialize, Default, Debug, Clone)]
pub struct Interview {
    pub number: i64,
    pub uid: String,
    pub interviewee: String,
    pub date: NaiveDate,
}

#[Object]
impl Interview {
    fn uid(&self) -> &str {
        &self.uid
    }

    fn interviewee(&self) -> &str {
        &self.interviewee
    }

    fn date(&self) -> NaiveDate {
        self.date
    }

    fn number(&self) -> i64 {
        self.number
    }

    async fn transcript(&self, ctx: &Context<'_>) -> async_graphql::Result<Transcript> {
        let db = ctx.data::<Db>()?;

        tracing::debug!("starting transcript fn");

        // TODO: Each interview should have exactly one transcript, but LIMIT 1
        // is a poor enforcement strategy, swallowing up drift in the schema
        // state and giving not-wholly-deterministic responses based on that
        // state.
        let mut stream = db
            .execute(
                query(
                    "MATCH (interview:Interview {uid: $uid})
                       -[:HAS_TRANSCRIPT]->(transcript:Transcript)
                       -[span:CONTAINS]->(statement:Statement)
                       <-[:SAYS]-(person:Person)

                 RETURN interview, transcript, statement, person, span
                 ORDER BY span.startTime
                 ",
                )
                .param("uid", self.uid.clone()),
            )
            .await?;

        tracing::debug!("Neo4j query returned");

        let mut statements: Vec<Statement> = Vec::new();
        let mut transcript_uid: Option<String> = None;

        while let Some(row) = stream.next().await? {
            let statement_node = row.get::<StatementNode>("statement")?;
            let person = row.get::<Person>("person")?;
            let span: neo4rs::Relation = row.get("span")?;

            statements.push(Statement {
                uid: statement_node.uid,
                text: statement_node.text,
                person: Some(person),
                start_time: Some(span.get("startTime")?),
                end_time: Some(span.get("endTime")?),
                words: statement_node.words,
            });

            if transcript_uid.is_none() {
                let transcript_node: neo4rs::Node = row.get("transcript")?;
                let uid = transcript_node.get("uid")?;
                transcript_uid = Some(uid);
            }
        }

        Ok(Transcript {
            uid: transcript_uid.expect("no transcript uid found"),
            statements,
        })
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
        number: Option<i64>,
        uid: Option<String>,
    ) -> async_graphql::Result<Interview> {
        if number.is_none() && uid.is_none() {
            return Err(Error {
                message: "missing uid and number".into(),
                extensions: None,
                source: None,
            });
        }

        let db = ctx.data::<Db>()?;
        let mut stream = db
            .execute(
                query(
                    r#"
                        MATCH (interview:Interview)
                        WHERE interview.uid = $uid OR interview.number = $number
                        RETURN interview
                    "#,
                )
                .param("uid", uid)
                .param("number", number),
            )
            .await?;

        let row = stream.single().await?;
        let node = row.get::<Interview>("interview")?;
        Ok(node)
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
            let node = row.get::<Interview>("interview")?;
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
            person: Some(person.to()?),
            start_time: Some(contains.get("startTime")?),
            end_time: Some(contains.get("endTime")?),
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
