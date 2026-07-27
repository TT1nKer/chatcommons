use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
};

const CONFIG_VERSION: u16 = 1;
const FEEDBACK_ENDPOINT: &str = "https://ttinker.net/chatcommons/api/app-feedback";
const PRODUCT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone)]
pub struct RuntimeState {
    paths: Paths,
    operation: Arc<Mutex<()>>,
}

impl RuntimeState {
    pub fn discover() -> Self {
        Self {
            paths: Paths::discover(),
            operation: Arc::new(Mutex::new(())),
        }
    }

    async fn run<T, F>(&self, operation: F) -> Result<T, ClientError>
    where
        T: Send + 'static,
        F: FnOnce(&Paths) -> Result<T, ClientError> + Send + 'static,
    {
        let paths = self.paths.clone();
        let lock = Arc::clone(&self.operation);
        tauri::async_runtime::spawn_blocking(move || {
            let _guard = lock.lock().map_err(|_| {
                ClientError::new("runtimeBusy", "the client operation lock is unavailable")
            })?;
            operation(&paths)
        })
        .await
        .map_err(|error| ClientError::new("runtimeTask", redact_diagnostic(&error.to_string())))?
    }

    pub async fn snapshot(&self, synchronize: bool) -> Result<ClientSnapshot, ClientError> {
        self.run(move |paths| load_snapshot(paths, synchronize))
            .await
    }

    pub async fn join(&self, invite_code: String) -> Result<ClientSnapshot, ClientError> {
        self.run(move |paths| {
            let invite = invite_code.trim();
            if invite.is_empty() {
                return Err(ClientError::new("inviteEmpty", "invite is empty"));
            }
            ensure_identity(paths)?;
            let output = run_node(
                paths,
                &[
                    "join",
                    "--state",
                    path_text(&paths.state)?,
                    "--invite-code",
                    invite,
                ],
            )?;
            let community_id = output_field(&output, "COMMUNITY_ID")?;
            save_config(&paths.config, Some(community_id))?;
            load_snapshot(paths, false)
        })
        .await
    }

    pub async fn send(&self, input: SendMessageInput) -> Result<ClientMessage, ClientError> {
        self.run(move |paths| {
            let config = load_config(&paths.config)?;
            let Some(configured_community) = config.community_id else {
                return Err(ClientError::new(
                    "communityMissing",
                    "no community is configured",
                ));
            };
            if configured_community != input.community_id {
                return Err(ClientError::new(
                    "communityMismatch",
                    "the selected community is not configured locally",
                ));
            }
            let body = input.body.trim();
            if body.is_empty() {
                return Err(ClientError::new("messageEmpty", "message is empty"));
            }
            let output = run_node(
                paths,
                &[
                    "send-message",
                    "--state",
                    path_text(&paths.state)?,
                    "--community",
                    &input.community_id,
                    "--channel",
                    &input.room_id,
                    "--text",
                    body,
                ],
            )?;
            let event_id = output_field(&output, "MESSAGE_EVENT_ID")?;
            let identity = identity_info(paths)?;
            Ok(ClientMessage {
                id: event_id,
                author: short_id(&identity.user_id),
                avatar: avatar_symbol(&identity.user_id),
                tone: "self",
                sent_at: "now".into(),
                body: body.into(),
                own: true,
            })
        })
        .await
    }

    pub async fn submit_feedback(
        &self,
        input: FeedbackInput,
    ) -> Result<FeedbackStatus, ClientError> {
        self.run(move |paths| submit_feedback(paths, input)).await
    }

