#!/usr/bin/env python3
"""Summarize privacy-bounded ChatCommons latency traces from multiple devices."""

import argparse
import json
import os
import stat
import sys
from pathlib import Path


MAX_TRACE_BYTES = 1024 * 1024
TRACE_FIELDS = {"version", "phase", "eventMarker", "unixMs", "elapsedMs"}
VOICE_PHASES = {
    "voice_join_started",
    "voice_microphone_ready",
    "voice_grant_ready",
    "voice_sfu_connected",
    "voice_audio_started",
    "voice_remote_track",
}
SYNC_PHASES = {
    "sync_queued",
    "sync_lock_acquired",
    "sync_started",
    "sync_succeeded",
    "sync_failed",
    "snapshot_ready",
}
TEXT_PHASES = {"event_persisted", "event_observed"}
ALLOWED_PHASES = VOICE_PHASES | SYNC_PHASES | TEXT_PHASES | {
    "send_queued",
    "send_lock_acquired",
    "send_response_ready",
}


def load_trace(path: Path) -> list[dict]:
    display_path = repr(str(path))
    if not stat.S_ISREG(path.stat().st_mode):
        raise ValueError(f"{display_path}: trace is not a regular file")
    flags = os.O_RDONLY | getattr(os, "O_NONBLOCK", 0) | getattr(os, "O_BINARY", 0)
    with os.fdopen(os.open(path, flags), "rb") as trace_file:
        if not stat.S_ISREG(os.fstat(trace_file.fileno()).st_mode):
            raise ValueError(f"{display_path}: trace is not a regular file")
        contents = trace_file.read(MAX_TRACE_BYTES + 1)
    if len(contents) > MAX_TRACE_BYTES:
        raise ValueError(f"{display_path}: trace exceeds 1 MiB")
    try:
        lines = contents.decode("utf-8").splitlines()
    except UnicodeDecodeError as error:
        raise ValueError(f"{display_path}: trace is not UTF-8") from error
    records = []
    for line_number, line in enumerate(lines, start=1):
        try:
            record = json.loads(line)
        except json.JSONDecodeError as error:
            raise ValueError(f"{display_path}:{line_number}: invalid JSON") from error
        if not _valid_record(record):
            raise ValueError(f"{display_path}:{line_number}: invalid trace record")
        records.append(record)
    return records


def _valid_record(record: object) -> bool:
    if not isinstance(record, dict):
        return False
    elapsed_ms = record.get("elapsedMs")
    event_marker = record.get("eventMarker")
    return (
        set(record) == TRACE_FIELDS
        and type(record.get("version")) is int
        and record["version"] == 1
        and record.get("phase") in ALLOWED_PHASES
        and isinstance(event_marker, str)
        and len(event_marker) <= 16
        and all(
            character.isascii() and (character.isalnum() or character in "-_")
            for character in event_marker
        )
        and type(record.get("unixMs")) is int
        and record["unixMs"] >= 0
        and (elapsed_ms is None or (type(elapsed_ms) is int and elapsed_ms >= 0))
    )


def build_report(traces: dict[str, list[dict]]) -> str:
    lines = [
        "ChatCommons latency report",
        "Cross-device delivery uses wall clocks; synchronize both device clocks first.",
        "",
        "Text delivery",
    ]
    deliveries = _text_deliveries(traces)
    if deliveries:
        lines.extend(deliveries)
    else:
        lines.append("- no cross-device match")

    lines.extend(["", "Synchronization"])
    sync_attempts = _attempt_summaries(traces, "sync_queued", SYNC_PHASES, "sync")
    lines.extend(sync_attempts or ["- no sync attempts"])

    lines.extend(["", "Voice"])
    voice_attempts = _attempt_summaries(
        traces, "voice_join_started", VOICE_PHASES, "voice attempt"
    )
    lines.extend(voice_attempts or ["- no voice attempts"])
    return "\n".join(lines)


def _text_deliveries(traces: dict[str, list[dict]]) -> list[str]:
    persisted = {}
    observed = {}
    for source, records in traces.items():
        for record in records:
            marker = record["eventMarker"]
            if not marker:
                continue
            target = persisted if record["phase"] == "event_persisted" else observed
            if record["phase"] in TEXT_PHASES:
                target.setdefault(marker, []).append((source, record["unixMs"]))

    lines = []
    for marker in sorted(persisted):
        for sender, sent_at in persisted[marker]:
            receivers = [entry for entry in observed.get(marker, []) if entry[0] != sender]
            if not receivers:
                continue
            receiver, received_at = min(receivers, key=lambda entry: entry[1])
            lines.append(
                f"- {marker}: {sender} -> {receiver}: {received_at - sent_at} ms"
            )
    return lines


def _attempt_summaries(
    traces: dict[str, list[dict]], start_phase: str, phases: set[str], label: str
) -> list[str]:
    lines = []
    for source, records in traces.items():
        attempts = []
        current = None
        for record in records:
            if record["phase"] == start_phase:
                current = []
                attempts.append(current)
            if current is not None and record["phase"] in phases:
                current.append(record)
        for number, attempt in enumerate(attempts, start=1):
            stages = []
            for record in attempt:
                elapsed_ms = record["elapsedMs"]
                if elapsed_ms is None or record["phase"] == start_phase:
                    continue
                stage = record["phase"].removeprefix("voice_").removeprefix("sync_")
                stages.append(f"{stage}={elapsed_ms} ms")
            summary = ", ".join(stages) if stages else "no completed stages"
            lines.append(f"- {source} {label} {number}: {summary}")
    return lines


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Compare ChatCommons latency-trace.jsonl files."
    )
    parser.add_argument("traces", nargs="+", type=Path)
    args = parser.parse_args()
    loaded = {}
    try:
        for index, path in enumerate(args.traces, start=1):
            loaded[f"device-{index}"] = load_trace(path)
    except (OSError, ValueError) as error:
        print(error, file=sys.stderr)
        return 2
    print(build_report(loaded))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
