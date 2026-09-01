mod transcription;

use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::{Json, Router, http, response::IntoResponse};
use serde::{Deserialize, Serialize};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter};
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::{Stream, StreamExt};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use transcription::{
    CancelError, Event as JobEvent, JobId, Registry, Status as JobStatus, SubmitError,
    SubmitOutcome, TranscribeSource,
};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum HealthStatus {
    Ok,
    Unstable,
}

#[derive(Debug, Deserialize, Serialize)]
struct GpuMeta {
    name: Option<String>,
    vram: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize)]
struct AvailableModels {
    whisper: String,
    diarization: String,
    embeddings: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct HealthcheckResponse {
    backend: String,
    capabilities: Vec<String>,
    models: AvailableModels,
    status: HealthStatus,
    version: String,
}

/// State for axum handlers. The same `Arc<Registry>` lives in
/// `tauri::Manager`'s typed state map, so Tauri commands and HTTP
/// routes share one source of truth. `AppHandle` is here so HTTP-
/// originated submits can still wire the app-emit bridge for the
/// webview.
#[derive(Clone)]
struct AppState {
    registry: Arc<Registry>,
    app: AppHandle,
}

fn main() {
    // Tracing goes to stderr so structured logs don't mix with any stdout
    // output (e.g. health-check scripts that parse the server's stdout).
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "auohp_desktop=debug".into()))
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    let registry = Arc::new(Registry::new());

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(Arc::clone(&registry))
        .invoke_handler(tauri::generate_handler![
            transcribe_local,
            transcribe_cancel,
            transcribe_status,
        ])
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let say_hello = MenuItem::with_id(app, "say-hello", "Say Hello", true, None::<&str>)?;
            let quit = PredefinedMenuItem::quit(app, Some("Quit"))?;
            let menu = Menu::with_items(app, &[&say_hello, &quit])?;

            // TrayIconBuilder stores the menu and the event handler together.
            // The closure is Fn (not FnMut) --- it may be called concurrently,
            // so it can only read captured state. Each click spawns an independent
            // task on the shared tokio runtime.
            TrayIconBuilder::new()
                .menu(&menu)
                .icon(tauri::include_image!("icons/tray-icon.png"))
                .icon_as_template(true)
                .on_menu_event(|_app, event| {
                    if event.id() == "say-hello" {
                        tauri::async_runtime::spawn(async {
                            reqwest::Client::new()
                                .post("http://127.0.0.1:8705/hello")
                                .send()
                                .await
                                .ok();
                        });
                    }
                })
                .build(app)?;

            let state = AppState {
                registry: Arc::clone(&registry),
                app: app.handle().clone(),
            };

            // axum runs as a background task on the same tokio runtime Tauri uses.
            // No separate thread, no IPC --- just another future in the pool.
            tauri::async_runtime::spawn(run_server(state));

            Ok(())
        })
        .run(tauri::generate_context!())
        // "The last line that executes; everything else is a callback or a task""
        .expect("failed to start app");
}

async fn run_server(state: AppState) {
    let router = Router::new()
        .route("/hello", axum::routing::post(hello_handler))
        .route("/health", axum::routing::get(healthcheck_handler))
        .route("/transcribe", axum::routing::post(transcribe_handler))
        .route("/transcribe/cancel", axum::routing::post(cancel_handler))
        .route("/transcribe/status", axum::routing::get(status_handler))
        .route("/transcribe/events", axum::routing::get(events_handler))
        .with_state(state);

    let listener = TcpListener::bind("127.0.0.1:8705")
        .await
        .expect("failed to bind 127.0.0.1:8705");

    axum::serve(listener, router)
        .await
        .expect("axum server error");
}

// ---- shared bridge helper ---------------------------------------------

