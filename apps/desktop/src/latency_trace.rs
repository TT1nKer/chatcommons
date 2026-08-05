use std::{
    collections::HashSet,
    fs::OpenOptions,
    io::Write,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

const EVENT_MARKER_LENGTH: usize = 16;
const TRACE_FILE_LIMIT_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, Copy)]
pub(crate) enum LatencyPhase {
    SendQueued,
    SendLockAcquired,
    SyncStarted,
    SyncQueued,
    SyncLockAcquired,
    SyncSucceeded,
    SyncFailed,
    EventPersisted,
    SendResponseReady,
    SnapshotReady,
    EventObserved,
    VoiceJoinStarted,
    VoiceMicrophoneReady,
    VoiceGrantReady,
    VoiceSfuConnected,
    VoiceAudioStarted,
    VoiceRemoteTrack,
}

impl LatencyPhase {
    pub(crate) fn from_client_name(name: &str) -> Option<Self> {
        match name {
            "voice_join_started" => Some(Self::VoiceJoinStarted),
            "voice_microphone_ready" => Some(Self::VoiceMicrophoneReady),
            "voice_grant_ready" => Some(Self::VoiceGrantReady),
            "voice_sfu_connected" => Some(Self::VoiceSfuConnected),
            "voice_audio_started" => Some(Self::VoiceAudioStarted),
            "voice_remote_track" => Some(Self::VoiceRemoteTrack),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::SendQueued => "send_queued",
            Self::SendLockAcquired => "send_lock_acquired",
            Self::SyncStarted => "sync_started",
            Self::SyncQueued => "sync_queued",
            Self::SyncLockAcquired => "sync_lock_acquired",
            Self::SyncSucceeded => "sync_succeeded",
            Self::SyncFailed => "sync_failed",
            Self::EventPersisted => "event_persisted",
            Self::SendResponseReady => "send_response_ready",
            Self::SnapshotReady => "snapshot_ready",
            Self::EventObserved => "event_observed",
            Self::VoiceJoinStarted => "voice_join_started",
            Self::VoiceMicrophoneReady => "voice_microphone_ready",
            Self::VoiceGrantReady => "voice_grant_ready",
            Self::VoiceSfuConnected => "voice_sfu_connected",
            Self::VoiceAudioStarted => "voice_audio_started",
            Self::VoiceRemoteTrack => "voice_remote_track",
        }
    }
}

#[derive(Debug, Default)]
struct TraceState {
    observed_events: HashSet<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct LatencyTracer {
    enabled: bool,
    path: PathBuf,
    state: Arc<Mutex<TraceState>>,
}

impl LatencyTracer {
    pub(crate) fn new(enabled: bool, path: PathBuf) -> Self {
        Self {
            enabled,
            path,
            state: Arc::new(Mutex::new(TraceState::default())),
        }
    }

    pub(crate) fn mark(
        &self,
        phase: LatencyPhase,
        event_id: Option<&str>,
        elapsed_ms: Option<u64>,
    ) {
        if !self.enabled {
            return;
        }
        let unix_ms = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => duration.as_millis(),
            Err(error) => {
                eprintln!("latency trace clock unavailable: {error}");
                return;
            }
        };
        let event_marker = event_id.map(event_marker).unwrap_or_default();
        let elapsed = elapsed_ms
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".to_owned());
        let line = format!(
            "{{\"version\":1,\"phase\":\"{}\",\"eventMarker\":\"{}\",\"unixMs\":{},\"elapsedMs\":{}}}\n",
            phase.as_str(),
            event_marker,
            unix_ms,
            elapsed
        );
        let Ok(_guard) = self.state.lock() else {
            eprintln!("latency trace state is unavailable");
            return;
        };
        let should_rotate = self
            .path
            .metadata()
            .map(|metadata| {
                metadata.len().saturating_add(line.len() as u64) > TRACE_FILE_LIMIT_BYTES
            })
            .unwrap_or(false);
        let result = if should_rotate {
            OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&self.path)
        } else {
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)
        }
        .and_then(|mut file| file.write_all(line.as_bytes()));
        if let Err(error) = result {
            eprintln!("latency trace write failed: {error}");
        }
    }

    pub(crate) fn baseline_events<'a>(&self, event_ids: impl IntoIterator<Item = &'a str>) {
        if !self.enabled {
            return;
        }
        let Ok(mut state) = self.state.lock() else {
            eprintln!("latency trace state is unavailable");
            return;
        };
        state
            .observed_events
            .extend(event_ids.into_iter().map(str::to_owned));
    }

    pub(crate) fn observe_events<'a>(&self, event_ids: impl IntoIterator<Item = &'a str>) {
        if !self.enabled {
            return;
        }
        let new_events = {
            let Ok(mut state) = self.state.lock() else {
                eprintln!("latency trace state is unavailable");
                return;
            };
            event_ids
                .into_iter()
                .filter(|event_id| state.observed_events.insert((*event_id).to_owned()))
                .map(str::to_owned)
                .collect::<Vec<_>>()
        };
        for event_id in new_events {
            self.mark(LatencyPhase::EventObserved, Some(&event_id), None);
        }
    }
}

