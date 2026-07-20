#!/usr/bin/env python3
"""Isolated feedback API for the ChatCommons browser prototype."""

from __future__ import annotations

import base64
import binascii
import hashlib
import hmac
import ipaddress
import json
import mimetypes
import os
import re
import secrets
import signal
import sqlite3
import threading
import time
from collections import defaultdict, deque
from contextlib import contextmanager
from datetime import UTC, datetime
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlsplit

MAX_BODY_BYTES = 2 * 1024 * 1024
MAX_SCREENSHOT_ENCODED = 1_500_000
MAX_SCREENSHOT_BYTES = 1_000_000
CATEGORIES = {"layout", "copy", "feature", "product"}
PRIORITIES = {"low", "normal", "high"}
STATUSES = {"pending", "in_progress", "client_review", "completed", "rejected", "withdrawn"}
PUBLIC_ID_PATTERN = re.compile(r"RV-[A-Za-z0-9_-]{12,32}")


def utc_now() -> str:
    return datetime.now(UTC).isoformat(timespec="seconds")


class Config:
    def __init__(self) -> None:
        self.host = os.environ.get("REVIEW_HOST", "127.0.0.1")
        self.port = int(os.environ.get("REVIEW_PORT", "8091"))
        self.database = Path(os.environ.get("REVIEW_DB_PATH", "./data/reviews.sqlite3"))
        self.screenshots = Path(os.environ.get("REVIEW_SCREENSHOT_DIR", "./data/screenshots"))
        static_root = os.environ.get("REVIEW_STATIC_ROOT", "")
        self.static_root = Path(static_root).resolve() if static_root else None
        self.review_token = os.environ.get("REVIEW_TOKEN", "")
        self.owner_token = os.environ.get("OWNER_TOKEN", "")
        self.allowed_origin = os.environ.get("REVIEW_ALLOWED_ORIGIN", "")

    def validate(self) -> None:
        if len(self.review_token) < 40 or len(self.owner_token) < 40:
            raise RuntimeError("REVIEW_TOKEN and OWNER_TOKEN must each contain at least 40 characters")
        if hmac.compare_digest(self.review_token, self.owner_token):
            raise RuntimeError("reviewer and owner credentials must be independent")
        origin = urlsplit(self.allowed_origin)
        if (
            origin.scheme not in {"http", "https"}
            or not origin.netloc
            or origin.username is not None
            or origin.password is not None
            or origin.path not in {"", "/"}
            or origin.query
            or origin.fragment
        ):
            raise RuntimeError("REVIEW_ALLOWED_ORIGIN must be an absolute HTTP(S) origin")


class RateLimiter:
    def __init__(self) -> None:
        self._events: dict[str, deque[float]] = defaultdict(deque)
        self._lock = threading.Lock()

    def limited(self, key: str, maximum: int, period: int) -> bool:
        now = time.monotonic()
        with self._lock:
            events = self._events[key]
            while events and events[0] <= now - period:
                events.popleft()
            if len(events) >= maximum:
                return True
            events.append(now)
            return False