/// Drains a per-job broadcast `Receiver`, emitting each event to the
/// Tauri webview under a single channel name. The task ends when the
/// channel closes (the registry's cleanup wrapper drops the last sender).
fn spawn_app_emit_bridge(app: AppHandle, mut rx: broadcast::Receiver<JobEvent>) {
    tauri::async_runtime::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    if let Err(e) = app.emit("transcription://event", &event) {
                        tracing::warn!(error = %e, "failed to emit transcription event");
                    }
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    tracing::warn!(skipped, "transcription event bridge lagged");
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

// ---- Tauri commands ---------------------------------------------------

#[tauri::command]
async fn transcribe_local(
    state: tauri::State<'_, Arc<Registry>>,
    app: AppHandle,
    path: PathBuf,
    interview_id: String,
) -> Result<JobId, SubmitError> {
    let source = TranscribeSource::Local { path, interview_id };
    let SubmitOutcome { id, events } = state.submit(source).await?;
    spawn_app_emit_bridge(app, events);
    Ok(id)
}

#[tauri::command]
async fn transcribe_cancel(
    state: tauri::State<'_, Arc<Registry>>,
    id: JobId,
) -> Result<(), CancelError> {
    state.cancel(&id).await
}

#[tauri::command]
async fn transcribe_status(state: tauri::State<'_, Arc<Registry>>) -> Result<JobStatus, ()> {
    // The `Result` is not about fallibility --- `status()` cannot fail. It is
    // what lets the command borrow `State<'_, _>`: tauri's macro dispatches
    // Result-returning futures to a code path with no `'static` bound, while
    // plain-value futures are spawned and must outlive the IPC message.
    Ok(state.status().await)
}

// ---- HTTP handlers ----------------------------------------------------

#[derive(Debug, Deserialize)]
struct TranscribeRequest {
    path: PathBuf,
    interview_id: String,
}

#[derive(Debug, Serialize)]
struct TranscribeResponse {
    id: JobId,
}

async fn transcribe_handler(
    State(state): State<AppState>,
    Json(body): Json<TranscribeRequest>,
) -> impl IntoResponse {
    let source = TranscribeSource::Local {
        path: body.path,
        interview_id: body.interview_id,
    };
    match state.registry.submit(source).await {
        Ok(SubmitOutcome { id, events }) => {
            spawn_app_emit_bridge(state.app.clone(), events);
            (http::StatusCode::ACCEPTED, Json(TranscribeResponse { id })).into_response()
        }
        Err(e) => (http::StatusCode::CONFLICT, Json(e)).into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct CancelRequest {
    id: JobId,
}

async fn cancel_handler(
    State(state): State<AppState>,
    Json(body): Json<CancelRequest>,
) -> impl IntoResponse {
    match state.registry.cancel(&body.id).await {
        Ok(()) => http::StatusCode::NO_CONTENT.into_response(),
        Err(e @ CancelError::NotRunning) => (http::StatusCode::NOT_FOUND, Json(e)).into_response(),
        Err(e @ CancelError::IdMismatch { .. }) => {
            (http::StatusCode::CONFLICT, Json(e)).into_response()
        }
    }
}

async fn status_handler(State(state): State<AppState>) -> impl IntoResponse {
    Json(state.registry.status().await)
}

/// SSE stream of the active job's events. 404 when nothing is running;
/// otherwise the connection stays open until the channel closes (the
/// registry's cleanup wrapper drops the last sender) or the client
/// disconnects.
async fn events_handler(State(state): State<AppState>) -> impl IntoResponse {
    let rx = match state.registry.subscribe().await {
        Some(rx) => rx,
        None => return http::StatusCode::NOT_FOUND.into_response(),
    };
    Sse::new(broadcast_to_sse(rx))
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
        .into_response()
}

/// Adapter: `broadcast::Receiver<JobEvent>` --> `Stream<Item = Result<SseEvent, _>>`.
///
/// `BroadcastStream` is the bridge from broadcast's recv-loop API to
/// the `Stream` trait axum's SSE expects. We swallow `Lagged` errors:
/// a slow client should lose events, not break the response.
fn broadcast_to_sse(
    rx: broadcast::Receiver<JobEvent>,
) -> impl Stream<Item = Result<SseEvent, Infallible>> {
    BroadcastStream::new(rx).filter_map(|item| match item {
        Ok(event) => Some(Ok(SseEvent::default()
            .json_data(&event)
            .unwrap_or_else(|_| SseEvent::default().comment("serialize failed")))),
        Err(_lagged) => None,
    })
}

// ---- preserved hello + healthcheck ------------------------------------

async fn hello_handler() -> impl IntoResponse {
    tracing::info!("logging the future");
    (http::StatusCode::ACCEPTED, "Goodbye, cruel world!")
}

async fn healthcheck_handler() -> impl IntoResponse {
    let health = HealthcheckResponse {
        status: HealthStatus::Ok,
        version: "0.0.0".into(),
        backend: gpu_backend().into(),
        capabilities: vec!["transcribe".into(), "diarize".into(), "embed".into()],
        models: AvailableModels {
            whisper: "large-v3".into(),
            diarization: "pyannote-segmentation-3.0".into(),
            embeddings: "wespeaker-voxceleb-ecapa-tdnn1024".into(),
        },
    };
    let data = serde_json::json!(&health);

    (http::StatusCode::OK, Json(data)).into_response()
}

fn gpu_backend() -> &'static str {
    if cfg!(feature = "metal") {
        "metal"
    } else if cfg!(feature = "cuda") {
        "cuda"
    } else {
        "cpu"
    }
}
