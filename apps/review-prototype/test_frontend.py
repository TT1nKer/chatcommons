from __future__ import annotations

import unittest
from pathlib import Path


PUBLIC = Path(__file__).parent / "public"


class FrontendLocalizationContractTest(unittest.TestCase):
    def test_home_prioritizes_joining_and_surfaces_activity_before_recents(self):
        html = (PUBLIC / "index.html").read_text()
        join = '<button class="primary-action" type="button" data-action="join">'
        create = '<button class="secondary-action" type="button" data-action="create">'
        self.assertIn(join, html)
        self.assertIn(create, html)
        self.assertLess(html.index('class="pulse-section"'), html.index('class="continue-section"'))

    def test_language_runtime_loads_before_interactions(self):
        html = (PUBLIC / "index.html").read_text()
        self.assertLess(html.index('src="./i18n.js"'), html.index('src="./app.js"'))
        self.assertIn('id="language-toggle"', html)
        self.assertIn('data-action="toggle-language"', html)

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


if __name__ == "__main__":
    unittest.main()
