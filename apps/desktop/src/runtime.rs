use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tempfile::NamedTempFile;

const CONFIG_VERSION: u16 = 1;
const FEEDBACK_ENDPOINT: &str = "https://ttinker.net/chatcommons/api/app-feedback";
const PRODUCT_VERSION: &str = env!("CARGO_PKG_VERSION");
const SYNC_IDLE_TIMEOUT_MS: &str = "1500";
const SYNC_OVERALL_TIMEOUT_MS: &str = "5000";
const SYNC_PROCESS_TIMEOUT: Duration = Duration::from_secs(7);
const JOIN_OVERALL_TIMEOUT_MS: &str = "15000";
const JOIN_PROCESS_TIMEOUT: Duration = Duration::from_secs(17);
const MESSAGE_PROCESS_TIMEOUT: Duration = Duration::from_secs(10);
const INVITATION_PROCESS_TIMEOUT: Duration = Duration::from_secs(10);
const VOICE_TOKEN_OVERALL_TIMEOUT_MS: &str = "8000";
const VOICE_TOKEN_PROCESS_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_VOICE_TOKEN_BYTES: usize = 16 * 1024;
const MAX_VOICE_SERVER_URL_BYTES: usize = 2 * 1024;
const FEEDBACK_REQUEST_TIMEOUT: Duration = Duration::from_secs(12);
const FEEDBACK_CONNECT_TIMEOUT: Duration = Duration::from_secs(4);
const FEEDBACK_RESPONSE_LIMIT: u64 = 64 * 1024;
const MAX_FEEDBACK_SCREENSHOT_BYTES: usize = 2 * 1024 * 1024;
const CLIENT_MESSAGE_LIMIT: &str = "500";
const MAX_LOCAL_METADATA_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone)]
pub struct RuntimeState {
    paths: Paths,
    operation: Arc<Mutex<()>>,
    feedback_agent: ureq::Agent,
}

impl RuntimeState {
    pub fn discover() -> Self {
        Self {
            paths: Paths::discover(),
            operation: Arc::new(Mutex::new(())),
            feedback_agent: feedback_agent(),
        }
    }

    async fn run_serialized<T, F>(&self, operation: F) -> Result<T, ClientError>
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

    pub async fn local_snapshot(&self) -> Result<ClientSnapshot, ClientError> {
        self.run_serialized(|paths| load_snapshot(paths, SnapshotConnection::Local))
            .await
    }

    pub async fn synchronized_snapshot(&self) -> Result<ClientSnapshot, ClientError> {
        self.run_serialized(|paths| {
            let connection = match synchronize_with_home_server(paths) {
                Ok(()) => SnapshotConnection::Connected,
                // A failed sync never invalidates the already-validated local DAG.
                // The UI receives a degraded snapshot instead of losing local access.
                Err(_) => SnapshotConnection::Degraded,
            };
            load_snapshot(paths, connection)
        })
        .await
    }

    pub async fn join(&self, invite_code: String) -> Result<ClientSnapshot, ClientError> {
        self.run_serialized(move |paths| {
            let invite = invite_code.trim();
            if invite.is_empty() {
                return Err(ClientError::new("inviteEmpty", "invite is empty"));
            }
            ensure_identity(paths)?;
            let output = run_node_with_input(
                paths,
                &[
                    "join",
                    "--state",
                    path_text(&paths.state)?,
                    "--stdin-field",
                    "invite-code",
                    "--overall-timeout-ms",
                    JOIN_OVERALL_TIMEOUT_MS,
                ],
                invite.as_bytes(),
                JOIN_PROCESS_TIMEOUT,
            )?;
            let community_id = output_field(&output, "COMMUNITY_ID")?;
            save_config(&paths.config, Some(community_id))?;
            load_snapshot(paths, SnapshotConnection::Local)
        })
        .await
    }

