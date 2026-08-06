#!/usr/bin/env python3
"""公共 JSON 文档标记的回归测试。"""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

import schema_docs_markers as markers


class SchemaDocsMarkerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp_dir = tempfile.TemporaryDirectory()
        self.root = Path(self.temp_dir.name)
        (self.root / "docs").mkdir()
        fixture = self.root / "schemas/fixtures/api/example.v1.valid.json"
        fixture.parent.mkdir(parents=True)
        fixture.write_text('{"data":{"ok":true}}\n', encoding="utf-8")
        manifest = self.root / "schemas/json-schema/draft-2020-12/manifest.json"
        manifest.parent.mkdir(parents=True)
        manifest.write_text(
            json.dumps(
                {
                    "roots": [
                        {
                            "contract_id": "api.example.response",
                            "schema_fixture": "schemas/fixtures/api/example.v1.valid.json",
                        }
                    ]
                }
            ),
            encoding="utf-8",
        )

    def tearDown(self) -> None:
        self.temp_dir.cleanup()

    def write_doc(self, body: str) -> None:
        (self.root / "docs/API_SPEC.md").write_text(body, encoding="utf-8")

    def assert_rejected(self, body: str, message: str) -> None:
        self.write_doc(body)
        with self.assertRaisesRegex(markers.MarkerError, message):
            markers.validate_repository(self.root)

    def test_exact_fixture_marker_passes(self) -> None:
        self.write_doc(
            "<!-- schema-doc: contract=api.example.response fixture=schemas/fixtures/api/example.v1.valid.json -->\n"
            "```json\n{\"data\": {\"ok\": true}}\n```\n"
        )
        self.assertEqual(markers.validate_repository(self.root), (1, 0))

    def test_reasoned_ignore_marker_passes(self) -> None:
        self.write_doc("<!-- schema-doc-ignore: illustrative partial payload -->\n```json\n{...}\n```\n")
        self.assertEqual(markers.validate_repository(self.root), (0, 1))

    def test_unmarked_json_block_is_rejected(self) -> None:
        self.assert_rejected("```json\n{}\n```\n", "unmarked public JSON")

    def test_unmarked_four_backtick_json_block_is_rejected(self) -> None:
        self.assert_rejected("````json\n{}\n````\n", "unmarked public JSON")

    def test_unmarked_tilde_json_block_is_rejected(self) -> None:
        self.assert_rejected("~~~json\n{}\n~~~\n", "unmarked public JSON")

    def test_unmarked_json_block_with_info_attributes_is_rejected(self) -> None:
        self.assert_rejected(
            '  ```json title="response"\n{}\n  ```\n',
            "unmarked public JSON",
        )

    def test_marked_commonmark_fence_variants_pass(self) -> None:
        self.write_doc(
            "<!-- schema-doc: contract=api.example.response fixture=schemas/fixtures/api/example.v1.valid.json -->\n"
            ' ````json title="response"\n{\"data\": {\"ok\": true}}\n `````\n'
            "<!-- schema-doc-ignore: illustrative tilde payload -->\n"
            "~~~json {#partial}\n{...}\n~~~~\n"
        )
        self.assertEqual(markers.validate_repository(self.root), (1, 1))

    def test_shorter_or_different_closing_fence_does_not_close_json_block(self) -> None:
        self.assert_rejected(
            "<!-- schema-doc: contract=api.example.response fixture=schemas/fixtures/api/example.v1.valid.json -->\n"
            "````json\n{\"data\":{\"ok\":true}}\n```\n````\n",
            "not valid JSON",
        )
        self.assert_rejected(
            "<!-- schema-doc: contract=api.example.response fixture=schemas/fixtures/api/example.v1.valid.json -->\n"
            "~~~json\n{\"data\":{\"ok\":true}}\n```\n~~~\n",
            "not valid JSON",
        )

    def test_javascript_and_jsonc_fences_are_not_public_json_examples(self) -> None:
        self.write_doc("```javascript\n{}\n```\n```jsonc\n// comment\n{}\n```\n")
        self.assertEqual(markers.validate_repository(self.root), (0, 0))

    def test_malformed_marker_is_rejected(self) -> None:
        self.assert_rejected(
            "<!-- schema-doc: api.example.response -->\n```json\n{}\n```\n",
            "malformed schema-doc marker",
        )

    def test_unknown_contract_is_rejected(self) -> None:
        self.assert_rejected(
            "<!-- schema-doc: contract=api.unknown.response fixture=schemas/fixtures/api/example.v1.valid.json -->\n"
            "```json\n{\"data\":{\"ok\":true}}\n```\n",
            "unknown contract",
        )

    def test_contract_fixture_mapping_mismatch_is_rejected(self) -> None:
        other = self.root / "schemas/fixtures/api/other.v1.valid.json"
        other.write_text("{}\n", encoding="utf-8")
        self.assert_rejected(
            "<!-- schema-doc: contract=api.example.response fixture=schemas/fixtures/api/other.v1.valid.json -->\n"
            "```json\n{}\n```\n",
            "fixture mismatch",
        )

    def test_inline_payload_mismatch_is_rejected(self) -> None:
        self.assert_rejected(
            "<!-- schema-doc: contract=api.example.response fixture=schemas/fixtures/api/example.v1.valid.json -->\n"
            "```json\n{\"data\":{\"ok\":false}}\n```\n",
            "does not match fixture",
        )

    def test_orphan_marker_is_rejected(self) -> None:
        self.assert_rejected(
            "<!-- schema-doc-ignore: stale marker -->\ntext instead of a JSON block\n",
            "must be immediately followed",
        )


if __name__ == "__main__":
    unittest.main()
