use crate::graphql::error::gql_err;
use crate::graphql::interviews::StatementNode;
use crate::neo4j::Db;
use async_graphql::{Context, SimpleObject};
use neo4rs::query;

struct Word {
    text: String,
}

struct Transcript {
    words: Vec<Word>,
}

pub async fn get_transcript_words(
    ctx: &Context<'_>,
    transcript_uid: &str,
) -> async_graphql::Result<Transcript> {
    todo!()
}