    pub async fn feedback_status(&self) -> Result<Option<FeedbackStatus>, ClientError> {
        self.run(refresh_feedback_status).await
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientError {
    code: &'static str,
    detail: String,
}

impl ClientError {
    fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }

    fn protocol(detail: impl AsRef<str>) -> Self {
        Self::new("protocolOperation", redact_diagnostic(detail.as_ref()))
    }
}

#[derive(Debug, Clone)]
struct Paths {
    state: PathBuf,
    config: PathBuf,
    feedback: PathBuf,
    node: PathBuf,
}

impl Paths {
    fn discover() -> Self {
        let base = ProjectDirs::from("net", "ttinker", "ChatCommonsAlpha")
            .map(|dirs| dirs.data_local_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from(".chatcommons-alpha"));
        let node = std::env::var_os("CHATCOMMONS_NODE_PATH")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::current_exe().ok().and_then(|path| {
                    path.parent().map(|parent| {
                        parent.join(if cfg!(windows) {
                            "chatcommons-node.exe"
                        } else {
                            "chatcommons-node"
                        })
                    })
                })
            })
            .unwrap_or_else(|| PathBuf::from("chatcommons-node"));
        Self {
            state: base.join("node"),
            config: base.join("client.json"),
            feedback: base.join("feedback-receipt.json"),
            node,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientSnapshot {
    mode: &'static str,
    profile_name: String,
    profile_symbol: String,
    profile_id: String,
    connection: ConnectionState,
    communities: Vec<ClientCommunity>,
    messages_by_room: BTreeMap<String, Vec<ClientMessage>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConnectionState {
    status: &'static str,
    warning_code: Option<&'static str>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClientCommunity {
    id: String,
    name: String,
    symbol: String,
    accent: &'static str,
    summary: String,
    room_summary: String,
    unread: u32,
    online: u32,
    rooms: Vec<ClientRoom>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClientRoom {
    id: String,
    name: String,
    unread: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientMessage {
    id: String,
    author: String,
    avatar: String,
    tone: &'static str,
    sent_at: String,
    body: String,
    own: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SendMessageInput {
    community_id: String,
    room_id: String,
    body: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FeedbackInput {
    what_happened: String,
    expected: String,
    screen: String,
    screenshot: String,
    viewport_width: usize,
    viewport_height: usize,
    confirmed: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FeedbackStatus {
    public_id: String,
    status: String,
    #[serde(default)]
    admin_reply: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ClientConfig {
    version: u16,
    community_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Channel {
    channel_id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredMessage {
    event_id: String,
    channel_id: String,
    author_id: String,
    timestamp_ms: i64,
    text: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommunityInfo {
    community_id: String,
    name: String,
}

struct IdentityInfo {
    user_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredFeedbackReceipt {
    public_id: String,
    edit_token: String,
    status: String,
    #[serde(default)]
    admin_reply: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FeedbackPayload {
    surface: &'static str,
    screen: String,
    target_id: &'static str,
    target_text: &'static str,
    x: f32,
    y: f32,
    scroll_x: f32,
    scroll_y: f32,
    viewport_width: usize,
    viewport_height: usize,
    category: &'static str,
    priority: &'static str,
    message: String,
    screenshot: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FeedbackCreated {
    public_id: String,
    edit_token: String,
    status: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FeedbackResponse {
    public_id: String,
    status: String,
    admin_reply: String,
}

fn ensure_identity(paths: &Paths) -> Result<(), ClientError> {
    if paths.state.join("identity.json").is_file() {
        return Ok(());
    }
    let parent = paths
        .state
        .parent()
        .ok_or_else(|| ClientError::new("statePath", "application data directory is invalid"))?;
    fs::create_dir_all(parent).map_err(|error| {
        ClientError::new(
            "stateCreate",
            redact_diagnostic(&format!("could not create application data: {error}")),
        )
    })?;
    run_node(paths, &["init", "--state", path_text(&paths.state)?]).map(|_| ())
}

fn identity_info(paths: &Paths) -> Result<IdentityInfo, ClientError> {
    let info = run_node(paths, &["info", "--state", path_text(&paths.state)?])?;
    Ok(IdentityInfo {
        user_id: output_field(&info, "USER_ID")?,
    })
}

fn load_snapshot(paths: &Paths, synchronize: bool) -> Result<ClientSnapshot, ClientError> {
    ensure_identity(paths)?;
    let identity = identity_info(paths)?;
    let config = load_config(&paths.config)?;
    let Some(community_id) = config.community_id else {
        return Ok(ClientSnapshot {
            mode: "local",
            profile_name: short_id(&identity.user_id),
            profile_symbol: avatar_symbol(&identity.user_id),
            profile_id: identity.user_id,
            connection: ConnectionState {
                status: "local",
                warning_code: None,
            },
            communities: Vec::new(),
            messages_by_room: BTreeMap::new(),
        });
    };

    let sync_warning = synchronize
        .then(|| {
            run_node(
                paths,
                &[
                    "sync-home-server",
                    "--state",
                    path_text(&paths.state)?,
                    "--community",
                    &community_id,
                    "--listen",
                    "/ip4/0.0.0.0/udp/0/quic-v1",
                    "--idle-timeout-ms",
                    "1500",
                ],
            )
            .map(|_| ())
        })
        .transpose()
        .err();

    let info_output = run_node(
        paths,
        &[
            "community-info",
            "--state",
            path_text(&paths.state)?,
            "--community",
            &community_id,
        ],
    )?;
    let community_info: CommunityInfo = serde_json::from_str(&info_output)
        .map_err(|error| ClientError::new("communityData", error.to_string()))?;
    if community_info.community_id != community_id {
        return Err(ClientError::new(
            "communityMismatch",
            "community summary does not match local configuration",
        ));
    }

    let channels_output = run_node(
        paths,
        &[
            "list-channels",
            "--state",
            path_text(&paths.state)?,
            "--community",
            &community_id,
        ],
    )?;
    let messages_output = run_node(
        paths,
        &[
            "list-messages",
            "--state",
            path_text(&paths.state)?,
            "--community",
            &community_id,
        ],
    )?;
    let channels: Vec<Channel> = serde_json::from_str(&channels_output)
        .map_err(|error| ClientError::new("channelData", error.to_string()))?;
    let stored_messages: Vec<StoredMessage> = serde_json::from_str(&messages_output)
        .map_err(|error| ClientError::new("messageData", error.to_string()))?;

    let mut messages_by_room = BTreeMap::new();
    for channel in &channels {
        let key = room_key(&community_id, &channel.channel_id);
        let messages = stored_messages
            .iter()
            .filter(|message| message.channel_id == channel.channel_id)
            .map(|message| ClientMessage {
                id: message.event_id.clone(),
                author: short_id(&message.author_id),
                avatar: avatar_symbol(&message.author_id),
                tone: if message.author_id == identity.user_id {
                    "self"
                } else {
                    author_tone(&message.author_id)
                },
                sent_at: message_time(message.timestamp_ms),
                body: message.text.clone(),
                own: message.author_id == identity.user_id,
            })
            .collect();
        messages_by_room.insert(key, messages);
    }

    let latest = stored_messages.last();
    let rooms: Vec<ClientRoom> = channels
        .iter()
        .map(|channel| ClientRoom {
            id: channel.channel_id.clone(),
            name: channel.name.clone(),
            unread: 0,
        })
        .collect();
    let room_summary = channels
        .first()
        .map(|channel| channel.name.clone())
        .unwrap_or_default();
    let summary = latest
        .map(|message| message.text.clone())
        .unwrap_or_else(|| community_info.name.clone());
    let symbol = community_info
        .name
        .chars()
        .find(|character| !character.is_whitespace())
        .map(|character| character.to_string())
        .unwrap_or_else(|| "C".into());

    Ok(ClientSnapshot {
        mode: "local",
        profile_name: short_id(&identity.user_id),
        profile_symbol: avatar_symbol(&identity.user_id),
        profile_id: identity.user_id,
        connection: ConnectionState {
            status: if sync_warning.is_some() {
                "degraded"
            } else {
                "connected"
            },
            warning_code: sync_warning.as_ref().map(|_| "homeServerUnavailable"),
        },
        communities: vec![ClientCommunity {
            id: community_id,
            name: community_info.name,
            symbol,
            accent: "coral",
            summary,
            room_summary,
            unread: 0,
            online: 0,
            rooms,
        }],
        messages_by_room,
    })
}

fn load_config(path: &Path) -> Result<ClientConfig, ClientError> {
    if !path.is_file() {
        return Ok(ClientConfig {
            version: CONFIG_VERSION,
            community_id: None,
        });
    }
    let bytes = fs::read(path)
        .map_err(|error| ClientError::new("configRead", redact_diagnostic(&error.to_string())))?;
    let config: ClientConfig = serde_json::from_slice(&bytes)
        .map_err(|error| ClientError::new("configInvalid", error.to_string()))?;
    if config.version != CONFIG_VERSION {
        return Err(ClientError::new(
            "configVersion",
            "client configuration version is unsupported",
        ));
    }
    Ok(config)
}

fn save_config(path: &Path, community_id: Option<String>) -> Result<(), ClientError> {
    let parent = path
        .parent()
        .ok_or_else(|| ClientError::new("configPath", "client configuration path is invalid"))?;
    fs::create_dir_all(parent)
        .map_err(|error| ClientError::new("configCreate", redact_diagnostic(&error.to_string())))?;
    let bytes = serde_json::to_vec(&ClientConfig {
        version: CONFIG_VERSION,
        community_id,
    })
    .map_err(|error| ClientError::new("configEncode", error.to_string()))?;
    fs::write(path, bytes)
        .map_err(|error| ClientError::new("configWrite", redact_diagnostic(&error.to_string())))
}

fn run_node(paths: &Paths, arguments: &[&str]) -> Result<String, ClientError> {
    let output = Command::new(&paths.node)
        .args(arguments)
        .output()
        .map_err(|error| {
            ClientError::new(
                "nodeUnavailable",
                redact_diagnostic(&format!("could not start protocol process: {error}")),
            )
        })?;
    if output.status.success() {
        String::from_utf8(output.stdout)
            .map_err(|error| ClientError::new("nodeOutput", error.to_string()))
    } else {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        Err(ClientError::protocol(if detail.is_empty() {
            "protocol operation failed"
        } else {
            &detail
        }))
    }
}

fn output_field(output: &str, name: &'static str) -> Result<String, ClientError> {
    let prefix = format!("{name}=");
    output
        .lines()
        .find_map(|line| line.strip_prefix(&prefix).map(str::to_owned))
        .ok_or_else(|| ClientError::new("nodeOutput", format!("protocol output omitted {name}")))
}

fn path_text(path: &Path) -> Result<&str, ClientError> {
    path.to_str()
        .ok_or_else(|| ClientError::new("statePath", "application path is not valid UTF-8"))
}

fn room_key(community_id: &str, room_id: &str) -> String {
    format!("{community_id}:{room_id}")
}

fn short_id(value: &str) -> String {
    value.chars().take(10).collect()
}

fn avatar_symbol(value: &str) -> String {
    value
        .chars()
        .next()
        .map(|character| character.to_ascii_uppercase().to_string())
        .unwrap_or_else(|| "C".into())
}

fn author_tone(value: &str) -> &'static str {
    match value.bytes().fold(0_u8, u8::wrapping_add) % 3 {
        0 => "coral",
        1 => "blue",
        _ => "green",
    }
}

fn message_time(timestamp_ms: i64) -> String {
    let seconds = (timestamp_ms / 1000).rem_euclid(86_400);
    format!("{:02}:{:02} UTC", seconds / 3_600, (seconds % 3_600) / 60)
}

fn redact_diagnostic(value: &str) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let value = if home.is_empty() {
        value.to_owned()
    } else {
        value.replace(&home, "$HOME")
    };
    let mut output = String::with_capacity(value.len());
    let mut run = String::new();
    let flush = |run: &mut String, output: &mut String| {
        if run.starts_with("cc1_") {
            output.push_str("[redacted invite]");
        } else if run.len() >= 32
            && run
                .chars()
                .all(|character| character.is_ascii_alphanumeric())
        {
            output.push_str("[redacted identifier]");
        } else {
            output.push_str(run);
        }
        run.clear();
    };
    for character in value.chars() {
        if character.is_ascii_alphanumeric() || character == '_' {
            run.push(character);
        } else {
            flush(&mut run, &mut output);
            output.push(character);
        }
    }
    flush(&mut run, &mut output);
    output
        .split_whitespace()
        .map(|token| {
            let candidate = token.trim_start_matches(['"', '\'', '(', '[', '{', '=', ':']);
            let is_network_multiaddr = [
                "/ip4/", "/ip6/", "/dns/", "/dns4/", "/dns6/", "/udp/", "/tcp/", "/p2p/",
            ]
            .iter()
            .any(|prefix| candidate.starts_with(prefix));
            let bytes = candidate.as_bytes();
            let is_windows_path =
                bytes.len() >= 3 && bytes[1] == b':' && matches!(bytes[2], b'\\' | b'/');
            if candidate.starts_with("$HOME")
                || candidate.starts_with("file://")
                || (candidate.starts_with('/') && !is_network_multiaddr)
                || is_windows_path
            {
                "[redacted path]"
            } else {
                token
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn submit_feedback(paths: &Paths, input: FeedbackInput) -> Result<FeedbackStatus, ClientError> {
    if !input.confirmed {
        return Err(ClientError::new(
            "feedbackConfirmation",
            "feedback was not confirmed",
        ));
    }
    if input.what_happened.trim().is_empty() || input.expected.trim().is_empty() {
        return Err(ClientError::new(
            "feedbackIncomplete",
            "feedback requires what happened and the expected result",
        ));
    }
    if !input.screenshot.is_empty()
        && !(input.screenshot.starts_with("data:image/jpeg;base64,")
            || input.screenshot.starts_with("data:image/png;base64,"))
    {
        return Err(ClientError::new(
            "feedbackScreenshot",
            "screenshot must be a JPEG or PNG data URL",
        ));
    }
    let report = format!(
        "## What happened\n\n{}\n\n## What did you expect?\n\n{}\n\n## App diagnostics\n\n- Version: `{PRODUCT_VERSION}`\n- OS: `{}`\n- Architecture: `{}`\n- Screen: `{}`\n\n## Privacy confirmation\n\nThe reporter reviewed this text and the optional screenshot before submitting them to the private ChatCommons feedback inbox. Chat messages, invite links, identity keys, identifiers, and local paths are not included in generated diagnostics.\n",
        input.what_happened.trim(),
        input.expected.trim(),
        std::env::consts::OS,
        std::env::consts::ARCH,
        redact_diagnostic(&input.screen),
    );
    let payload = FeedbackPayload {
        surface: "desktop",
        screen: redact_diagnostic(&input.screen),
        target_id: "chat-window",
        target_text: "ChatCommons desktop feedback",
        x: 0.5,
        y: 0.5,
        scroll_x: 0.0,
        scroll_y: 0.0,
        viewport_width: input.viewport_width.clamp(280, 10_000),
        viewport_height: input.viewport_height.clamp(300, 10_000),
        category: "feature",
        priority: "normal",
        message: report,
        screenshot: input.screenshot,
    };
    let mut response = ureq::post(FEEDBACK_ENDPOINT)
        .header(
            "User-Agent",
            &format!("ChatCommonsDesktop/{PRODUCT_VERSION}"),
        )
        .send_json(&payload)
        .map_err(|error| {
            ClientError::new(
                "feedbackSend",
                redact_diagnostic(&format!("feedback request failed: {error}")),
            )
        })?;
    let created: FeedbackCreated = response
        .body_mut()
        .read_json()
        .map_err(|error| ClientError::new("feedbackReceipt", error.to_string()))?;
    let receipt = StoredFeedbackReceipt {
        public_id: created.public_id,
        edit_token: created.edit_token,
        status: created.status,
        admin_reply: String::new(),
    };
    save_feedback_receipt(&paths.feedback, &receipt)?;
    Ok(FeedbackStatus {
        public_id: receipt.public_id,
        status: receipt.status,
        admin_reply: receipt.admin_reply,
    })
}

fn refresh_feedback_status(paths: &Paths) -> Result<Option<FeedbackStatus>, ClientError> {
    let Some(mut receipt) = load_feedback_receipt(&paths.feedback)? else {
        return Ok(None);
    };
    let endpoint = format!("{FEEDBACK_ENDPOINT}/{}", receipt.public_id);
    let mut response = ureq::get(&endpoint)
        .header("X-Edit-Token", &receipt.edit_token)
        .header(
            "User-Agent",
            &format!("ChatCommonsDesktop/{PRODUCT_VERSION}"),
        )
        .call()
        .map_err(|error| {
            ClientError::new(
                "feedbackStatus",
                redact_diagnostic(&format!("feedback status request failed: {error}")),
            )
        })?;
    let status: FeedbackResponse = response
        .body_mut()
        .read_json()
        .map_err(|error| ClientError::new("feedbackStatus", error.to_string()))?;
    if status.public_id != receipt.public_id {
        return Err(ClientError::new(
            "feedbackReceipt",
            "feedback receipt identifier does not match",
        ));
    }
    receipt.status = status.status;
    receipt.admin_reply = status.admin_reply;
    save_feedback_receipt(&paths.feedback, &receipt)?;
    Ok(Some(FeedbackStatus {
        public_id: receipt.public_id,
        status: receipt.status,
        admin_reply: receipt.admin_reply,
    }))
}

fn load_feedback_receipt(path: &Path) -> Result<Option<StoredFeedbackReceipt>, ClientError> {
    if !path.is_file() {
        return Ok(None);
    }
    let bytes = fs::read(path).map_err(|error| {
        ClientError::new(
            "feedbackReceipt",
            redact_diagnostic(&format!("could not read feedback receipt: {error}")),
        )
    })?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| ClientError::new("feedbackReceipt", error.to_string()))
}

fn save_feedback_receipt(path: &Path, receipt: &StoredFeedbackReceipt) -> Result<(), ClientError> {
    let parent = path.parent().ok_or_else(|| {
        ClientError::new("feedbackReceipt", "feedback receipt directory is invalid")
    })?;
    fs::create_dir_all(parent).map_err(|error| {
        ClientError::new(
            "feedbackReceipt",
            redact_diagnostic(&format!(
                "could not create feedback receipt directory: {error}"
            )),
        )
    })?;
    let bytes = serde_json::to_vec(receipt)
        .map_err(|error| ClientError::new("feedbackReceipt", error.to_string()))?;
    fs::write(path, bytes).map_err(|error| {
        ClientError::new(
            "feedbackReceipt",
            redact_diagnostic(&format!("could not save feedback receipt: {error}")),
        )
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|error| {
            ClientError::new(
                "feedbackReceipt",
                redact_diagnostic(&format!("could not protect feedback receipt: {error}")),
            )
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_fields_require_the_complete_name() {
        let output = "USER_ID=abc\nCOMMUNITY_ID=def\n";
        assert_eq!(
            output_field(output, "USER_ID").map_err(|e| e.code),
            Ok("abc".into())
        );
        assert!(output_field(output, "ID").is_err());
    }

    #[test]
    fn display_helpers_do_not_change_protocol_identity() {
        assert_eq!(short_id("0123456789abcdef"), "0123456789");
        assert_eq!(room_key("community", "room"), "community:room");
        assert_eq!(message_time(3_661_000), "01:01 UTC");
    }

    #[test]
    fn diagnostics_remove_invites_identifiers_and_paths() {
        let input = "cc1_secret 0123456789abcdef0123456789abcdef0123456789abcdef /tmp/private";
        let output = redact_diagnostic(input);
        assert!(!output.contains("cc1_secret"));
        assert!(!output.contains("0123456789abcdef"));
        assert!(!output.contains("/tmp/private"));
    }

    #[test]
    fn config_round_trips_without_external_input_unwraps() -> Result<(), Box<dyn std::error::Error>>
    {
        let temporary = tempfile::tempdir()?;
        let path = temporary.path().join("client.json");
        save_config(&path, Some("ab".repeat(32))).map_err(|error| error.detail)?;
        let loaded = load_config(&path).map_err(|error| error.detail)?;
        assert_eq!(
            loaded.community_id.as_deref(),
            Some("ab".repeat(32).as_str())
        );
        Ok(())
    }
}
