# Verification record

## 2026-10-23 local development

- Platform: Apple Silicon, Rust 1.96.0. `mupdf` 0.8.0; `ort` 2.0.0-rc.15, native ONNX Runtime 1.31.0. Cargo.lock records the resolved dependencies.
- Native build and tests passed. The latest suite has 11 tests: six unit tests, four PDF integration tests, and one HTTP/process lifecycle integration test. The HTTP fixture is explicitly a mock OpenAI-compatible translator; it is not presented as live translation quality evidence.
- Live Google translation succeeded on the synthetic public test paragraph, with mono/side-by-side output rendered and visually inspected. The public `https://arxiv.org/pdf/1706.03762` was used for selected-page regression checks; source PDF and translations stay in ignored artifacts.
- Real native layout inference produced class-labeled detections using the pre-existing model. No Python interpreter is called by the application.
- A real-paper inline-math defect was found and fixed: whole paragraphs now carry exact formula markers; original formulas are moved as native PDF forms. Text outside each captured formula is removed before clipping, preventing invisible copies of whole source paragraphs.
- Source Han font aliases initially yielded compatibility ideographs such as `器` when copying visually correct `器`. PDF ActualText now preserves exact translated Unicode. The real-paper extraction found seven exact occurrences of `编码器`, and one original `LayerNorm` formula, after this fix.
- Latest full suite passed: six unit, four PDF integration and one lifecycle integration test. The lifecycle test now includes real native model analysis, exact Greek inline-formula extraction in reading order, queued and blocked cancellation, restart recovery, streamed PNG preview and removal of private job snapshots. Release build passed after the final UI stage-label change.
- Final browser check passed with the standalone release bundle launched from `/private/tmp`: synthetic upload → native layout → live Google → vector-preserving side-by-side PDF → visible PNG preview → browser download. The downloaded file has a PDF-1.7 header. Screenshot: ignored `artifacts/local-web-preview.jpg`. No Python interpreter is involved in these stages.
- Model inference now runs in a separate Rust analyzer process that exits before translation requests. Optional lifecycle profiling (`PDF2ZH_PROFILE_MEMORY=1 cargo test --test service_lifecycle`) sampled process RSS: analyzer peak 1296.6 MiB, translation-worker peak 71.9 MiB, service peak 11.9 MiB. During the first two seconds of a deliberately delayed local mock request, analyzer RSS was zero, worker RSS 29.3 MiB and service RSS 11.4 MiB. This checks allocation lifetime, not real-provider throughput or overall peak reduction. Samples are in ignored `artifacts/native-stage-memory.json`.
- After the final browser job completed, release-service RSS samples were approximately 5–7 MiB on this Mac. RSS varies with macOS memory management. These process figures are not a controlled comparison to the NAS original's 334.4 MiB cgroup figure; NAS validation remains pending.
- Main release executable is approximately 12.0 MiB. Its direct `otool -L` dependencies are system `libiconv` and `libSystem`; ONNX Runtime is loaded dynamically. The standalone folder contains the executable, model, font and native runtime; resource hashes were verified and standalone translation passed with Python/resource environment variables unset. Native runtime assets remain OS/architecture-specific.
- Bing returned HTTP 401 in live development checks, including the cookie-session implementation. That adapter remains experimental and is omitted from the Web selector; other unverified adapters are not claimed to work live.

## 2026-10-23 Linux amd64 cloud image

- Public repository: https://github.com/jmjFortune/pdf2zh-rust . User explicitly
  selected the Rust repository and authorized public visibility after checking
  that only the separate translator repository had previously been public.
