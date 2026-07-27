use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chatcommons_crypto::UserId;
use chatcommons_protocol::CommunityId;
use chatcommons_sync::{auth::DeviceId, network::VoiceGrant};
use livekit_api::access_token::{AccessToken, AccessTokenError, VideoGrants};
use std::{env, time::Duration};
use thiserror::Error;

const TOKEN_TTL: Duration = Duration::from_secs(5 * 60);
const MAX_CONFIG_VALUE_BYTES: usize = 2 * 1024;

#[derive(Debug, Error)]
pub enum VoiceIssuerError {
    #[error("token generation failed: {0}")]
    Token(#[from] AccessTokenError),
    #[error("configuration is invalid: {0}")]
    Config(&'static str),
    #[error("token expiry exceeds the supported timestamp range")]
    InvalidExpiry,
}

pub struct VoiceTokenIssuer {
    server_url: String,
    api_key: String,
    api_secret: String,
}

impl VoiceTokenIssuer {
    pub fn from_environment() -> Result<Option<Self>, VoiceIssuerError> {
        let read = |name| match env::var(name) {
            Ok(value) => Ok(Some(value)),
            Err(env::VarError::NotPresent) => Ok(None),
            Err(env::VarError::NotUnicode(_)) => {
                Err(VoiceIssuerError::Config("environment value is not UTF-8"))
            }
        };
        let server_url = read("CHATCOMMONS_VOICE_SERVER_URL")?;
        let api_key = read("CHATCOMMONS_LIVEKIT_API_KEY")?;
        let api_secret = read("CHATCOMMONS_LIVEKIT_API_SECRET")?;
        match (server_url, api_key, api_secret) {
            (None, None, None) => Ok(None),
            (Some(server_url), Some(api_key), Some(api_secret)) => {
                let valid_url = server_url.starts_with("wss://")
                    && server_url.len() <= MAX_CONFIG_VALUE_BYTES
                    && !server_url.chars().any(char::is_whitespace);
                let valid_key = (4..=MAX_CONFIG_VALUE_BYTES).contains(&api_key.len())
                    && !api_key.chars().any(char::is_whitespace);
                let valid_secret = (32..=MAX_CONFIG_VALUE_BYTES).contains(&api_secret.len())
                    && !api_secret.chars().any(char::is_whitespace);
                if !valid_url || !valid_key || !valid_secret {
                    return Err(VoiceIssuerError::Config(
                        "URL, API key, or API secret is malformed",
                    ));
                }
                Ok(Some(Self {
                    server_url,
                    api_key,
                    api_secret,
                }))
            }
            _ => Err(VoiceIssuerError::Config(
                "all three voice environment variables must be provided together",
            )),
        }
    }

    pub fn issue(
        &self,
        community: CommunityId,
        channel_id: [u8; 32],
        user_id: UserId,
        device_id: DeviceId,
        issued_at_ms: i64,
    ) -> Result<VoiceGrant, VoiceIssuerError> {
        let room = format!(
            "cc-{}-{}",
            hex::encode(community.as_bytes()),
            hex::encode(channel_id)
        );
        let identity = format!(
            "{}:{}",
            URL_SAFE_NO_PAD.encode(user_id.as_bytes()),
            URL_SAFE_NO_PAD.encode(device_id.as_bytes())
        );
        let display_name = format!("member-{}", &hex::encode(user_id.as_bytes())[..10]);
        let participant_token = AccessToken::with_api_key(&self.api_key, &self.api_secret)
            .with_ttl(TOKEN_TTL)
            .with_identity(&identity)
            .with_name(&display_name)
            .with_grants(VideoGrants {
                room_join: true,
                room,
                can_publish: true,
                can_subscribe: true,
                can_publish_data: false,
                can_publish_sources: vec!["microphone".into()],
                ..Default::default()
            })
            .to_jwt()?;
        let expires_at_ms = issued_at_ms
            .checked_add(TOKEN_TTL.as_millis() as i64)
            .ok_or(VoiceIssuerError::InvalidExpiry)?;
        Ok(VoiceGrant {
            server_url: self.server_url.clone(),
            participant_token,
            expires_at_ms,
        })
    }
}
