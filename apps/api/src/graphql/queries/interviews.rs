use crate::graphql::nodes::{Person, Statement, StatementNode, Transcript, Video};
use crate::neo4j::Db;
use async_graphql::{Context, Error, Object};
use chrono::NaiveDate;
use neo4rs::query;
use serde::Deserialize;

#[derive(Deserialize, Default, Debug, Clone)]
pub struct Interview {
    pub number: i64,
    pub uid: String,
    pub date: NaiveDate,
}

#[Object]
impl Interview {
    fn uid(&self) -> &str {
        &self.uid
    }

    async fn interviewee(&self, ctx: &Context<'_>) -> async_graphql::Result<Person> {
        let db = ctx.data::<Db>()?;

        let mut stream = db
            .execute(
                query(
                    r#"
                        MATCH (interview:Interview {uid: $uid})-[:INTERVIEWS]->(interviewee:Person)
                        RETURN interviewee
                    "#,
                )
                .param("uid", self.uid.clone()),
            )
            .await?;

        if let Some(row) = stream.next().await? {
            let interviewee = row.get::<Person>("interviewee")?;
            Ok(interviewee)
        } else {
            Err(async_graphql::Error::new("could not find interviewee"))
        }
    }

    fn date(&self) -> &NaiveDate {
        &self.date
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

    async fn videos(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<Video>> {
        let db = ctx.data::<Db>()?;

        let mut stream = db
            .execute_read(query!(
                r#"
                    MATCH (interview:Interview)-[:HAS_ASSET]->(video:Video)
                    WHERE interview.uid = {uid}
                    RETURN video
                "#,
                uid = self.uid.clone()
            ))
            .await?;

        let mut videos: Vec<Video> = Vec::new();

        while let Some(row) = stream.next().await? {
            let v = row.get::<Video>("video")?;
            videos.push(v);
        }

        Ok(videos)
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
