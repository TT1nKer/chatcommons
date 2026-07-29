mod network_path;
mod runtime;

use runtime::{
    ClientError, ClientInvitation, ClientMessage, ClientSnapshot, CreateInvitationInput,
    FeedbackInput, FeedbackStatus, RuntimeState, SendMessageInput, VoiceGrant, VoiceTokenInput,
};
use tauri::{Manager, State, WebviewUrl, WebviewWindowBuilder};

#[tauri::command]
async fn client_snapshot(state: State<'_, RuntimeState>) -> Result<ClientSnapshot, ClientError> {
    state.local_snapshot().await
}

#[tauri::command]
async fn sync_client(state: State<'_, RuntimeState>) -> Result<ClientSnapshot, ClientError> {
    state.synchronized_snapshot().await
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
async fn create_invitation(
    state: State<'_, RuntimeState>,
    input: CreateInvitationInput,
) -> Result<ClientInvitation, ClientError> {
    state.create_invitation(input).await
}

#[tauri::command]
async fn voice_token(
    state: State<'_, RuntimeState>,
    input: VoiceTokenInput,
) -> Result<VoiceGrant, ClientError> {
    state.voice_token(input).await
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
            create_invitation,
            send_message,
            voice_token,
            submit_feedback,
            feedback_status
        ])
        .run(tauri::generate_context!());
    if let Err(error) = application {
        eprintln!("ChatCommons desktop failed: {error}");
    }
}
