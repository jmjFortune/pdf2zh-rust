# Inline math, code and reader repair — 2026-10-23

Requested: correct overlapping inline formulas, clarify/protect source code, enlarge PDF reading area and enable arrow-key panning.

1. Reproduce against the retained completed NAS PDF; inspect raw glyph grouping, formula capture, typeset placements and code fonts — complete.
2. Fix the demonstrated engine defects with real PDF regression fixtures (formula ownership/geometry and code preservation), retaining source PDF/cached translations — complete; 23 tests and real-page geometry/code pixels verified.
3. Replace embedded browser-owned navigation with an application-controlled full-area reader; keyboard arrows scroll both axes without intercepting text inputs; keep exports and page navigation — implemented and local browser verified.
4. Run meaningful native/layout tests and production web build; render the affected real page locally without paid retranslation and inspect before/after. Verify reader controls through browser — complete.
5. Cloud build and idle-only NAS deployment with history, settings and input hashes preserved; verify final UI and document limitations — complete; run 40086728911, healthy revision 2d0ff61, ten records and exact settings hash retained, live reader asset hashes verified. Browser interaction was verified locally; post-deployment browser tool unavailable.

No original translator business code, user key disclosure, paid-provider rerun, proxy changes or deletion of hidden history.