class ReviewApplication:
    def __init__(self, config: Config) -> None:
        self.config = config
        self.limiter = RateLimiter()
        self.config.database.parent.mkdir(parents=True, exist_ok=True, mode=0o750)
        self.config.screenshots.mkdir(parents=True, exist_ok=True, mode=0o750)
        self._initialize_database()

    def connect(self) -> sqlite3.Connection:
        connection = sqlite3.connect(self.config.database, timeout=10)
        connection.row_factory = sqlite3.Row
        connection.execute("PRAGMA foreign_keys = ON")
        connection.execute("PRAGMA busy_timeout = 10000")
        return connection

    @contextmanager
    def database(self):
        connection = self.connect()
        try:
            with connection:
                yield connection
        finally:
            connection.close()

    def _initialize_database(self) -> None:
        with self.database() as connection:
            connection.execute("PRAGMA journal_mode = WAL")
            connection.execute(
                """
                CREATE TABLE IF NOT EXISTS reviews (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    public_id TEXT NOT NULL UNIQUE,
                    surface TEXT NOT NULL,
                    screen TEXT NOT NULL,
                    target_id TEXT NOT NULL,
                    target_text TEXT NOT NULL,
                    x REAL NOT NULL,
                    y REAL NOT NULL,
                    scroll_x REAL NOT NULL DEFAULT 0,
                    scroll_y REAL NOT NULL DEFAULT 0,
                    viewport_width INTEGER NOT NULL,
                    viewport_height INTEGER NOT NULL,
                    category TEXT NOT NULL,
                    priority TEXT NOT NULL,
                    message TEXT NOT NULL,
                    screenshot_file TEXT NOT NULL DEFAULT '',
                    status TEXT NOT NULL DEFAULT 'pending',
                    admin_reply TEXT NOT NULL DEFAULT '',
                    created_ip TEXT NOT NULL,
                    user_agent TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                )
                """
            )
            columns = {row["name"] for row in connection.execute("PRAGMA table_info(reviews)")}
            if "edit_token_hash" not in columns:
                connection.execute("ALTER TABLE reviews ADD COLUMN edit_token_hash TEXT NOT NULL DEFAULT ''")
            if "scroll_x" not in columns:
                connection.execute("ALTER TABLE reviews ADD COLUMN scroll_x REAL NOT NULL DEFAULT 0")
            if "scroll_y" not in columns:
                connection.execute("ALTER TABLE reviews ADD COLUMN scroll_y REAL NOT NULL DEFAULT 0")
            connection.execute(
                """
                CREATE TABLE IF NOT EXISTS audit_log (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    review_id INTEGER NOT NULL REFERENCES reviews(id) ON DELETE CASCADE,
                    action TEXT NOT NULL,
                    detail TEXT NOT NULL,
                    created_at TEXT NOT NULL
                )
                """
            )

    @staticmethod
    def clean_text(value: object, maximum: int) -> str:
        if not isinstance(value, str):
            return ""
        return " ".join(value.strip().split())[:maximum]

    @staticmethod
    def edit_token_hash(token: str) -> str:
        return hashlib.sha256(token.encode("utf-8")).hexdigest()

    @classmethod
    def valid_edit_token(cls, supplied: str, expected_hash: str) -> bool:
        if not (40 <= len(supplied) <= 200) or len(expected_hash) != 64:
            return False
        return hmac.compare_digest(cls.edit_token_hash(supplied), expected_hash)

    def save_screenshot(self, value: object) -> str:
        if value in (None, ""):
            return ""
        if not isinstance(value, str):
            raise ValueError("截图格式无效")
        formats = {
            "data:image/jpeg;base64,": (".jpg", lambda data: data.startswith(b"\xff\xd8\xff")),
            "data:image/png;base64,": (".png", lambda data: data.startswith(b"\x89PNG\r\n\x1a\n")),
            "data:image/webp;base64,": (".webp", lambda data: len(data) >= 12 and data[:4] == b"RIFF" and data[8:12] == b"WEBP"),
        }
        match = next(((prefix, details) for prefix, details in formats.items() if value.startswith(prefix)), None)
        if match is None:
            raise ValueError("截图仅支持 JPG、PNG 或 WebP")
        prefix, (extension, valid_magic) = match
        encoded = value[len(prefix) :]
        if not encoded or len(encoded) > MAX_SCREENSHOT_ENCODED:
            raise ValueError("截图过大或为空")
        try:
            content = base64.b64decode(encoded, validate=True)
        except (binascii.Error, ValueError) as error:
            raise ValueError("截图数据无效") from error
        if not content or len(content) > MAX_SCREENSHOT_BYTES or not valid_magic(content):
            raise ValueError("截图内容无效或超过 1MB")
        filename = f"{secrets.token_hex(24)}{extension}"
        path = self.config.screenshots / filename
        with path.open("xb") as output:
            os.chmod(path, 0o640)
            output.write(content)
        return filename

    def create_review(self, payload: object, client_ip: str, user_agent: str) -> dict[str, object]:
        if not isinstance(payload, dict):
            raise ValueError("请求内容无效")
        allowed = {
            "surface", "screen", "targetId", "targetText", "x", "y", "viewportWidth",
            "viewportHeight", "scrollX", "scrollY", "category", "priority", "message", "screenshot",
        }
        if set(payload) - allowed:
            raise ValueError("请求包含未知字段")
        surface = self.clean_text(payload.get("surface"), 30)
        screen = self.clean_text(payload.get("screen"), 180)
        target_id = self.clean_text(payload.get("targetId"), 500)
        target_text = self.clean_text(payload.get("targetText"), 300)
        message = self.clean_text(payload.get("message"), 1000)
        category = payload.get("category")
        priority = payload.get("priority")
        try:
            x, y = float(payload.get("x")), float(payload.get("y"))
            scroll_x = float(payload.get("scrollX", 0))
            scroll_y = float(payload.get("scrollY", 0))
            width, height = int(payload.get("viewportWidth")), int(payload.get("viewportHeight"))
        except (TypeError, ValueError) as error:
            raise ValueError("标注位置无效") from error
        if surface != "prototype" or not screen or len(message) < 2:
            raise ValueError("请完整填写意见")
        if category not in CATEGORIES or priority not in PRIORITIES:
            raise ValueError("意见类型或优先级无效")
        if not (
            0 <= x <= 1
            and 0 <= y <= 1
            and 0 <= scroll_x <= 10_000_000
            and 0 <= scroll_y <= 10_000_000
            and 280 <= width <= 10_000
            and 300 <= height <= 10_000
        ):
            raise ValueError("标注位置或窗口尺寸无效")
        screenshot_file = self.save_screenshot(payload.get("screenshot"))
        now = utc_now()
        public_id = f"RV-{secrets.token_urlsafe(12)}"
        edit_token = secrets.token_urlsafe(32)
        try:
            with self.database() as connection:
                cursor = connection.execute(
                    """
                    INSERT INTO reviews (
                        public_id,surface,screen,target_id,target_text,x,y,scroll_x,scroll_y,viewport_width,
                        viewport_height,category,priority,message,screenshot_file,created_ip,
                        user_agent,created_at,updated_at,edit_token_hash
                    ) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)
                    """,
                    (
                        public_id, surface, screen, target_id, target_text, x, y, scroll_x, scroll_y, width, height,
                        category, priority, message, screenshot_file, client_ip,
                        self.clean_text(user_agent, 300), now, now, self.edit_token_hash(edit_token),
                    ),
                )
                if cursor.lastrowid is None:
                    raise RuntimeError("意见编号生成失败")
        except Exception:
            if screenshot_file:
                (self.config.screenshots / screenshot_file).unlink(missing_ok=True)
            raise
        return {"publicId": public_id, "editToken": edit_token, "status": "pending", "createdAt": now}

    def list_reviews(self, owner: bool) -> list[dict[str, object]]:
        with self.database() as connection:
            query = "SELECT * FROM reviews ORDER BY id DESC LIMIT 300" if owner else (
                "SELECT * FROM reviews WHERE status <> 'withdrawn' ORDER BY id DESC LIMIT 300"
            )
            rows = connection.execute(query).fetchall()
        items = []
        for row in rows:
            item = {
                "publicId": row["public_id"], "surface": row["surface"], "screen": row["screen"],
                "targetText": row["target_text"], "x": row["x"], "y": row["y"],
                "scrollX": row["scroll_x"], "scrollY": row["scroll_y"],
                "viewportWidth": row["viewport_width"], "viewportHeight": row["viewport_height"],
                "category": row["category"], "priority": row["priority"], "message": row["message"],
                "status": row["status"], "adminReply": row["admin_reply"],
                "createdAt": row["created_at"], "updatedAt": row["updated_at"],
            }
            if owner:
                item.update({"id": row["id"], "targetId": row["target_id"], "hasScreenshot": bool(row["screenshot_file"])})
            items.append(item)
        return items

    def edit_review(self, public_id: str, payload: object, edit_token: str) -> str:
        if not isinstance(payload, dict) or set(payload) - {"category", "priority", "message"}:
            raise ValueError("请求内容无效")
        category = payload.get("category")
        priority = payload.get("priority")
        message = self.clean_text(payload.get("message"), 1000)
        if category not in CATEGORIES or priority not in PRIORITIES or len(message) < 2:
            raise ValueError("请完整填写意见")
        now = utc_now()
        with self.database() as connection:
            row = connection.execute(
                "SELECT id,status,edit_token_hash FROM reviews WHERE public_id=?", (public_id,)
            ).fetchone()
            if row is None:
                return "not_found"
            if not self.valid_edit_token(edit_token, row["edit_token_hash"]):
                return "forbidden"
            if row["status"] != "pending":
                return "conflict"
            connection.execute(
                "UPDATE reviews SET category=?,priority=?,message=?,updated_at=? WHERE id=?",
                (category, priority, message, now, row["id"]),
            )
            connection.execute(
                "INSERT INTO audit_log(review_id,action,detail,created_at) VALUES(?,?,?,?)",
                (
                    row["id"],
                    "review.reviewer_edit",
                    json.dumps({"category": category, "priority": priority, "message": message}, ensure_ascii=False),
                    now,
                ),
            )
        return "ok"

    def withdraw_review(self, public_id: str, edit_token: str) -> str:
        now = utc_now()
        with self.database() as connection:
            row = connection.execute(
                "SELECT id,status,edit_token_hash FROM reviews WHERE public_id=?", (public_id,)
            ).fetchone()
            if row is None:
                return "not_found"
            if not self.valid_edit_token(edit_token, row["edit_token_hash"]):
                return "forbidden"
            if row["status"] in {"completed", "rejected", "withdrawn"}:
                return "conflict"
            connection.execute(
                "UPDATE reviews SET status='withdrawn',updated_at=? WHERE id=?", (now, row["id"])
            )
            connection.execute(
                "INSERT INTO audit_log(review_id,action,detail,created_at) VALUES(?,?,?,?)",
                (
                    row["id"],
                    "review.withdraw",
                    json.dumps({"previousStatus": row["status"]}, ensure_ascii=False),
                    now,
                ),
            )
        return "ok"

    def update_review(self, review_id: int, payload: object) -> bool:
        if not isinstance(payload, dict) or set(payload) - {"status", "adminReply"}:
            raise ValueError("请求内容无效")
        status = payload.get("status")
        reply = self.clean_text(payload.get("adminReply"), 1000)
        if status not in STATUSES:
            raise ValueError("处理状态无效")
        now = utc_now()
        with self.database() as connection:
            result = connection.execute(
                "UPDATE reviews SET status=?,admin_reply=?,updated_at=? WHERE id=?",
                (status, reply, now, review_id),
            )
            if result.rowcount == 0:
                return False
            connection.execute(
                "INSERT INTO audit_log(review_id,action,detail,created_at) VALUES(?,?,?,?)",
                (review_id, "review.update", json.dumps({"status": status, "reply": reply}, ensure_ascii=False), now),
            )
        return True

    def screenshot_path(self, review_id: int) -> Path | None:
        with self.database() as connection:
            row = connection.execute("SELECT screenshot_file FROM reviews WHERE id=?", (review_id,)).fetchone()
        if row is None or not row["screenshot_file"] or Path(row["screenshot_file"]).name != row["screenshot_file"]:
            return None
        path = self.config.screenshots / row["screenshot_file"]
        return path if path.is_file() else None


