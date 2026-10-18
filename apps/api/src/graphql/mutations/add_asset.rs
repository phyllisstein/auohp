use crate::neo4j::Db;
use async_graphql::{Context, Enum, InputObject, SimpleObject};
use neo4rs::query;
use serde::Deserialize;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Enum, Deserialize)]
pub enum AssetKind {
    Video,
    Unknown,
}

// FIXME: { parent_id: "", asset : {  } }
#[derive(Debug, InputObject)]
pub struct AddAssetInput {
    pub parent_id: String,
    pub kind: AssetKind,
    uri: String,
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

    let mut txn = db.start_txn().await?;

    let node_label = match input.kind {
        AssetKind::Video => "Video",
        AssetKind::Unknown => "Unknown",
    };

    let asset_uid = crate::uid::generate();
    let mut create_stream = txn
        .execute(query!(

            "
                MATCH (a) WHERE a.uid = {parentUid}
                MERGE (a) -[:HAS_ASSET]-> (asset:$(['Asset', $nodeLabel]) {{uri: {uri}, kind: {nodeLabel}}})
                    ON CREATE SET asset.uid = {uid}
                RETURN asset, a.uid as artifactUid
            ",
            nodeLabel = node_label,
            uri = input.uri,
            uid = asset_uid.clone(),
            parentUid = input.parent_id,
        ))
        .await?;

    let asset = match create_stream.single(&mut txn).await {
        Ok(row) => row.get("asset").unwrap(),
        Err(_) => Asset {
            uid: "".into(),
            uri: "".into(),
            kind: AssetKind::Unknown,
        },
    };

    txn.commit().await?;

    Ok(AddAssetPayload { asset })
}
