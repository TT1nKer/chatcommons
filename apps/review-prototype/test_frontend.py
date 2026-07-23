from __future__ import annotations

import re
import unittest
from pathlib import Path


PUBLIC = Path(__file__).parent / "public"
ROOT = Path(__file__).resolve().parents[2]


class FrontendLocalizationContractTest(unittest.TestCase):
    def test_product_version_is_consistent_across_workspace_and_prototype(self):
        version = (ROOT / "VERSION").read_text().strip()
        cargo = (ROOT / "Cargo.toml").read_text()
        html = (PUBLIC / "index.html").read_text()
        manifest = (PUBLIC / "version.json").read_text()
        cargo_version = re.search(r'^version = "([^"]+)"$', cargo, re.MULTILINE)
        self.assertIsNotNone(cargo_version)
        self.assertEqual(version, cargo_version.group(1))
        self.assertIn(f"v{version}", html)
        self.assertIn(f'"productVersion": "{version}"', manifest)

    def test_home_prioritizes_joining_and_surfaces_activity_before_recents(self):
        html = (PUBLIC / "index.html").read_text()
        join = '<button class="primary-action" type="button" data-action="join">'
        create = '<button class="secondary-action" type="button" data-action="create">'
        self.assertIn(join, html)
        self.assertIn(create, html)
        self.assertLess(html.index('class="pulse-section"'), html.index('class="continue-section"'))

    def test_first_view_explains_the_product_before_requesting_ui_feedback(self):
        html = (PUBLIC / "index.html").read_text()
        localization = (PUBLIC / "i18n.js").read_text()
        application = (PUBLIC / "app.js").read_text()
        self.assertIn("让社区聊天不再被单一平台锁住。", html)
        self.assertIn("Community chat without platform lock-in", html)
        self.assertIn('data-action="copy-brief"', html)
        self.assertIn("releases/tag/v0.1.0-alpha.3", html)
        self.assertIn("data-review-only", html)
        self.assertIn("桌面 alpha 已连接真实签名聊天", html)
        self.assertIn("function openAbout()", application)
        self.assertIn("function copyProjectBrief()", application)
        self.assertIn("function openCommunityBrowser(roomsOnly = false)", application)
        self.assertIn("Community chat without platform lock-in.", localization)
        self.assertNotIn('class="connection-pill"', html)

    def test_language_runtime_loads_before_interactions(self):
        html = (PUBLIC / "index.html").read_text()
        self.assertLess(html.index('src="./i18n.js?v='), html.index('src="./app.js?v='))
        self.assertIn('id="language-toggle"', html)
        self.assertIn('data-action="toggle-language"', html)

    def test_static_entry_assets_share_a_cache_busting_revision(self):
        html = (PUBLIC / "index.html").read_text()
        assets = re.findall(
            r'(?:href|src)="\./(?:styles\.css|review\.css|i18n\.js|review\.js|app\.js)\?v=([^"]+)"',
            html,
        )
        self.assertEqual(len(assets), 5)
        self.assertEqual(len(set(assets)), 1)
        self.assertNotIn('href="./styles.css"', html)
        self.assertNotIn('src="./review.js"', html)

    def test_review_toolbar_does_not_wait_for_screenshot_library(self):
        html = (PUBLIC / "index.html").read_text()
        review = (PUBLIC / "review.js").read_text()
        self.assertNotIn('src="./vendor/html2canvas.min.js"', html)
        self.assertLess(html.index('src="./review.js?v='), html.index('src="./app.js?v='))
        self.assertIn("script.src = './vendor/html2canvas.min.js?v=", review)
        self.assertIn("const html2canvas = await loadScreenshotLibrary()", review)

    def test_locale_is_persistent_and_review_context_is_language_neutral(self):
        localization = (PUBLIC / "i18n.js").read_text()
        application = (PUBLIC / "app.js").read_text()
        review = (PUBLIC / "review.js").read_text()
        self.assertIn("chatcommons-locale", localization)
        self.assertIn("localStorage.setItem", localization)
        self.assertIn("chatcommons:locale-change", application)
        self.assertIn("dataset.reviewScreen", application)
        self.assertIn("dataset.reviewScreen", review)
        self.assertIn("return document.documentElement.dataset.reviewScreen", review)

    def test_reviewer_can_reshare_and_manage_only_owned_feedback(self):
        review = (PUBLIC / "review.js").read_text()
        self.assertIn("data-review-share", review)
        self.assertIn("link.searchParams.set('review', token)", review)
        self.assertIn("chatcommons-review-edit-tokens-v1", review)
        self.assertIn("'X-Edit-Token': editToken", review)
        self.assertIn("method: 'PATCH'", review)
        self.assertIn("method: 'DELETE'", review)
        self.assertIn("rememberEditToken(created.publicId, created.editToken)", review)

    def test_reviewer_toolbar_publicly_thanks_feedback_contributors(self):
        review = (PUBLIC / "review.js").read_text()
        localization = (PUBLIC / "i18n.js").read_text()
        self.assertIn("Thank you, early reviewers", review)
        self.assertIn("Your comments about the project explanation", review)
        self.assertIn("credit you as early product and design contributors", review)
        self.assertIn("prefer to stay anonymous", review)
        self.assertIn("感谢每一位早期评审者", localization)
        self.assertIn("我们也希望把你列为早期产品与设计贡献者", localization)
        self.assertIn("data-review-credit", review)
        self.assertIn("function openContributorForm()", review)
        self.assertIn("name=\"creditName\"", review)
        self.assertIn("name=\"anonymous\"", review)
        self.assertIn("screen: 'contributor-credit'", review)
        self.assertIn("rememberEditToken(created.publicId, created.editToken)", review)
        self.assertIn("提交署名信息", localization)

    def test_owner_inbox_has_a_bilingual_thank_you_close_loop(self):
        admin = (PUBLIC / "admin.js").read_text()
        self.assertIn("const thankYouReply", admin)
        self.assertIn("Thank you for taking the time", admin)
        self.assertIn("感谢并待验收", admin)
        self.assertIn("select.value='client_review'", admin)

    def test_review_markers_follow_document_and_nested_scroll(self):
        review = (PUBLIC / "review.js").read_text()
        styles = (PUBLIC / "review.css").read_text()
        self.assertIn("scrollX: Math.max(0, window.scrollX)", review)
        self.assertIn("scrollY: Math.max(0, window.scrollY)", review)
        self.assertIn("document.addEventListener('scroll', scheduleMarkerPositions, true)", review)
        self.assertIn("new ResizeObserver(scheduleMarkerPositions)", review)
        self.assertIn("targetFor(item)", review)
        self.assertIn("position: absolute", styles)
        self.assertNotIn("position: fixed; z-index: 990", styles)

    def test_review_toolbar_can_collapse_away_from_bottom_right_actions(self):
        review = (PUBLIC / "review.js").read_text()
        styles = (PUBLIC / "review.css").read_text()
        localization = (PUBLIC / "i18n.js").read_text()
        self.assertIn("data-review-collapse", review)
        self.assertIn("function setCollapsed(value)", review)
        self.assertIn("collapsed: false", review)
        self.assertNotIn("chatcommons-review-collapsed", review)
        self.assertIn(".review-toolbar.is-collapsed", styles)
        self.assertIn("bottom: 42%", styles)
        self.assertIn("收起评审工具", localization)
        self.assertIn("展开评审工具", localization)

    def test_download_entry_is_only_revealed_after_review_authorization(self):
        html = (PUBLIC / "index.html").read_text()
        application = (PUBLIC / "app.js").read_text()
        review = (PUBLIC / "review.js").read_text()
        styles = (PUBLIC / "styles.css").read_text()
        self.assertIn("data-review-only", html)
        self.assertIn("alpha-access-banner", html)
        self.assertIn("alpha-download-button", html)
        self.assertIn("当前仅开放朋友内测", html)
        self.assertIn("Currently open to invited friends only", html)
        self.assertIn("测试资格通过邀请的评审链接发放。", html)
        self.assertIn("Alpha access is shared through invited review links.", html)
        self.assertIn("Download desktop alpha", html)
        self.assertIn("data-no-i18n", html)
        self.assertNotIn("data-review-only", application)
        self.assertIn("dataset.reviewAuthorized = 'true'", review)
        self.assertIn('[data-review-only] { display: none !important; }', styles)
        self.assertIn('html[data-review-authorized="true"] [data-review-only]', styles)

    def test_feedback_forms_scroll_and_have_no_product_character_limit(self):
        review = (PUBLIC / "review.js").read_text()
        styles = (PUBLIC / "review.css").read_text()
        self.assertNotIn(
            'name="message" required minlength="2" maxlength=', review
        )
        self.assertIn("max-height: calc(100vh - 32px)", styles)
        self.assertIn("overflow: auto", styles)

    def test_compact_density_is_coherent_and_persistent(self):
        application = (PUBLIC / "app.js").read_text()
        styles = (PUBLIC / "styles.css").read_text()
        self.assertIn("chatcommons-density", application)
        self.assertIn("localStorage.setItem(densityStorageKey, density)", application)
        self.assertIn("applyDensity(storedDensity(), false)", application)
        for selector in (
            '.app-shell[data-density="compact"] .workspace',
            '.app-shell[data-density="compact"] .pulse-list button',
            '.app-shell[data-density="compact"] .community-card',
            '.app-shell[data-density="compact"] .room-strip',
            '.app-shell[data-density="compact"] .message',
            '.app-shell[data-density="compact"] .composer',
        ):
            self.assertIn(selector, styles)


if __name__ == "__main__":
    unittest.main()