fn event_marker(event_id: &str) -> String {
    event_id
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
        .take(EVENT_MARKER_LENGTH)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{LatencyPhase, LatencyTracer};
    use std::{
        fs,
        path::PathBuf,
        process, thread,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    fn temporary_trace_path(test_name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after the Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "chatcommons-{test_name}-{}-{unique}.jsonl",
            process::id()
        ))
    }

    fn trace_lines(path: &std::path::Path) -> Vec<String> {
        fs::read_to_string(path)
            .expect("latency trace should be readable")
            .lines()
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn disabled_tracer_does_not_create_a_file() {
        let path = temporary_trace_path("disabled");
        let tracer = LatencyTracer::new(false, path.clone());

        tracer.mark(LatencyPhase::SyncStarted, None, None);

        assert!(!path.exists());
    }

    #[test]
    fn event_marker_is_stable_bounded_and_contains_no_full_event_id() {
        let path = temporary_trace_path("marker");
        let tracer = LatencyTracer::new(true, path.clone());
        let event_id = "0123456789abcdef0123456789abcdef-secret-tail";

        tracer.mark(LatencyPhase::EventPersisted, Some(event_id), Some(17));

        let lines = trace_lines(&path);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].starts_with('{') && lines[0].ends_with('}'));
        assert!(lines[0].contains("\"version\":1"));
        assert!(lines[0].contains("\"phase\":\"event_persisted\""));
        assert!(lines[0].contains("\"eventMarker\":\"0123456789abcdef\""));
        assert!(lines[0].contains("\"elapsedMs\":17"));
        assert!(
            !fs::read_to_string(path)
                .expect("trace contents")
                .contains(event_id)
        );
    }

    #[test]
    fn baseline_suppresses_history_and_observation_records_each_new_event_once() {
        let path = temporary_trace_path("observation");
        let tracer = LatencyTracer::new(true, path.clone());
        tracer.baseline_events(["history-event"]);

        tracer.observe_events(["history-event", "new-event"]);
        thread::sleep(Duration::from_millis(2));
        tracer.observe_events(["new-event"]);

        let lines = trace_lines(&path);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("\"phase\":\"event_observed\""));
        assert!(lines[0].contains("\"eventMarker\":\"new-event\""));
        assert!(lines[0].contains("\"unixMs\":"));
    }

    #[test]
    fn trace_file_is_rotated_before_it_can_grow_without_bound() {
        let path = temporary_trace_path("bounded");
        let tracer = LatencyTracer::new(true, path.clone());

        for _ in 0..20_000 {
            tracer.mark(LatencyPhase::SyncStarted, None, None);
        }

        let size = fs::metadata(path).expect("trace metadata").len();
        assert!(size <= 1024 * 1024, "trace grew to {size} bytes");
    }

    #[test]
    fn client_marks_accept_only_the_bounded_voice_phase_allowlist() {
        assert!(matches!(
            LatencyPhase::from_client_name("voice_sfu_connected"),
            Some(LatencyPhase::VoiceSfuConnected)
        ));
        assert!(LatencyPhase::from_client_name("event_persisted").is_none());
        assert!(LatencyPhase::from_client_name("message body").is_none());
    }
}
