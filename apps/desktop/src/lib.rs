mod runtime;

use runtime::{
    ClientError, ClientMessage, ClientSnapshot, FeedbackInput, FeedbackStatus, RuntimeState,
    SendMessageInput,
};
use tauri::{Manager, State, WebviewUrl, WebviewWindowBuilder};

#[tauri::command]
async fn client_snapshot(state: State<'_, RuntimeState>) -> Result<ClientSnapshot, ClientError> {
    state.snapshot(true).await
}

#[tauri::command]
async fn sync_client(state: State<'_, RuntimeState>) -> Result<ClientSnapshot, ClientError> {
    state.snapshot(true).await
}

#[tauri::command]
async fn join_community(
    state: State<'_, RuntimeState>,
    invite_code: String,
) -> Result<ClientSnapshot, ClientError> {
    state.join(invite_code).await
}

#[tauri::command]
async fn send_message(
    state: State<'_, RuntimeState>,
    input: SendMessageInput,
) -> Result<ClientMessage, ClientError> {
    state.send(input).await
}

#[tauri::command]
async fn submit_feedback(
    state: State<'_, RuntimeState>,
    input: FeedbackInput,
) -> Result<FeedbackStatus, ClientError> {
    state.submit_feedback(input).await
}

#[tauri::command]
async fn feedback_status(
    state: State<'_, RuntimeState>,
) -> Result<Option<FeedbackStatus>, ClientError> {
    state.feedback_status().await
}

pub fn run() {
    let application = tauri::Builder::default()
        .manage(RuntimeState::discover())
        .setup(|app| {
            if app.get_webview_window("main").is_none() {
                WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                    .title("ChatCommons Alpha")
                    .inner_size(1180.0, 760.0)
                    .min_inner_size(760.0, 540.0)
                    .build()?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            client_snapshot,
            sync_client,
            join_community,
            send_message,
            submit_feedback,
            feedback_status
        ])
        .run(tauri::generate_context!());
    if let Err(error) = application {
        eprintln!("ChatCommons desktop failed: {error}");
    }
}
