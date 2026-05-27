use crate::graphql::error::gql_err;
use crate::neo4j::Db;
use async_graphql::{Context, Enum, InputObject, SimpleObject};
use neo4rs::query;
use serde::Deserialize;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Enum, Deserialize)]
pub enum AssetKind {
    Video,
    Unknown,
}

#[derive(Debug, InputObject)]
pub struct AddAssetInput {
    pub uri: String,
    pub kind: AssetKind,
}

#[derive(SimpleObject)]
pub struct AddAssetPayload {
    pub asset: Asset,
}

#[derive(SimpleObject, Clone, Deserialize)]
pub struct Asset {
    pub uid: String,
    pub uri: String,
    pub kind: AssetKind,
}

pub async fn add_asset(
    ctx: &Context<'_>,
    input: AddAssetInput,
) -> async_graphql::Result<AddAssetPayload> {
    let db = ctx.data::<Db>()?;

    let mut txn = db.start_txn().await.map_err(gql_err)?;

    let node_label = match input.kind {
        AssetKind::Video => "Video",
        AssetKind::Unknown => "Unknown",
    };

    let asset_uid = nanoid::nanoid!();
    let mut create_stream = txn
        .execute(
            query(
                "
                MERGE (asset:$(['Asset', $nodeLabel]) {uri: $uri, kind: $nodeLabel})
                    ON CREATE SET asset.uid = $uid
                RETURN asset
            ",
            )
            .param("nodeLabel", node_label)
            .param("uri", input.uri)
            .param("uid", asset_uid.clone()),
        )
        .await
        .map_err(gql_err)?;

    let asset = match create_stream.single(&mut txn).await {
        Ok(row) => row.get("asset").map_err(gql_err).unwrap(),
        Err(_) => Asset {
            uid: "".into(),
            uri: "".into(),
            kind: AssetKind::Unknown,
        },
    };

    txn.commit().await.map_err(gql_err)?;

    Ok(AddAssetPayload { asset })
}
