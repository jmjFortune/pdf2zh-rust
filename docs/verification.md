# Verification record

## 2026-10-23 local development

- Platform: Apple Silicon, Rust 1.96.0. `mupdf` 0.8.0; `ort` 2.0.0-rc.15, native ONNX Runtime 1.31.0. Cargo.lock records the resolved dependencies.
- Native build and tests passed. The latest suite has 11 tests: six unit tests, four PDF integration tests, and one HTTP/process lifecycle integration test. The HTTP fixture is explicitly a mock OpenAI-compatible translator; it is not presented as live translation quality evidence.
- Live Google translation succeeded on the synthetic public test paragraph, with mono/side-by-side output rendered and visually inspected. The public `https://arxiv.org/pdf/1706.03762` was used for selected-page regression checks; source PDF and translations stay in ignored artifacts.
- Real native layout inference produced class-labeled detections using the pre-existing model. No Python interpreter is called by the application.
- A real-paper inline-math defect was found and fixed: whole paragraphs now carry exact formula markers; original formulas are moved as native PDF forms. Text outside each captured formula is removed before clipping, preventing invisible copies of whole source paragraphs.
- Source Han font aliases initially yielded compatibility ideographs such as `器` when copying visually correct `器`. PDF ActualText now preserves exact translated Unicode. The real-paper extraction found seven exact occurrences of `编码器`, and one original `LayerNorm` formula, after this fix.
- Local Web upload, queue, completion and download links were observed in the browser. An embedded-PDF viewer black-screen issue was found; preview now uses streamed PNG pages rendered by separate Rust processes. API preview checks pass; final browser recheck is pending the latest build.
- During early local service checks, resident process RSS was approximately 8–10 MiB and returned to that range after its worker exited. A layout/translation worker was observed around 1.3 GiB. This is not a controlled comparison to the NAS original's 334.4 MiB cgroup figure, and no reduction of translation peak memory is claimed.
- Main release executable was approximately 10 MiB before the latest provider/resource additions. Its direct `otool -L` dependencies were system `libiconv` and `libSystem`; ONNX Runtime is loaded dynamically. Final bundle checks and size/memory measurements are pending.

## Remaining coverage

Full upstream configuration/provider parity, cross-page paragraphs, complete table text translation, OCR, broad multilingual shaping, font subsetting, production packaging and NAS deployment are not complete. The development preview and README state these limits explicitly.
