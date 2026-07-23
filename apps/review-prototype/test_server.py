from __future__ import annotations

import base64
import json
import os
import tempfile
import threading
import unittest
import urllib.error
import urllib.request
from http.server import ThreadingHTTPServer
from pathlib import Path

from server import Config, ReviewApplication, make_handler


REVIEW_TOKEN = "review-" + "a" * 48
OWNER_TOKEN = "owner-" + "b" * 48
ORIGIN = "https://review.example.test"


class ReviewServerTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.temporary = tempfile.TemporaryDirectory()
        root = Path(cls.temporary.name)
        os.environ.update(
            {
                "REVIEW_DB_PATH": str(root / "reviews.sqlite3"),
                "REVIEW_SCREENSHOT_DIR": str(root / "screenshots"),
                "REVIEW_TOKEN": REVIEW_TOKEN,
                "OWNER_TOKEN": OWNER_TOKEN,
                "REVIEW_ALLOWED_ORIGIN": ORIGIN,
            }
        )
        config = Config()
        config.validate()
        cls.application = ReviewApplication(config)
        cls.server = ThreadingHTTPServer(("127.0.0.1", 0), make_handler(cls.application))
        cls.thread = threading.Thread(target=cls.server.serve_forever, daemon=True)
        cls.thread.start()
        cls.base = f"http://127.0.0.1:{cls.server.server_port}"

    @classmethod
    def tearDownClass(cls) -> None:
        cls.server.shutdown()
        cls.server.server_close()
        cls.thread.join(timeout=2)
        cls.temporary.cleanup()

    def request(self, method: str, path: str, payload=None, token=None, owner=False, origin=ORIGIN, edit_token=None):
        body = None if payload is None else json.dumps(payload).encode()
        headers = {}
        if payload is not None:
            headers["Content-Type"] = "application/json"
        if token:
            headers["X-Owner-Token" if owner else "X-Review-Token"] = token
        if edit_token:
            headers["X-Edit-Token"] = edit_token
        if origin:
            headers["Origin"] = origin
        request = urllib.request.Request(self.base + path, data=body, headers=headers, method=method)
        try:
            with urllib.request.urlopen(request, timeout=3) as response:
                return response.status, response.headers, response.read()
        except urllib.error.HTTPError as error:
            try:
                return error.code, error.headers, error.read()
            finally:
                error.close()

    @staticmethod
    def valid_payload(screenshot=""):
        return {
            "surface": "prototype",
            "screen": "home · ChatCommons",
            "targetId": "button.card[data-x='<script>']",
            "targetText": "<b>不执行</b>",
            "x": 0.4,
            "y": 0.6,
            "scrollX": 12,
            "scrollY": 640,
            "viewportWidth": 1440,
            "viewportHeight": 900,
            "category": "layout",
            "priority": "normal",
            "message": "'); DROP TABLE reviews; -- <script>alert(1)</script>",
            "screenshot": screenshot,
        }

    @staticmethod
    def desktop_payload(screenshot=""):
        return {
            "surface": "desktop",
            "screen": "community",
            "targetId": "chat-window",
            "targetText": "ChatCommons desktop feedback",
            "x": 0.5,
            "y": 0.5,
            "scrollX": 0,
            "scrollY": 0,
            "viewportWidth": 1180,
            "viewportHeight": 760,
            "category": "feature",
            "priority": "normal",
            "message": "## What happened\n\n" + "很长的说明\n" * 800,
            "screenshot": screenshot,
        }

    def test_config_rejects_non_origin_urls(self):
        original = os.environ["REVIEW_ALLOWED_ORIGIN"]
        try:
            for invalid in (
                "ftp://review.example.test",
                "https://user@review.example.test",
                "https://review.example.test/path",
                "https://review.example.test?query=yes",
                "https://review.example.test#fragment",
            ):
                os.environ["REVIEW_ALLOWED_ORIGIN"] = invalid
                with self.subTest(origin=invalid), self.assertRaises(RuntimeError):
                    Config().validate()
        finally:
            os.environ["REVIEW_ALLOWED_ORIGIN"] = original

    def test_missing_wrong_and_valid_reviewer_credentials(self):
        status, _, _ = self.request("GET", "/api/reviews")
        self.assertEqual(status, 401)
        status, _, _ = self.request("GET", "/api/reviews", token="wrong-" + "x" * 48)
        self.assertEqual(status, 401)
        status, _, _ = self.request("GET", "/api/reviews", token=REVIEW_TOKEN)
        self.assertEqual(status, 200)
        status, _, _ = self.request("GET", "/api/admin/reviews", token=REVIEW_TOKEN, owner=True)
        self.assertEqual(status, 401)

    def test_create_list_update_and_audit_inert_text(self):
        payload = self.valid_payload()
        payload["message"] = "第一行\n" + "很长的网页反馈\n" * 800
        status, _, body = self.request("POST", "/api/reviews", payload, REVIEW_TOKEN)
        self.assertEqual(status, 201, body)
        created = json.loads(body)
        self.assertNotIn("id", created)
        self.assertGreaterEqual(len(created["editToken"]), 40)
        status, _, body = self.request("GET", "/api/reviews", token=REVIEW_TOKEN)
        reviewer_item = json.loads(body)["reviews"][0]
        self.assertNotIn("id", reviewer_item)
        self.assertNotIn("targetId", reviewer_item)
        self.assertGreater(len(reviewer_item["message"]), 1000)
        self.assertIn("\n", reviewer_item["message"])
        self.assertEqual(reviewer_item["scrollX"], 12)
        self.assertEqual(reviewer_item["scrollY"], 640)
        status, _, body = self.request("GET", "/api/admin/reviews", token=OWNER_TOKEN, owner=True)
        self.assertEqual(status, 200)
        admin_item = json.loads(body)["reviews"][0]
        self.assertEqual(admin_item["publicId"], created["publicId"])
        status, _, _ = self.request(
            "PATCH",
            f"/api/admin/reviews/{admin_item['id']}",
            {"status": "client_review", "adminReply": "已修改，请验收 <script>"},
            OWNER_TOKEN,
            owner=True,
        )
        self.assertEqual(status, 200)
        status, _, body = self.request("GET", "/api/reviews", token=REVIEW_TOKEN)
        updated = json.loads(body)["reviews"][0]
        self.assertEqual(updated["status"], "client_review")
        self.assertIn("<script>", updated["adminReply"])
        with self.application.database() as connection:
            self.assertEqual(connection.execute("SELECT COUNT(*) FROM audit_log").fetchone()[0], 1)

    def test_origin_unknown_status_and_fake_image_are_rejected(self):
        status, _, _ = self.request("POST", "/api/reviews", self.valid_payload(), REVIEW_TOKEN, origin="https://evil.example")
        self.assertEqual(status, 403)
        fake = "data:image/png;base64," + base64.b64encode(b"not really a png").decode()
        status, _, _ = self.request("POST", "/api/reviews", self.valid_payload(fake), REVIEW_TOKEN)
        self.assertEqual(status, 400)
        oversized = "data:image/png;base64," + "A" * 1_500_001
        status, _, _ = self.request("POST", "/api/reviews", self.valid_payload(oversized), REVIEW_TOKEN)
        self.assertEqual(status, 400)
        status, _, body = self.request("GET", "/api/admin/reviews", token=OWNER_TOKEN, owner=True)
        review_id = json.loads(body)["reviews"][0]["id"]
        status, _, _ = self.request("PATCH", f"/api/admin/reviews/{review_id}", {"status": "unknown", "adminReply": ""}, OWNER_TOKEN, owner=True)
        self.assertEqual(status, 400)
        status, _, body = self.request(
            "PATCH",
            f"/api/admin/reviews/{review_id}",
            {"status": "client_review", "adminReply": ""},
            OWNER_TOKEN,
            owner=True,
        )
        self.assertEqual(status, 400, body)

    def test_private_real_screenshot(self):
        png = b"\x89PNG\r\n\x1a\n" + b"test-payload"
        screenshot = "data:image/png;base64," + base64.b64encode(png).decode()
        status, _, body = self.request("POST", "/api/reviews", self.valid_payload(screenshot), REVIEW_TOKEN)
        self.assertEqual(status, 201)
        public_id = json.loads(body)["publicId"]
        status, _, body = self.request("GET", "/api/admin/reviews", token=OWNER_TOKEN, owner=True)
        review_id = next(item["id"] for item in json.loads(body)["reviews"] if item["publicId"] == public_id)
        status, _, _ = self.request("GET", f"/api/admin/reviews/{review_id}/image")
        self.assertEqual(status, 401)
        status, headers, body = self.request("GET", f"/api/admin/reviews/{review_id}/image", token=OWNER_TOKEN, owner=True)
        self.assertEqual(status, 200)
        self.assertEqual(headers.get_content_type(), "image/png")
        self.assertEqual(body, png)

    def test_desktop_feedback_needs_no_embedded_secret_and_has_private_receipt(self):
        png = b"\x89PNG\r\n\x1a\n" + b"desktop-screenshot"
        screenshot = "data:image/png;base64," + base64.b64encode(png).decode()
        payload = self.desktop_payload(screenshot)
        status, _, body = self.request(
            "POST", "/api/app-feedback", payload, token=None, origin=None
        )
        self.assertEqual(status, 201, body)
        created = json.loads(body)
        self.assertGreater(len(payload["message"]), 1000)

        status, _, _ = self.request(
            "GET", f"/api/app-feedback/{created['publicId']}", origin=None
        )
        self.assertEqual(status, 404)
        status, _, body = self.request(
            "GET",
            f"/api/app-feedback/{created['publicId']}",
            origin=None,
            edit_token=created["editToken"],
        )
        self.assertEqual(status, 200, body)
        receipt = json.loads(body)
        self.assertEqual(receipt["status"], "pending")

        status, _, body = self.request(
            "GET", "/api/reviews", token=REVIEW_TOKEN
        )
        self.assertEqual(status, 200)
        self.assertNotIn(
            created["publicId"],
            {item["publicId"] for item in json.loads(body)["reviews"]},
        )

        status, _, body = self.request(
            "GET", "/api/admin/reviews", token=OWNER_TOKEN, owner=True
        )
        self.assertEqual(status, 200)
        item = next(
            item
            for item in json.loads(body)["reviews"]
            if item["publicId"] == created["publicId"]
        )
        self.assertEqual(item["surface"], "desktop")
        self.assertTrue(item["hasScreenshot"])
        self.assertGreater(len(item["message"]), 1000)

    def test_reviewer_can_edit_and_withdraw_only_their_own_pending_review(self):
        payload = self.valid_payload()
        payload["message"] = "初始意见"
        status, _, body = self.request("POST", "/api/reviews", payload, REVIEW_TOKEN)
        self.assertEqual(status, 201, body)
        created = json.loads(body)
        path = f"/api/reviews/{created['publicId']}"

        edited = {
            "category": "feature",
            "priority": "high",
            "message": "更新后的意见 <script>\n" + "继续说明\n" * 800,
        }
        status, _, _ = self.request(
            "PATCH", path, edited, REVIEW_TOKEN, edit_token="wrong-" + "x" * 48
        )
        self.assertEqual(status, 403)
        status, _, _ = self.request(
            "PATCH", path, edited, REVIEW_TOKEN, edit_token=created["editToken"]
        )
        self.assertEqual(status, 200)
        status, _, body = self.request("GET", "/api/reviews", token=REVIEW_TOKEN)
        updated = next(
            item
            for item in json.loads(body)["reviews"]
            if item["publicId"] == created["publicId"]
        )
        self.assertGreater(len(updated["message"]), 1000)

        status, _, body = self.request("GET", "/api/reviews", token=REVIEW_TOKEN)
        item = next(review for review in json.loads(body)["reviews"] if review["publicId"] == created["publicId"])
        self.assertEqual(item["category"], "feature")
        self.assertEqual(item["priority"], "high")
        self.assertIn("<script>", item["message"])

        status, _, _ = self.request(
            "DELETE", path, token=REVIEW_TOKEN, edit_token=created["editToken"]
        )
        self.assertEqual(status, 200)
        status, _, body = self.request("GET", "/api/reviews", token=REVIEW_TOKEN)
        self.assertNotIn(created["publicId"], {item["publicId"] for item in json.loads(body)["reviews"]})
        status, _, body = self.request("GET", "/api/admin/reviews", token=OWNER_TOKEN, owner=True)
        withdrawn = next(item for item in json.loads(body)["reviews"] if item["publicId"] == created["publicId"])
        self.assertEqual(withdrawn["status"], "withdrawn")

        with self.application.database() as connection:
            actions = {
                row[0]
                for row in connection.execute(
                    "SELECT action FROM audit_log WHERE review_id=?", (withdrawn["id"],)
                )
            }
        self.assertEqual(actions, {"review.reviewer_edit", "review.withdraw"})

    def test_reviewer_cannot_edit_after_owner_starts_processing(self):
        payload = self.valid_payload()
        payload["message"] = "等待管理员处理"
        status, _, body = self.request("POST", "/api/reviews", payload, REVIEW_TOKEN)
        self.assertEqual(status, 201, body)
        created = json.loads(body)
        status, _, body = self.request("GET", "/api/admin/reviews", token=OWNER_TOKEN, owner=True)
        admin_item = next(item for item in json.loads(body)["reviews"] if item["publicId"] == created["publicId"])
        status, _, _ = self.request(
            "PATCH",
            f"/api/admin/reviews/{admin_item['id']}",
            {"status": "in_progress", "adminReply": "正在处理"},
            OWNER_TOKEN,
            owner=True,
        )
        self.assertEqual(status, 200)
        status, _, _ = self.request(
            "PATCH",
            f"/api/reviews/{created['publicId']}",
            {"category": "copy", "priority": "normal", "message": "不应保存"},
            REVIEW_TOKEN,
            edit_token=created["editToken"],
        )
        self.assertEqual(status, 409)


if __name__ == "__main__":
    unittest.main()
