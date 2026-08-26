#!/usr/bin/env python3
"""Minimal OCR adapter for RustDesk --api-config ocr_cmd.

Replace the body with Tesseract/Paddle/Windows.OCR as needed.
RustDesk invokes:  ocr-stub.py /path/to/frame.jpg
and expects JSON on stdout (or plain text).

Example api-config:
  "ocr_cmd": ["python3", "examples/ocr-stub.py", "{image}"]
"""
import json
import sys

# Pass-through: real deployments should OCR sys.argv[1] (JPEG path).
# This stub prints empty text so the client falls back to CAD + password.
print(json.dumps({"ok": True, "text": "", "lines": []}))