    pub async fn send(&self, input: SendMessageInput) -> Result<ClientMessage, ClientError> {
        self.run_serialized(move |paths| {
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
            let output = run_node_with_input(
                paths,
                &[
                    "send-message",
                    "--state",
                    path_text(&paths.state)?,
                    "--community",
                    &input.community_id,
                    "--channel",
                    &input.room_id,
                    "--stdin-field",
                    "text",
                ],
                body.as_bytes(),
                MESSAGE_PROCESS_TIMEOUT,
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

    pub async fn create_invitation(
        &self,
        input: CreateInvitationInput,
    ) -> Result<ClientInvitation, ClientError> {
        self.run_serialized(move |paths| create_ready_invitation(paths, input))
            .await
    }

    pub async fn voice_token(&self, input: VoiceTokenInput) -> Result<VoiceGrant, ClientError> {
        self.run_serialized(move |paths| {
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
            let output = run_node_with_timeout(
                paths,
                &[
                    "voice-token",
                    "--state",
                    path_text(&paths.state)?,
                    "--community",
                    &input.community_id,
                    "--channel",
                    &input.room_id,
                    "--overall-timeout-ms",
                    VOICE_TOKEN_OVERALL_TIMEOUT_MS,
                ],
                VOICE_TOKEN_PROCESS_TIMEOUT,
            )?;
            let grant: VoiceGrant = serde_json::from_str(&output)
                .map_err(|error| ClientError::new("voiceGrant", error.to_string()))?;
            validate_voice_grant(&grant)?;
            Ok(grant)
        })
        .await
    }

    pub async fn submit_feedback(
        &self,
        input: FeedbackInput,
    ) -> Result<FeedbackStatus, ClientError> {
        let paths = self.paths.clone();
        let agent = self.feedback_agent.clone();
        tauri::async_runtime::spawn_blocking(move || submit_feedback(&agent, &paths, input))
            .await
            .map_err(|error| {
                ClientError::new("runtimeTask", redact_diagnostic(&error.to_string()))
            })?
    }

    pub async fn feedback_status(&self) -> Result<Option<FeedbackStatus>, ClientError> {
        let paths = self.paths.clone();
        let agent = self.feedback_agent.clone();
        tauri::async_runtime::spawn_blocking(move || refresh_feedback_status(&agent, &paths))
            .await
            .map_err(|error| {
                ClientError::new("runtimeTask", redact_diagnostic(&error.to_string()))
            })?
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

#[derive(Clone, Copy)]
enum SnapshotConnection {
    Local,
    Connected,
    Degraded,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClientCommunity {
    id: String,
    name: String,
    symbol: String,
    accent: &'static str,
    can_invite: bool,
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
pub struct CreateInvitationInput {
    community_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientInvitation {
    code: String,
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
pub struct VoiceTokenInput {
    community_id: String,
    room_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VoiceGrant {
    #[serde(alias = "server_url")]
    pub server_url: String,
    #[serde(alias = "participant_token")]
    pub participant_token: String,
    #[serde(alias = "expires_at_ms")]
    pub expires_at_ms: i64,
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
    #[serde(default)]
    can_invite: bool,
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

fn synchronize_with_home_server(paths: &Paths) -> Result<(), ClientError> {
    let config = load_or_recover_config(paths)?;
    let community_id = config
        .community_id
        .ok_or_else(|| ClientError::new("communityMissing", "no community is configured"))?;
    run_node_with_timeout(
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
            SYNC_IDLE_TIMEOUT_MS,
            "--overall-timeout-ms",
            SYNC_OVERALL_TIMEOUT_MS,
        ],
        SYNC_PROCESS_TIMEOUT,
    )
    .map(|_| ())
}

fn create_ready_invitation(
    paths: &Paths,
    input: CreateInvitationInput,
) -> Result<ClientInvitation, ClientError> {
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

    synchronize_with_home_server(paths).map_err(|_| {
        ClientError::new(
            "inviteServerUnavailable",
            "the Community Home Server must be reachable before creating an invite",
        )
    })?;
    let output = run_node_with_timeout(
        paths,
        &[
            "create-invite",
            "--state",
            path_text(&paths.state)?,
            "--community",
            &input.community_id,
        ],
        INVITATION_PROCESS_TIMEOUT,
    )?;
    let code = output_field(&output, "INVITE_CODE")?;
    synchronize_with_home_server(paths).map_err(|_| {
        ClientError::new(
            "invitePublish",
            "the signed invite was not published to the Community Home Server",
        )
    })?;
    Ok(ClientInvitation { code })
}

fn load_snapshot(
    paths: &Paths,
    connection: SnapshotConnection,
) -> Result<ClientSnapshot, ClientError> {
    ensure_identity(paths)?;
    let identity = identity_info(paths)?;
    let config = load_or_recover_config(paths)?;
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
            "--limit",
            CLIENT_MESSAGE_LIMIT,
        ],
    )?;
    let channels: Vec<Channel> = serde_json::from_str(&channels_output)
        .map_err(|error| ClientError::new("channelData", error.to_string()))?;
    let stored_messages: Vec<StoredMessage> = serde_json::from_str(&messages_output)
        .map_err(|error| ClientError::new("messageData", error.to_string()))?;

    let mut messages_by_room = BTreeMap::new();
    let mut message_room_keys = BTreeMap::new();
    for channel in &channels {
        let key = room_key(&community_id, &channel.channel_id);
        message_room_keys.insert(channel.channel_id.as_str(), key.clone());
        messages_by_room.insert(key, Vec::new());
    }
    for message in &stored_messages {
        let Some(key) = message_room_keys.get(message.channel_id.as_str()) else {
            continue;
        };
        let own = message.author_id == identity.user_id;
        messages_by_room
            .get_mut(key)
            .ok_or_else(|| ClientError::new("messageData", "message room index is inconsistent"))?
            .push(ClientMessage {
                id: message.event_id.clone(),
                author: short_id(&message.author_id),
                avatar: avatar_symbol(&message.author_id),
                tone: if own {
                    "self"
                } else {
                    author_tone(&message.author_id)
                },
                sent_at: message_time(message.timestamp_ms),
                body: message.text.clone(),
                own,
            });
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
            status: match connection {
                SnapshotConnection::Local => "local",
                SnapshotConnection::Connected => "connected",
                SnapshotConnection::Degraded => "degraded",
            },
            warning_code: matches!(connection, SnapshotConnection::Degraded)
                .then_some("homeServerUnavailable"),
        },
        communities: vec![ClientCommunity {
            id: community_id,
            name: community_info.name,
            symbol,
            accent: "coral",
            can_invite: community_info.can_invite,
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
    let bytes = read_bounded_file(path, MAX_LOCAL_METADATA_BYTES, "configRead")?;
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

fn load_or_recover_config(paths: &Paths) -> Result<ClientConfig, ClientError> {
    let config = load_config(&paths.config)?;
    if config.community_id.is_some() {
        return Ok(config);
    }
    let output = run_node(
        paths,
        &[
            "list-joined-communities",
            "--state",
            path_text(&paths.state)?,
        ],
    )?;
    let joined: Vec<String> = serde_json::from_str(&output)
        .map_err(|error| ClientError::new("communityData", error.to_string()))?;
    match joined.as_slice() {
        [] => Ok(config),
        [community_id] => {
            save_config(&paths.config, Some(community_id.clone()))?;
            Ok(ClientConfig {
                version: CONFIG_VERSION,
                community_id: Some(community_id.clone()),
            })
        }
        _ => Err(ClientError::new(
            "communityData",
            "multiple local communities require an explicit client selection",
        )),
    }
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
    atomic_write(path, &bytes, "configWrite")
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
    parse_node_output(output)
}

fn run_node_with_input(
    paths: &Paths,
    arguments: &[&str],
    input: &[u8],
    timeout: Duration,
) -> Result<String, ClientError> {
    let mut child = Command::new(&paths.node)
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(node_start_error)?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| ClientError::new("nodeInput", "protocol input pipe is unavailable"))?;
    stdin
        .write_all(input)
        .map_err(|error| ClientError::new("nodeInput", redact_diagnostic(&error.to_string())))?;
    drop(stdin);
    parse_node_output(wait_with_timeout(child, timeout)?)
}

fn run_node_with_timeout(
    paths: &Paths,
    arguments: &[&str],
    timeout: Duration,
) -> Result<String, ClientError> {
    let child = Command::new(&paths.node)
        .args(arguments)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(node_start_error)?;
    parse_node_output(wait_with_timeout(child, timeout)?)
}

fn wait_with_timeout(
    mut child: std::process::Child,
    timeout: Duration,
) -> Result<Output, ClientError> {
    let deadline = Instant::now() + timeout;
    loop {
        if child
            .try_wait()
            .map_err(|error| ClientError::new("nodeWait", redact_diagnostic(&error.to_string())))?
            .is_some()
        {
            return child.wait_with_output().map_err(|error| {
                ClientError::new("nodeWait", redact_diagnostic(&error.to_string()))
            });
        }
        if Instant::now() >= deadline {
            if let Err(kill_error) = child.kill() {
                let still_running = child
                    .try_wait()
                    .map_err(|wait_error| {
                        ClientError::new(
                            "nodeTerminate",
                            redact_diagnostic(&format!(
                                "could not inspect timed-out protocol process: {wait_error}"
                            )),
                        )
                    })?
                    .is_none();
                if still_running {
                    return Err(ClientError::new(
                        "nodeTerminate",
                        redact_diagnostic(&format!(
                            "could not terminate timed-out protocol process: {kill_error}"
                        )),
                    ));
                }
            }
            child.wait().map_err(|error| {
                ClientError::new(
                    "nodeTerminate",
                    redact_diagnostic(&format!(
                        "could not reap timed-out protocol process: {error}"
                    )),
                )
            })?;
            return Err(ClientError::new(
                "nodeTimeout",
                "protocol operation exceeded its local deadline",
            ));
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn node_start_error(error: std::io::Error) -> ClientError {
    ClientError::new(
        "nodeUnavailable",
        redact_diagnostic(&format!("could not start protocol process: {error}")),
    )
}

fn parse_node_output(output: Output) -> Result<String, ClientError> {
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

fn feedback_agent() -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(FEEDBACK_REQUEST_TIMEOUT))
        .timeout_connect(Some(FEEDBACK_CONNECT_TIMEOUT))
        .build();
    config.into()
}

fn submit_feedback(
    agent: &ureq::Agent,
    paths: &Paths,
    input: FeedbackInput,
) -> Result<FeedbackStatus, ClientError> {
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
    let screenshot_is_supported = input.screenshot.is_empty()
        || input.screenshot.starts_with("data:image/jpeg;base64,")
        || input.screenshot.starts_with("data:image/png;base64,");
    if !screenshot_is_supported {
        return Err(ClientError::new(
            "feedbackScreenshot",
            "screenshot must be a JPEG or PNG data URL",
        ));
    }
    if input.screenshot.len() > MAX_FEEDBACK_SCREENSHOT_BYTES {
        return Err(ClientError::new(
            "feedbackScreenshot",
            "screenshot exceeds the feedback upload limit",
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
    let mut response = agent
        .post(FEEDBACK_ENDPOINT)
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
        .with_config()
        .limit(FEEDBACK_RESPONSE_LIMIT)
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

fn refresh_feedback_status(
    agent: &ureq::Agent,
    paths: &Paths,
) -> Result<Option<FeedbackStatus>, ClientError> {
    let Some(mut receipt) = load_feedback_receipt(&paths.feedback)? else {
        return Ok(None);
    };
    let endpoint = format!("{FEEDBACK_ENDPOINT}/{}", receipt.public_id);
    let mut response = agent
        .get(&endpoint)
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
        .with_config()
        .limit(FEEDBACK_RESPONSE_LIMIT)
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
    let bytes = read_bounded_file(path, MAX_LOCAL_METADATA_BYTES, "feedbackReceipt")?;
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
    atomic_write(path, &bytes, "feedbackReceipt")?;
    Ok(())
}

fn validate_voice_grant(grant: &VoiceGrant) -> Result<(), ClientError> {
    let valid_url = grant.server_url.starts_with("wss://")
        && grant.server_url.len() <= MAX_VOICE_SERVER_URL_BYTES
        && !grant.server_url.chars().any(char::is_whitespace);
    let valid_token = !grant.participant_token.is_empty()
        && grant.participant_token.len() <= MAX_VOICE_TOKEN_BYTES
        && !grant.participant_token.chars().any(char::is_whitespace);
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok());
    let valid_expiry = now_ms.is_some_and(|now| grant.expires_at_ms > now + 5_000);
    if !valid_url || !valid_token || !valid_expiry {
        return Err(ClientError::new(
            "voiceGrant",
            "voice service returned an invalid grant",
        ));
    }
    Ok(())
}

fn atomic_write(path: &Path, bytes: &[u8], error_code: &'static str) -> Result<(), ClientError> {
    let parent = path
        .parent()
        .ok_or_else(|| ClientError::new(error_code, "destination directory is invalid"))?;
    let mut temporary = NamedTempFile::new_in(parent).map_err(|error| {
        ClientError::new(
            error_code,
            redact_diagnostic(&format!("could not create temporary file: {error}")),
        )
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        temporary
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|error| {
                ClientError::new(
                    error_code,
                    redact_diagnostic(&format!("could not protect temporary file: {error}")),
                )
            })?;
    }
    temporary.write_all(bytes).map_err(|error| {
        ClientError::new(
            error_code,
            redact_diagnostic(&format!("could not write temporary file: {error}")),
        )
    })?;
    temporary.as_file().sync_all().map_err(|error| {
        ClientError::new(
            error_code,
            redact_diagnostic(&format!("could not synchronize temporary file: {error}")),
        )
    })?;
    temporary.persist(path).map_err(|error| {
        ClientError::new(
            error_code,
            redact_diagnostic(&format!("could not replace destination file: {error}")),
        )
    })?;
    #[cfg(unix)]
    fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            ClientError::new(
                error_code,
                redact_diagnostic(&format!(
                    "could not synchronize destination directory: {error}"
                )),
            )
        })?;
    Ok(())
}

fn read_bounded_file(
    path: &Path,
    max_bytes: usize,
    error_code: &'static str,
) -> Result<Vec<u8>, ClientError> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|error| ClientError::new(error_code, redact_diagnostic(&error.to_string())))?
        .take((max_bytes + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| ClientError::new(error_code, redact_diagnostic(&error.to_string())))?;
    if bytes.len() > max_bytes {
        return Err(ClientError::new(
            error_code,
            format!("local metadata exceeds {max_bytes} bytes"),
        ));
    }
    Ok(bytes)
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
        save_config(&path, Some("cd".repeat(32))).map_err(|error| error.detail)?;
        let loaded = load_config(&path).map_err(|error| error.detail)?;
        assert_eq!(
            loaded.community_id.as_deref(),
            Some("cd".repeat(32).as_str())
        );
        Ok(())
    }

    #[test]
    fn oversized_local_metadata_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
        let temporary = tempfile::tempdir()?;
        let path = temporary.path().join("oversized.json");
        fs::write(&path, vec![0; MAX_LOCAL_METADATA_BYTES + 1])?;

        let error = load_config(&path).expect_err("oversized metadata must be rejected");

        assert_eq!(error.code, "configRead");
        Ok(())
    }

    #[test]
    fn voice_grants_accept_protocol_json_and_serialize_for_the_client()
    -> Result<(), Box<dyn std::error::Error>> {
        let expires_at_ms =
            i64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())? + 60_000;
        let grant: VoiceGrant = serde_json::from_value(serde_json::json!({
            "server_url": "wss://voice.example.test",
            "participant_token": "header.payload.signature",
            "expires_at_ms": expires_at_ms,
        }))?;
        validate_voice_grant(&grant).map_err(|error| error.detail)?;
        let client_json = serde_json::to_value(&grant)?;
        assert_eq!(
            client_json["serverUrl"],
            serde_json::Value::String("wss://voice.example.test".into())
        );
        assert!(client_json.get("server_url").is_none());

        let invalid = VoiceGrant {
            server_url: "http://voice.example.test".into(),
            participant_token: "token with spaces".into(),
            expires_at_ms: 0,
        };
        assert_eq!(
            validate_voice_grant(&invalid)
                .expect_err("invalid grant must be rejected")
                .code,
            "voiceGrant"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn snapshot_limits_message_listing_not_community_metadata()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt as _;

        let temporary = tempfile::tempdir()?;
        let state = temporary.path().join("state");
        fs::create_dir_all(&state)?;
        fs::write(state.join("identity.json"), b"test identity marker")?;
        let config = temporary.path().join("client.json");
        let community_id = "ab".repeat(32);
        let channel_id = "cd".repeat(32);
        save_config(&config, Some(community_id.clone())).map_err(|error| error.detail)?;
        let executable = temporary.path().join("fake-node");
        fs::write(
            &executable,
            format!(
                r#"#!/bin/sh
case "$1" in
  info)
    printf 'USER_ID={user_id}\n'
    ;;
  community-info)
    if [ "$6" = "--limit" ]; then exit 9; fi
    printf '{{"communityId":"{community_id}","name":"Test"}}\n'
    ;;
  list-channels)
    printf '[{{"channelId":"{channel_id}","name":"general"}}]\n'
    ;;
  list-messages)
    if [ "$6" != "--limit" ] || [ "$7" != "{CLIENT_MESSAGE_LIMIT}" ]; then exit 10; fi
    printf '[]\n'
    ;;
  *)
    exit 11
    ;;
esac
"#,
                user_id = "ef".repeat(32),
            ),
        )?;
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700))?;
        let paths = Paths {
            state,
            config,
            feedback: temporary.path().join("feedback.json"),
            node: executable,
        };

        let snapshot =
            load_snapshot(&paths, SnapshotConnection::Local).map_err(|error| error.detail)?;

        assert_eq!(snapshot.communities.len(), 1);
        assert_eq!(snapshot.communities[0].rooms.len(), 1);
        assert_eq!(
            snapshot
                .messages_by_room
                .get(&room_key(&community_id, &channel_id))
                .map(Vec::len),
            Some(0)
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn sensitive_node_input_uses_stdin_instead_of_arguments()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt as _;

        let temporary = tempfile::tempdir()?;
        let arguments_path = temporary.path().join("arguments");
        let stdin_path = temporary.path().join("stdin");
        let executable = temporary.path().join("fake-node");
        fs::write(
            &executable,
            format!(
                "#!/bin/sh\nprintf '%s' \"$*\" > '{}'\ncat > '{}'\nprintf 'MESSAGE_EVENT_ID=test\\n'\n",
                arguments_path.display(),
                stdin_path.display(),
            ),
        )?;
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700))?;
        let paths = Paths {
            state: temporary.path().join("state"),
            config: temporary.path().join("client.json"),
            feedback: temporary.path().join("feedback.json"),
            node: executable,
        };
        let secret = b"message that must not enter argv";

        let output = run_node_with_input(
            &paths,
            &["send-message", "--stdin-field", "text"],
            secret,
            Duration::from_secs(1),
        )
        .map_err(|error| error.detail)?;

        assert_eq!(
            output_field(&output, "MESSAGE_EVENT_ID").map_err(|error| error.detail)?,
            "test"
        );
        assert!(!fs::read_to_string(arguments_path)?.contains("message that must not enter argv"));
        assert_eq!(fs::read(stdin_path)?, secret);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn invitation_is_returned_only_after_two_successful_server_syncs()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt as _;

        let temporary = tempfile::tempdir()?;
        let community_id = "ab".repeat(32);
        let config = temporary.path().join("client.json");
        save_config(&config, Some(community_id.clone())).map_err(|error| error.detail)?;
        let executable = temporary.path().join("fake-node");
        let calls = temporary.path().join("calls");
        fs::write(
            &executable,
            format!(
                r#"#!/bin/sh
printf '%s\n' "$1" >> '{}'
case "$1" in
  sync-home-server)
    printf 'SYNCHRONIZED=1\n'
    ;;
  create-invite)
    printf 'INVITE_CODE=cc1_private_test_invite\n'
    ;;
  *)
    exit 9
    ;;
esac
"#,
                calls.display(),
            ),
        )?;
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700))?;
        let paths = Paths {
            state: temporary.path().join("state"),
            config,
            feedback: temporary.path().join("feedback.json"),
            node: executable,
        };

        let invitation = create_ready_invitation(
            &paths,
            CreateInvitationInput {
                community_id: community_id.clone(),
            },
        )
        .map_err(|error| error.detail)?;

        assert_eq!(invitation.code, "cc1_private_test_invite");
        assert_eq!(
            fs::read_to_string(calls)?.lines().collect::<Vec<_>>(),
            ["sync-home-server", "create-invite", "sync-home-server"]
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn unavailable_server_prevents_local_invitation_creation()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt as _;

        let temporary = tempfile::tempdir()?;
        let community_id = "ab".repeat(32);
        let config = temporary.path().join("client.json");
        save_config(&config, Some(community_id.clone())).map_err(|error| error.detail)?;
        let executable = temporary.path().join("fake-node");
        let calls = temporary.path().join("calls");
        fs::write(
            &executable,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$1\" >> '{}'\nexit 1\n",
                calls.display(),
            ),
        )?;
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700))?;
        let paths = Paths {
            state: temporary.path().join("state"),
            config,
            feedback: temporary.path().join("feedback.json"),
            node: executable,
        };

        let error = create_ready_invitation(
            &paths,
            CreateInvitationInput {
                community_id: community_id.clone(),
            },
        )
        .expect_err("offline servers must reject invite creation");

        assert_eq!(error.code, "inviteServerUnavailable");
        assert_eq!(fs::read_to_string(calls)?.trim(), "sync-home-server");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn timed_node_process_is_terminated() -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt as _;

        let temporary = tempfile::tempdir()?;
        let executable = temporary.path().join("slow-node");
        fs::write(&executable, "#!/bin/sh\nexec sleep 5\n")?;
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700))?;
        let paths = Paths {
            state: temporary.path().join("state"),
            config: temporary.path().join("client.json"),
            feedback: temporary.path().join("feedback.json"),
            node: executable,
        };

        let error = run_node_with_timeout(&paths, &[], Duration::from_millis(30))
            .expect_err("the child should exceed its deadline");

        assert_eq!(error.code, "nodeTimeout");
        Ok(())
    }
}
