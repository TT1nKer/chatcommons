import tempfile
import unittest
from pathlib import Path

from tools.latency_report import build_report, load_trace


def record(phase, unix_ms, *, marker="", elapsed_ms=None):
    return {
        "version": 1,
        "phase": phase,
        "eventMarker": marker,
        "unixMs": unix_ms,
        "elapsedMs": elapsed_ms,
    }


class LatencyReportTests(unittest.TestCase):
    def test_matches_delivery_by_event_marker_across_devices(self):
        traces = {
            "mac": [record("event_persisted", 1_000, marker="event-a")],
            "windows": [record("event_observed", 1_245, marker="event-a")],
        }

        report = build_report(traces)

        self.assertIn("event-a", report)
        self.assertIn("mac -> windows", report)
        self.assertIn("245 ms", report)

    def test_does_not_claim_delivery_when_only_one_device_saw_an_event(self):
        traces = {
            "mac": [
                record("event_persisted", 1_000, marker="event-a"),
                record("event_observed", 1_050, marker="event-a"),
            ],
        }

        report = build_report(traces)

        self.assertIn("no cross-device match", report)
        self.assertNotIn("mac -> mac", report)

    def test_groups_voice_stages_by_join_attempt(self):
        traces = {
            "windows": [
                record("voice_join_started", 2_000, elapsed_ms=0),
                record("voice_microphone_ready", 2_100, elapsed_ms=100),
                record("voice_grant_ready", 2_130, elapsed_ms=130),
                record("voice_sfu_connected", 2_380, elapsed_ms=380),
            ],
        }

        report = build_report(traces)

        self.assertIn("windows voice attempt 1", report)
        self.assertIn("microphone_ready=100 ms", report)
        self.assertIn("sfu_connected=380 ms", report)

    def test_rejects_malformed_or_oversized_input(self):
        with tempfile.TemporaryDirectory() as directory:
            malformed = Path(directory) / "malformed.jsonl"
            malformed.write_text('{"phase": 7}\n', encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "invalid trace record"):
                load_trace(malformed)

            oversized = Path(directory) / "oversized.jsonl"
            oversized.write_bytes(b"x" * (1024 * 1024 + 1))
            with self.assertRaisesRegex(ValueError, "exceeds 1 MiB"):
                load_trace(oversized)

            unsafe_marker = Path(directory) / "unsafe-marker.jsonl"
            unsafe_marker.write_text(
                '{"version":1,"phase":"event_observed",'
                '"eventMarker":"bad marker!","unixMs":true,"elapsedMs":null}\n',
                encoding="utf-8",
            )
            with self.assertRaisesRegex(ValueError, "invalid trace record"):
                load_trace(unsafe_marker)

            extra_field = Path(directory) / "extra-field.jsonl"
            extra_field.write_text(
                '{"version":1,"phase":"event_observed",'
                '"eventMarker":"event-a","unixMs":1,"elapsedMs":null,'
                '"participantToken":"must-not-be-accepted"}\n',
                encoding="utf-8",
            )
            with self.assertRaisesRegex(ValueError, "invalid trace record"):
                load_trace(extra_field)

            with self.assertRaisesRegex(ValueError, "not a regular file"):
                load_trace(Path(directory))


if __name__ == "__main__":
    unittest.main()
