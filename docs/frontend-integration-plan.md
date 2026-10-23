# Translator frontend integration — 2026-10-23

User-approved scope: replace NAS minimal UI with existing translator frontend; preserve its visuals and Rust runtime.

1. Copy frontend into this independent repository, leave original sources untouched.
2. Implement a separate /api/ui adapter: private settings, glossary, sanitized job snapshots, history, retained-input retry, SSE and downloads; keep legacy API.
3. Serve Next static export; translate once and export selected PDF variants. Show actual engine stages and restore task settings.
4. Verify local production build, native suite, mock-provider integration and browser flows.
5. Push, verify cloud image, upgrade idle NAS preserving volume/cache/history; check real cached page-32 font regression and deployed UI.

Limits: Rust OCR/structured paragraph reader/Zotero are not implemented. PDF reader uses existing embedded-PDF fallback. No model/API credentials copied from translator.