def make_handler(application: ReviewApplication):
    class Handler(BaseHTTPRequestHandler):
        server_version = "ChatCommonsReview/1"
        sys_version = ""

        def log_message(self, format_string: str, *args: object) -> None:
            # Do not log request URLs because reviewer credentials arrive in the first page URL.
            return

        def end_headers(self) -> None:
            self.send_header("X-Content-Type-Options", "nosniff")
            self.send_header("Cache-Control", "no-store")
            self.send_header("Referrer-Policy", "no-referrer")
            super().end_headers()

        def json_response(self, status: HTTPStatus, payload: dict[str, object]) -> None:
            body = json.dumps(payload, ensure_ascii=False, separators=(",", ":")).encode()
            self.send_response(status)
            self.send_header("Content-Type", "application/json; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def error_response(self, status: HTTPStatus, message: str) -> None:
            self.json_response(status, {"error": message})

        def client_ip(self) -> str:
            candidate = self.headers.get("X-Real-IP", "") if self.client_address[0] in {"127.0.0.1", "::1"} else self.client_address[0]
            try:
                return str(ipaddress.ip_address(candidate or self.client_address[0]))
            except ValueError:
                return self.client_address[0]

        def authorized(self, owner: bool = False) -> bool:
            supplied = self.headers.get("X-Owner-Token" if owner else "X-Review-Token", "").strip()
            expected = application.config.owner_token if owner else application.config.review_token
            return len(supplied) >= 40 and hmac.compare_digest(supplied, expected)

        def require_authorized(self, owner: bool = False) -> bool:
            if self.authorized(owner):
                return True
            self.error_response(HTTPStatus.UNAUTHORIZED, "评审链接无效或已停用" if not owner else "管理员链接无效或已停用")
            return False

        def require_origin(self) -> bool:
            if self.headers.get("Origin", "") == application.config.allowed_origin:
                return True
            self.error_response(HTTPStatus.FORBIDDEN, "请求来源无效")
            return False

        def read_json(self) -> object:
            try:
                length = int(self.headers.get("Content-Length", "0"))
            except ValueError as error:
                raise ValueError("请求长度无效") from error
            if length <= 0 or length > MAX_BODY_BYTES:
                raise ValueError("请求为空或超过 2MB")
            if self.headers.get_content_type() != "application/json":
                raise ValueError("请求必须使用 JSON")
            try:
                return json.loads(self.rfile.read(length))
            except (json.JSONDecodeError, UnicodeDecodeError) as error:
                raise ValueError("JSON 内容无效") from error

        def do_GET(self) -> None:  # noqa: N802
            path = urlsplit(self.path).path.rstrip("/") or "/"
            if path == "/api/health":
                self.json_response(HTTPStatus.OK, {"ok": True})
                return
            if path == "/api/reviews":
                if self.require_authorized():
                    self.json_response(HTTPStatus.OK, {"reviews": application.list_reviews(False)})
                return
            if path == "/api/admin/reviews":
                if self.require_authorized(True):
                    self.json_response(HTTPStatus.OK, {"reviews": application.list_reviews(True)})
                return
            image_prefix = "/api/admin/reviews/"
            if path.startswith(image_prefix) and path.endswith("/image"):
                if not self.require_authorized(True):
                    return
                raw_id = path[len(image_prefix) : -len("/image")].strip("/")
                if not raw_id.isdigit():
                    self.error_response(HTTPStatus.BAD_REQUEST, "意见编号无效")
                    return
                image = application.screenshot_path(int(raw_id))
                if image is None:
                    self.error_response(HTTPStatus.NOT_FOUND, "截图不存在")
                    return
                content = image.read_bytes()
                content_type = {".jpg": "image/jpeg", ".png": "image/png", ".webp": "image/webp"}.get(image.suffix, "application/octet-stream")
                self.send_response(HTTPStatus.OK)
                self.send_header("Content-Type", content_type)
                self.send_header("Content-Length", str(len(content)))
                self.send_header("Content-Disposition", "inline")
                self.end_headers()
                self.wfile.write(content)
                return
            if not path.startswith("/api/") and application.config.static_root is not None:
                relative = "index.html" if path == "/" else path.lstrip("/")
                candidate = (application.config.static_root / relative).resolve()
                if (
                    candidate.is_relative_to(application.config.static_root)
                    and candidate.is_file()
                    and not any(part.startswith(".") for part in Path(relative).parts)
                ):
                    content = candidate.read_bytes()
                    content_type = mimetypes.guess_type(candidate.name)[0] or "application/octet-stream"
                    self.send_response(HTTPStatus.OK)
                    self.send_header("Content-Type", content_type)
                    self.send_header("Content-Length", str(len(content)))
                    self.send_header("Cache-Control", "no-cache")
                    self.end_headers()
                    self.wfile.write(content)
                    return
            self.error_response(HTTPStatus.NOT_FOUND, "接口不存在")

        def do_POST(self) -> None:  # noqa: N802
            path = urlsplit(self.path).path.rstrip("/")
            if path != "/api/reviews":
                self.error_response(HTTPStatus.NOT_FOUND, "接口不存在")
                return
            if not self.require_authorized() or not self.require_origin():
                return
            client_ip = self.client_ip()
            if application.limiter.limited(f"submit:{client_ip}", 30, 3600):
                self.error_response(HTTPStatus.TOO_MANY_REQUESTS, "提交过于频繁，请稍后再试")
                return
            try:
                result = application.create_review(self.read_json(), client_ip, self.headers.get("User-Agent", ""))
            except ValueError as error:
                self.error_response(HTTPStatus.BAD_REQUEST, str(error))
                return
            except Exception:
                self.error_response(HTTPStatus.INTERNAL_SERVER_ERROR, "意见保存失败")
                return
            self.json_response(HTTPStatus.CREATED, result)

        def do_PATCH(self) -> None:  # noqa: N802
            path = urlsplit(self.path).path.rstrip("/")
            reviewer_prefix = "/api/reviews/"
            if path.startswith(reviewer_prefix):
                public_id = path[len(reviewer_prefix) :]
                if PUBLIC_ID_PATTERN.fullmatch(public_id) is None:
                    self.error_response(HTTPStatus.NOT_FOUND, "意见不存在")
                    return
                if not self.require_authorized() or not self.require_origin():
                    return
                if application.limiter.limited(f"mutate:{self.client_ip()}", 60, 3600):
                    self.error_response(HTTPStatus.TOO_MANY_REQUESTS, "操作过于频繁，请稍后再试")
                    return
                try:
                    result = application.edit_review(
                        public_id, self.read_json(), self.headers.get("X-Edit-Token", "").strip()
                    )
                except ValueError as error:
                    self.error_response(HTTPStatus.BAD_REQUEST, str(error))
                    return
                except Exception:
                    self.error_response(HTTPStatus.INTERNAL_SERVER_ERROR, "意见修改失败")
                    return
                self.reviewer_mutation_response(result, "意见已进入处理流程，不能再编辑")
                return
            prefix = "/api/admin/reviews/"
            if not path.startswith(prefix) or not path[len(prefix) :].isdigit():
                self.error_response(HTTPStatus.NOT_FOUND, "接口不存在")
                return
            if not self.require_authorized(True) or not self.require_origin():
                return
            try:
                found = application.update_review(int(path[len(prefix) :]), self.read_json())
            except ValueError as error:
                self.error_response(HTTPStatus.BAD_REQUEST, str(error))
                return
            except Exception:
                self.error_response(HTTPStatus.INTERNAL_SERVER_ERROR, "处理结果保存失败")
                return
            if not found:
                self.error_response(HTTPStatus.NOT_FOUND, "意见不存在")
                return
            self.json_response(HTTPStatus.OK, {"ok": True})

        def reviewer_mutation_response(self, result: str, conflict_message: str) -> None:
            if result == "ok":
                self.json_response(HTTPStatus.OK, {"ok": True})
            elif result == "conflict":
                self.error_response(HTTPStatus.CONFLICT, conflict_message)
            elif result == "forbidden":
                self.error_response(HTTPStatus.FORBIDDEN, "只能修改自己提交的意见")
            else:
                self.error_response(HTTPStatus.NOT_FOUND, "意见不存在")

        def do_DELETE(self) -> None:  # noqa: N802
            path = urlsplit(self.path).path.rstrip("/")
            prefix = "/api/reviews/"
            if not path.startswith(prefix) or PUBLIC_ID_PATTERN.fullmatch(path[len(prefix) :]) is None:
                self.error_response(HTTPStatus.NOT_FOUND, "接口不存在")
                return
            if not self.require_authorized() or not self.require_origin():
                return
            if application.limiter.limited(f"mutate:{self.client_ip()}", 60, 3600):
                self.error_response(HTTPStatus.TOO_MANY_REQUESTS, "操作过于频繁，请稍后再试")
                return
            try:
                result = application.withdraw_review(
                    path[len(prefix) :], self.headers.get("X-Edit-Token", "").strip()
                )
            except Exception:
                self.error_response(HTTPStatus.INTERNAL_SERVER_ERROR, "意见撤回失败")
                return
            self.reviewer_mutation_response(result, "意见已经结束，不能撤回")

    return Handler


def main() -> None:
    config = Config()
    config.validate()
    application = ReviewApplication(config)
    server = ThreadingHTTPServer((config.host, config.port), make_handler(application))
    signal.signal(signal.SIGTERM, lambda _signum, _frame: threading.Thread(target=server.shutdown, daemon=True).start())
    print(f"ChatCommons review API listening on {config.host}:{config.port}", flush=True)
    try:
        server.serve_forever(poll_interval=0.25)
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