- Source/image revision: `8c5dbb4b2d68c0d4d1cf7215c8049cd6cb5ef99d`.
- GitHub-hosted Ubuntu 24.04 run
  [39885551001](https://github.com/jmjFortune/pdf2zh-rust/actions/runs/39885551001)
  succeeded; job duration 9m34s. No Docker images were downloaded or built on Mac.
- Docker's Linux release suite passed six unit tests, four PDF integration tests
  and one real HTTP/process lifecycle integration test. That last test took
  8.55s and used native Linux ONNX inference with a local mock translator.
- The final runtime container passed font/model hash verification, native model
  inference with nonempty detections, HTTP health and UI response, and UID 10001.
- Published image: `ghcr.io/jmjfortune/pdf2zh-rust:sha-8c5dbb4b2d68`; `dev` matched
  that image at verification time. Both publish logs report manifest digest
  `sha256:2dfb6624948c13d3eef6c6f6ebee76a08b6810c1d7b85673ae0dd122935222be`.
- An unauthenticated registry-token request followed by a manifest request
  returned HTTP 200 with the same digest. No local Docker pull was performed.
  This verifies anonymous registry access independently of the separate GitHub
  package-metadata API, which rejected the current OAuth token for missing
  read:packages scope. No credential permissions were changed.
- The image contains pinned native runtime, model/font data, license texts,
  application license and NOTICE. Release tests run before publication. The
  workflow exports Buildx layer caches and does not upload a build-record artifact.
- This verifies Linux image operation on the cloud runner. It is not a NAS
  deployment, NAS memory benchmark or full upstream quality/parity acceptance.

## 2026-10-23 NAS deployment smoke

- Hardware: Intel Celeron J1900 at 1.99 GHz, approximately 8 GB RAM, Linux amd64;
  Docker 28.5.2 and Compose 2.40.3. Image manifest digest is the same verified
  cloud digest above; container image ID is
  `sha256:252debbe5d41cc62692a8f554c3271e17ece1d48f6520977d222ee07778b62a0`.
- Deployment is an independent Compose project, container `pdf2zh-rust`, UID/GID
  10001, private-interface port 7861 and named volume `pdf2zh-rust_pdf2zh-data`.
  Restart policy is `unless-stopped`. Host-specific runtime files are outside
  this repository; addresses, reports, samples and screenshots stay in ignored
  local artifacts. No Docker image was downloaded or built on Mac.
- Large-layer pull retries eventually succeeded without Docker daemon/network
  changes. The model and font passed the embedded hash checks on NAS.
- The first Google smoke job failed at the network stage. Container and host
  curl reproduced TLS timeouts. Independent proxy delay tests returned a 504
  for the existing default selector but HTTP 200/438 ms through the existing
  registry egress. A backed-up, validated rule scoped to
  `translate.googleapis.com` now selects that working egress. Hot reload
  preserved all prior selector values and the proxy container's start time;
  proxy mode, TUN and Docker daemon settings were not changed.
- A fresh synthetic one-page task then passed real native layout inference,
  live Google English→Chinese, side-by-side export, native PDF reopening/text
  extraction, PDF download and PNG preview. Task completion took 29.59 seconds.
  Output is 9,026,930 bytes and the preview is 20,997 bytes. The browser showed
  completed history and the bilingual page; the PNG was visually inspected.
  This was a warm-filesystem second attempt, not a cold benchmark. The first
  failed synthetic task remains in test history as diagnostic evidence.
- Host-side cgroup v2 sampling at approximately 100 ms (318 samples) recorded
  working set as `memory.current - inactive_file`: baseline 1.72 MiB, sampled
  peak 432.45 MiB, and 1.94 MiB after completion. Raw `memory.current` peak was
  432.84 MiB. Docker stats independently reported 1.934 MiB after the task and
  1.359 MiB after restart. The native analyzer and translation workers had exited;
  the idle container held docker-init and the Rust service only. The Python
  measurement script runs on the host, not as an application dependency.
- Restart verification ran only while all tasks were terminal. The new Rust
  container returned healthy; completed task history and the PDF SHA-256 survived
  on the named volume. No lifecycle operation targeted the old upstream service.
- Text-layer follow-up: native extraction of this visibly correct page returned
  the full translated sentence plus a duplicate trailing `变。` fragment and
  whitespace. The cause is not yet established. Exact full-paragraph extraction
  across all PDF consumers is therefore not claimed; keep this case for a later
  PDF text-layer regression fix. Existing Unicode-presence tests alone do not
  cover this defect.
- Private evidence: ignored `artifacts/nas-smoke-report.json`,
  `artifacts/nas-deployment-record.json`, `artifacts/nas-preview.png` and
  `artifacts/nas-deployed.jpg`; raw samples/restart record remain in the NAS
  deployment's smoke directory. This is one small fixture, not an upstream A/B
  comparison or a bound for larger/scanned documents.

## Remaining coverage

Full upstream configuration/provider parity, cross-page paragraphs, complete table text translation, OCR, broad multilingual shaping, exact text-layer roundtrips, font subsetting, production hardening and a controlled upstream comparison are not complete. The development preview and README state these limits explicitly.


## 2026-10-23 first-page HTTP 400 investigation

- Real NAS failure for a 47-page input was at `translate` page 1, zero completed blocks. Only the previous one-page smoke translation was in the shared cache. Full-document layout analysis had consumed the time and looked like nearly completed translation in the old UI.
- The old code discarded the HTTP response body and removed credentials at terminal state. User confirms a key was supplied. Therefore the exact original rejection cannot be reconstructed from retained history. Missing OpenCode session/client identity is a concrete compatibility defect and plausible cause, supported by [OpenCode documentation](https://dev.opencode.ai/v2/docs/console/go) and [official-repository report #47756](https://github.com/anomalyco/opencode/issues/47756); it is not claimed as confirmed against the user's key.
- Local regression checks cover structured/redacted/bounded provider diagnostics, permanent-vs-transient retry selection, application/session headers, a two-page native-layout task rejected on page 1, retained-file retry into a distinct completed history record, and private-settings deletion. Full local suite and cloud/NAS deployment verification recorded after execution below.
- Anonymous NAS diagnostic probes did not reproduce the authenticated failure (one generic client got edge HTTP 403; later honest-user-agent requests timed out). These are not evidence of an invalid user key or successful OpenCode integration. No credentials were read from unrelated applications or requested in chat.

- Fresh local verification: 12 tests passed (7 unit, 4 PDF, 1 process/HTTP lifecycle), release build succeeded; JS parsed, fmt and diff checks passed. The expanded lifecycle regression failed against restored HEAD engine/translator because the old client omitted required identity headers; restoring the patch and running the full suite passed. Logs of the intentional before-fix failure are ignored under artifacts.

- Cloud run [39916128410](https://github.com/jmjFortune/pdf2zh-rust/actions/runs/39916128410) succeeded: 12 Linux tests and final image resource/inference/HTTP/non-root checks passed. Published/pulled digest `sha256:d916e2f53f107e47e3b8cdb9b58e3d2ff1fc36355d2bb69699a0a644654aaa48`, source `80d499bf02bc1377c7813de3a110e4ed6674c9d5`.
- NAS idle-only upgrade succeeded, healthy. Three original history records compared equal, original input hashes unchanged, existing volume retained. NAS live native-worker regression using a clearly named synthetic PDF and LAN mock completed in 56.48 s: one non-retried 400 with sanitized diagnostic, same-input retry completed with downloadable PDF, original failure retained, credentials files removed. Two HTTP requests total; identity/session headers present. This validates infrastructure and behavior, not real OpenCode credentials/translation.
- Browser confirms the original user failure now displays translate page 1/47 and zero completed blocks alongside retained-file retry. Original task cannot recover its discarded provider body or Key; real OpenCode validation awaits the user's key entered in the page. Screenshot/report private under ignored artifacts and NAS diagnostic directory. The 84.14 MiB resource blob needed a resumed download; no proxy/daemon setting was changed during this repair.


## 2026-10-23 16:15–16:16 running-task liveness check

A user screenshot showed page 14/47, 235 completed blocks. Read-only NAS sampling showed the same task at 244 blocks, then page 15/249 blocks and page 17/253 blocks across 20 seconds, running without error. Worker remained alive, low CPU, approximately 77.63 MiB Docker memory, and active OpenCode traffic. Sanitized current-task configuration confirmed OpenAI-compatible OpenCode / DeepSeek; the browser form's Google default was not its running-task configuration. No task, service, provider, proxy or data was modified. This confirms continued progress at that time, not completion of the document or proof that the user's Edge event stream never disconnected.
The NAS SSE endpoint also returned HTTP 200 / text/event-stream with a live job event at page 17/47, 257 completed blocks, running without error. This verifies the server event endpoint, not the particular Edge connection.

## 2026-10-23 missing U+2023 repair and offline recovery

- The real 47-page task failed at page 32 with 510 rendered blocks. NAS cache backup contained 512 rows, including the current failed translation and previous smoke entries. Two cached values needed fallback; all visible characters in those values were covered by GoNotoKurrent. Its local bytes matched the public BabelDOC-Assets Git blob, and the asset URL/SHA3 and OFL text/SHA256 are pinned.
- New regression uses mixed Chinese/U+2023 alongside an original Greek inline formula in real worker processes, plus a plain paragraph with the same bullet. Temporarily restoring the old engine made that lifecycle test fail with the exact original `output font lacks character U+2023`; restore repair and run the full suite before publishing. A separate mock counter test confirms cache-only reuse and rejection of misses without extra HTTP calls.
- Private offline recovery on the user's local machine regenerated pages 1–31 from existing cache using the release binary, with network translation disabled. It rendered 498 translated blocks and saved a 47-page PDF: pages 1–31 translated, pages 32–47 original English. Page 1 and page 31 were rendered and visually checked; this is a recovered partial document, not a full translation or an upstream-quality guarantee.
- The first debug-binary recovery attempt triggered a native MuPDF ActualText assertion while reaching page 23 and produced no output. The release recovery completed. This debug/native assertion has not been fixed by the font repair and remains a separate known issue; original NAS input/cache were backed up before recovery and unchanged.
- Cloud build/deployment verification follows below; live OpenCode requests and remaining pages still require the user's task credentials to be re-entered because old terminal configuration was removed.

## 2026-10-23 — translator frontend deployed on NAS

- Source `de39b82a3f886f7cc1b1a47506f72c929e10584f`; final image `ghcr.io/jmjfortune/pdf2zh-rust@sha256:d0d850c12286617b0369639fb828b557b7e0063fbe78b65c803a4e125c454d79`. [Cloud run](https://github.com/jmjFortune/pdf2zh-rust/actions/runs/39951931277) passed the static build, all 15 Linux native tests and final-image resource/inference/HTTP/nonroot checks.
- Local production frontend build passed (101 kB initial JS); browser upload, task refresh, library, local download and settings/tab restore checked. Actual layout analysis was observed as `Analyzing` with an indeterminate accessible progress indicator. Styles, layout, Sidebar and Navbar matched the original translator source.
- NAS is healthy at port 7861. Original named volume retained. Both upgrades compared every original input SHA-256 and all native job records: six original tasks plus two new diagnostic tasks remain intact. User explicitly confirmed manually clearing the Library; preserve those hidden flags. Files and cache remain recoverable.
- Browser created a real NAS Google task: one-page input with `first3` selection succeeded, producing mono, interleaved and side-by-side PDFs in one translation pass. Direct HTTP read all four original/generated downloads successfully; the three generated PDFs were ~9 MB each and loaded through MuPDF. The retained original smoke task's known side-by-side metadata was restored from its recorded creation request and confirmed double page width.
- Actual user page 32 cache-only verification processed 13 translated blocks including the former U+2023 failure, then deliberately stopped on uncached content without provider requests. A local mock on NAS completed a separate U+2023 PDF; all 512 previous cache values were unchanged (513 rows after that diagnostic). This does not mean the user's full 47-page PDF completed. The earlier recovered first-31-page PDF remains available.
- Docker working-set idle samples: 6.984 MiB after tests on the first UI image, 1.879 MiB on the fresh final container. These are idle measurements, not translation/model peak memory. The container runs docker-init plus Rust only, with no Node or Python application process.
- Browser limitation: the in-app browser displayed blank PDF panels for both this API and an independent standard HTTP PDF control. Direct PDFs were valid and streamed successfully; local browser download succeeded, while the NAS browser download event timed out. Ordinary Edge PDF preview could not be visually verified because native computer-control permissions were unavailable. Do not claim successful PDF rendering inside the in-app browser.
- Optional strict Clippy check still reports the existing `collapsible_if` (engine rich-text merge) and `too_many_arguments` (PDF helper) lints. Required builds and tests pass; unrelated lint refactoring was left out.

## 2026-10-23 — provider catalogue, About removal and Dual naming

- Source `adc81bf6d255273c3a4480ed945bbadbab674005`; [cloud run 39955888217](https://github.com/jmjFortune/pdf2zh-rust/actions/runs/39955888217) passed all 17 Linux tests (11 unit, 4 PDF, 1 lifecycle, 1 UI adapter), the frontend production build and final-image resource/inference/HTTP/nonroot checks. NAS pinned image `ghcr.io/jmjfortune/pdf2zh-rust@sha256:ac1a3f3fa8641811c3a07f5be28007d6eb8582d1a503fd5587a9787af95c8de7` is healthy.
- Local HTTP protocol matrix covers 19 non-Google profiles, provider-specific request paths, bodies, private credentials, optional Xinference authentication and Azure options; Google retains the previous live NAS check. This is not a claim of live access to 19 paid accounts. Required model/URL validation rejects incomplete profiles before PDF analysis; unknown saved options are rejected with HTTP 400.
- Local browser verified Azure region save/reload, Azure OpenAI deployment/API-version controls, legacy About tab fallback and twenty service choices. A local mock completed all three exports; clicking Export Dual downloaded a PDF with 1190 pt page width against a 595 pt original. Download names distinguish `-dual.pdf` and `-interleaved.pdf`; old persisted API format IDs keep their original meaning.
- Idle-only NAS update preserved the named volume, all eight task records and every input SHA256. Private settings file SHA256 is identical before/after; user-hidden Library remains empty. No paid requests or original-document reruns were triggered by this update. About-only XPI URL returns 404.
- NAS browser confirmed all twenty buttons, no About navigation and Dual/Interleaved/Mono modes. Desktop layout checked at 1280×1100, temporary viewport reset afterward. Private screenshot and filtered reports are in ignored artifacts. Owned local mock/server and test download cleaned up after verification. Existing narrow-panel/PDF-viewer limitations remain as previously documented.

## 2026-10-24 — inline math, code preservation and controlled reader

- Actual completed user PDF (47 pages, 764 translated blocks) reproduces overlap at page 22. Native extraction split the same prose paragraph into overlapping blocks around stacked fractions and scripts, although the detector recognized one text paragraph. Combined paragraph ownership and prose-baseline grouping correct that geometry; exact glyph capture excludes neighboring lines. Covered-path-only redaction moves fraction rules while keeping crossing frames/images/text intact.
- Original two new paragraph/code regressions failed against the old behavior, then passed after repair. Latest release suite passed all **23 tests**: 15 unit, 6 PDF, 1 process lifecycle, 1 UI adapter. Coverage includes broad detector regions crossing standalone code, ligature coordinate roundoff, exact fragment ownership under rotation and fraction-rule/frame pixel preservation. `cargo fmt --check` and `git diff --check` passed.
- Private local geometry-only HTTP mock regenerated selected source pages 5, 15, 22 (28 units); no external translation account was contacted. Page 22 was rendered and visually inspected with its fourteen inline fragments and original display equation. Synthetic mock Chinese is deliberately labeled layout validation; it is not a newly translated user document. Seven pure monospaced code regions on actual page 15 compared pixel-identical before/after. Existing completed NAS output is retained and requires regeneration to receive engine fixes.
- Static frontend production build passed (103 kB initial JS; PDF.js loads lazily). Mozilla PDF.js **6.4.299** worker/resources/license are copied from the pinned npm package, not fetched from a CDN at runtime. JavaScript modules and WebAssembly MIME responses checked. No Node/Python runtime added to the service.
- Browser renders the real completed PDF with selectable text and no iframes. At 200% zoom, Right/Down changed scroll coordinates (0,0)→(64,64); Left/Up returned to (0,0). Full-window mode/exit, Fit width and direct page-22 navigation passed. The old plugin-specific blank panel is avoided locally; NAS rendering verification is pending connectivity. Arrow keys belong to the focusable reading area and do not intercept toolbar inputs.
- On continuation, workspace had moved to Desktop/projects. Local test process paths refreshed. NAS ZeroTier HTTP/SSH and former LAN HTTP/SSH probes timed out; no network configuration or production data changed. Cloud build and any eventual deployment outcome follow below.

- [Cloud run 40086728911](https://github.com/jmjFortune/pdf2zh-rust/actions/runs/40086728911) succeeded for source `2d0ff611fee1e348515d94b35c24b13ea8ec9931`: the same 23 Linux tests, static frontend build, final resource/inference/nonroot/HTTP checks and worker-module/WebAssembly MIME checks passed. Published digest `sha256:1af635e9b07c37bce6424e8b64a6010fb6ab5fd2bc0b94ec023224da6ede208e`.
- NAS was reachable via the existing local HTTP proxy (including SSH through HTTP CONNECT); preflight found ten retained terminal tasks, none active. Docker pull started on NAS and several layers completed, then SSH disconnected. Subsequent direct, proxy and old LAN probes failed. No compose upgrade has been run yet; image presence/completeness and idle state must be rechecked on reconnection. Do not call this revision deployed based on the cloud build alone.

- Deployment completed after connectivity recovered: NAS image revision `2d0ff611fee1e348515d94b35c24b13ea8ec9931`, digest `sha256:1af635e9b07c37bce6424e8b64a6010fb6ab5fd2bc0b94ec023224da6ede208e`, healthy. Detached upgrade process preserved all ten terminal task records, all input hashes, the original named data volume and exact private settings-file hash. No old output was overwritten and no paid provider was called.
- Live same-origin HTTP checks on NAS compared PDF.js worker, OpenJPEG WASM and the reader application chunk to the verified local production build; every SHA256 matched and JavaScript/WASM MIME types were correct. Browser tools became unavailable during the continuation, so post-deployment checks use these exact resource hashes and prior actual local UI tests, not a claim of a fresh visual NAS browser test. Local diagnostic processes were cleaned up or already stopped after interruption.
