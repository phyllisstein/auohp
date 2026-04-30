use axum::{Router, http, response::IntoResponse, routing::post};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tokio::net::TcpListener;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

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
    let router = Router::new().route("/hello", post(hello_handler));

    let listener = TcpListener::bind("127.0.0.1:8705")
        .await
        .expect("failed to bind 127.0.0.1:34042");

    axum::serve(listener, router)
        .await
        .expect("axum server error");
}

// TODO(human): implement hello_handler
async fn hello_handler() -> impl IntoResponse {
    tracing::info!("logging the future");

    (http::StatusCode::ACCEPTED, "Goodbye, cruel world!")
}
