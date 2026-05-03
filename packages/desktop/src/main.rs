use auohp_core::embeddings;
use axum::{Json, Router, http, response::IntoResponse};
use serde::{Deserialize, Serialize};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tokio::net::TcpListener;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum Status {
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
    gpu: GpuMeta,
    models: AvailableModels,
    status: Status,
    version: String,
}

fn main() {
    // Tracing goes to stderr so structured logs don't mix with any stdout
    // output (e.g. health-check scripts that parse the server's stdout).
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "auohp_desktop=debug".into()))
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
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

            // axum runs as a background task on the same tokio runtime Tauri uses.
            // No separate thread, no IPC --- just another future in the pool.
            tauri::async_runtime::spawn(run_server());

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to start app");
}

async fn run_server() {
    let router = Router::new()
        .route("/hello", axum::routing::post(hello_handler))
        .route("/health", axum::routing::get(healthcheck_handler));

    let listener = TcpListener::bind("127.0.0.1:8705")
        .await
        .expect("failed to bind 127.0.0.1:8705");

    axum::serve(listener, router)
        .await
        .expect("axum server error");
}

async fn hello_handler() -> impl IntoResponse {
    tracing::info!("logging the future");

    (http::StatusCode::ACCEPTED, "Goodbye, cruel world!")
}

async fn healthcheck_handler() -> impl IntoResponse {
    let gpu = match metal_gpu_info() {
        Some((name, vram)) => GpuMeta {
            name: Some(name),
            vram: Some(vram),
        },
        None => GpuMeta {
            name: None,
            vram: None,
        },
    };

    let health = HealthcheckResponse {
        status: Status::Ok,
        version: "0.0.0".into(),
        backend: gpu_backend().into(),
        capabilities: vec!["transcribe".into(), "diarize".into(), "embed".into()],
        gpu,
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

fn metal_gpu_info() -> Option<(String, u64)> {
    let device = metal::Device::system_default()?;
    let name = device.name().to_string();
    let vram = device.recommended_max_working_set_size();
    Some((name, vram))
}
