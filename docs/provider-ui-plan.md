# Provider and terminology follow-up — 2026-10-23

User-directed design: remove About entirely; label every visible Side by Side as Dual; preserve Interleaved. Expand the six-entry adapter catalogue to twenty real configuration profiles using existing native adapters and documented OpenAI-compatible endpoints. Native Tencent MT and the previously failing Bing endpoint are not disguised as working profiles. Model selection remains account-specific.

1. Inspect original catalogue and official endpoint docs — done.
2. Remove About and its exclusive download; update labels and stale-tab fallback — done.
3. Add provider profiles, explicit Azure parameters and early missing-settings validation; preserve all old IDs and keys — done.
4. Verify provider routing/privacy with a local mock, build frontend/native tests, check actual UI — done (17 tests).
5. Push cloud build, upgrade idle NAS preserving all settings/history/hidden flags, verify final screen and catalogue — done; run 39955888217, source adc81bf, settings unchanged and eight records retained.

No Python/Node runtime, paid provider requests, credentials import, or original translator source changes.
