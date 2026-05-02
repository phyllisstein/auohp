//! Handler for attaching media assets to an interview.
//!
//! Route:
//!
//!   POST /interviews/:number/assets  --- create and attach an asset node

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use neo4rs::query;
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::error::{AppError, internal};
use crate::models::{Asset, AssetKind};

// ---------------------------------------------------------------------------
// Request / response bodies
// ---------------------------------------------------------------------------

/// Request body for `POST /interviews/:number/assets`.
#[derive(Debug, Deserialize)]
pub struct AddAssetBody {
    pub uri: String,
    pub kind: AssetKind,
}

/// Response body for `POST /interviews/:number/assets`.
#[derive(Debug, Serialize)]
pub struct AddAssetResponse {
    pub asset: Asset,
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

/// Creates an asset node and attaches it to the given interview.
///
/// `:number` is the public-facing interview number (e.g. 25, 64), matching
/// the `number` property on `(:Interview)` nodes.
pub async fn add_asset(
    State(state): State<AppState>,
    Path(number): Path<i64>,
    Json(body): Json<AddAssetBody>,
) -> Result<impl IntoResponse, AppError> {
    let mut txn = state.db.start_txn().await.map_err(internal)?;

    // The Cypher dynamic-label syntax `$(['Asset', $nodeLabel])` lets us set
    // multiple labels without string interpolation. The asset is always tagged
    // `:Asset` plus the kind-specific label (e.g. `:Video`).
    let node_label = match body.kind {
        AssetKind::Video => "Video",
        AssetKind::Unknown => "Unknown",
    };

    let asset_uid = nanoid::nanoid!();

    let mut create_stream = txn
        .execute(
            query(
                "
                MATCH (interview:Interview {number: $number})
                MERGE (asset:$([ 'Asset', $nodeLabel ]) {uri: $uri, kind: $nodeLabel})
                    ON CREATE SET asset.uid = $uid
                MERGE (interview)-[:HAS_ASSET]->(asset)
                RETURN asset
            ",
            )
            .param("number", number)
            .param("nodeLabel", node_label)
            .param("uri", body.uri.clone())
            .param("uid", asset_uid.clone()),
        )
        .await
        .map_err(internal)?;

    // `single()` consumes exactly one row from the stream. If the MATCH found
    // no interview, we get back nothing and return 404 rather than a bogus
    // empty-uid asset.
    let asset: Asset = match create_stream.single(&mut txn).await {
        Ok(row) => row.get::<neo4rs::Node>("asset").map_err(internal).and_then(
            |node| node.to::<Asset>().map_err(internal),
        )?,
        Err(_) => {
            return Err(AppError::NotFound(format!(
                "interview #{number} not found"
            )));
        }
    };

    txn.commit().await.map_err(internal)?;

    Ok((StatusCode::CREATED, Json(AddAssetResponse { asset })))
}
