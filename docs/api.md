# HTTP API and configuration

The independent service defaults to `127.0.0.1:7861`. These endpoints are not claimed to be a Gradio API compatibility layer.

| Method | Path | Behavior |
| --- | --- | --- |
| GET | `/api/health` | Service health and engine version |
| POST | `/api/jobs` | Multipart upload, fields `file` and optional JSON `options` |
| GET | `/api/jobs` | Latest 100 public task records |
| GET | `/api/jobs/{id}` | Public state, progress, error and output page count |
| GET | `/api/jobs/{id}/events` | SSE `job` events until terminal state |
| POST | `/api/jobs/{id}/retry` | Create a new task using retained input and explicitly supplied JSON upload options; only failed/cancelled/interrupted tasks |
| POST | `/api/jobs/{id}/cancel` | Cancel queued work, or signal/terminate active worker |
| GET | `/api/jobs/{id}/input` | Stream original PDF |
| GET | `/api/jobs/{id}/output` | Stream completed PDF |
| GET | `/api/jobs/{id}/preview/{page}` | 1-based PNG preview; missing pages render in a separate bounded worker |

States: `queued`, `running`, `cancelling`, `completed`, `failed`, `cancelled`, `interrupted`. A restart marks previously active tasks interrupted. It does not claim they completed or automatically resubmit their content.

Example upload options:

```json
{
  "translation": {
    "provider": "openai",
    "source": "en",
    "target": "zh-CN",
    "base_url": "http://127.0.0.1:8080/v1",
    "model": "your-model",
    "api_key": null,
    "glossary": {"neural network": "神经网络"},
    "requests_per_second": 2,
    "options": {}
  },
  "pages": "1,3-5",
  "format": "side_by_side",
  "layout": true,
  "password": null
}
```

Formats: `mono`, `interleaved`, `side_by_side`. CLI `--format` uses `side-by-side`. Upload filenames are display labels; input/output paths are assigned by the service. Uploads are streamed, with a 190 MiB PDF limit. There is one translation worker at a time and one additional on-demand preview renderer at a time.

A CLI settings file accepts `JobConfig`: the same `translation`, `pages`, `format`, `layout` and `password` fields, plus optional `min_font_size`, `data_dir`, `cache_path`. CLI positional input/output paths take precedence; other CLI flags only override explicitly supplied values. `PDF2ZH_API_KEY` can supply the API key without a command-line argument.

Provider URL conventions:

- `google`: no endpoint or key required; public web-compatible protocol, subject to availability and rate limits.
- `bing`: no key required; optional `base_url` selects its translator page. Native adapter refreshes the public session and bounds request chunks without truncating source text.
- `openai`: `base_url` ends at the API version root such as `/v1`; `model` required. Compatible providers can use this path.
- `ollama`: root defaults to `http://127.0.0.1:11434`; `model` required.
- `deepl`: `base_url` optionally overrides the `/v2` API root; the key selects free versus paid default endpoint.
- `deeplx`: `base_url` is the service root before `/translate`.
- `azure`: `base_url` is the Translator endpoint; `options.region` defaults to `chinaeast2`.
- `azureopenai`: `base_url` is the Azure resource root, `model` is the deployment; `options.api_version` defaults to `2024-10-21`.
- `dify` and `anythingllm`: `base_url` is the complete workflow/chat endpoint.

Provider adapters beyond live Google and the local OpenAI contract fixture remain unverified against real credentials. They are not marked production-ready.


## Provider failures and phase progress

The first selected page is analyzed and translated before analyzing the remaining pages. Model processes still exit before translation HTTP waits. Layout page counters describe analysis only, not completed translations; the browser uses an indeterminate indicator in that stage and displays the number of completed translated blocks separately.

HTTP errors retain bounded structured `error.type`, `error.code` and `error.message` diagnostics (maximum 16 KiB input, 600 diagnostic characters). Supplied API keys, Bearer/sk tokens and exact submitted text are redacted; raw HTML/error-body dumps are omitted. HTTP 400/401/403 and other permanent statuses are not blindly retried. 408, 429 and 5xx retain bounded retries. Provider-specific error messages may still contain provider-supplied context; never deliberately return raw request dumps.

OpenAI-compatible requests identify this application honestly as `pdf2zh-rust/<version>` and send a stable random `x-opencode-session` for the current worker's translation session, as required by [OpenCode Go's client documentation](https://dev.opencode.ai/v2/docs/console/go#where-can-i-use-it). This does not impersonate a coding client or guarantee Go accepts PDF translation traffic.

Retry JSON uses the same fields as upload `options`. It creates a new UUID/history record and hard-links (or copies) the retained immutable input; the original failure stays intact. The caller must explicitly provide all desired settings again, including API key/password when needed. The web button says it uses the current form settings. No key is restored from history, stored in browser persistence, or returned by this API. Valid per-paragraph translation cache entries remain reusable when all cache-key settings match. This is a cached re-run, not a saved partially translated PDF or an automatic resume at the previous page.

## Fonts and cache-only recovery

Worker output keeps the primary CJK font for paragraphs it covers, and lazily
selects GoNotoKurrent for a paragraph containing unsupported characters. The same
selected font bytes are used for measurement, fitting and writing, for both plain
text and text around protected formulas. Formula placeholders are not visible
font characters. Unsupported visible characters in both fonts still produce an
explicit error instead of silently dropping text or changing Unicode.

`PDF2ZH_CACHE_ONLY=1` on a CLI/worker process permits validated cache hits and
rejects a missing cache entry before any provider request. It does not change
cache keys, providers, or translate missing content. Use it for offline recovery;
do not set it globally on a service intended to translate new content.

## Translator frontend adapter (`/api/ui`)

The static Next.js export is served by Rust. Node is used only during build;
there is no Node/Python web server in the image. The original `/api/jobs` API
remains available with its existing status names and named SSE events.

- `GET /health`, `GET/PUT /settings`: defaults and supported provider profiles.
  Keys may be supplied explicitly by `PUT`; responses contain `has_key` only.
  Omitted keys are retained, an explicit empty key removes the saved key.
  Settings/glossary are atomically saved to private `ui-settings.json` (0600).
- `GET /glossary`, `POST/DELETE /glossary/terms`: `{term, zh}` or `?term=...`.
- `GET/POST /jobs`, `GET /jobs/active`, `GET /jobs/{id}`: frontend job shape,
  with `queued/running/done/error/cancelled`, `progress.current/total`, actual
  `stage`, `options`, `files`, filename/size/pages/UTC timestamps.
- `POST /jobs/{id}/retry` reuses the retained input in a new task. Upload and
  retry accept multipart fields `service`, `lang_in/out`, `page_range`,
  `output_formats` (comma-separated mono/dual/side_by_side); upload also needs
  `file`. Credentials come from explicitly saved settings; terminal history
  keeps only allowlisted public metadata in `settings.json`. No old key is
  recovered from a failed job. Legacy records whose settings were discarded
  expose `options: null`; the UI explicitly asks the user to choose settings.
- `POST /jobs/{id}/cancel`, `GET /jobs/{id}/events`: unnamed SSE JSON includes
  monotonically increasing `seq`, actual stage, whole current job and terminal
  type. Reconnecting receives a current snapshot, never starts another job.
- `GET /jobs/{id}/download/{original|mono|dual|side_by_side}`: streamed PDF,
  attachment by default; `?inline=true` supports embedded readers.
- `GET /papers`, `GET /papers/{id}`, `GET /papers/{id}/content`: all retained
  task statuses, metadata, and `has_content:false` for the existing PDF reader
  fallback. `DELETE /papers/{id}` hides a stopped task from the library; PDF
  files/cache are retained. Clear its ID from private settings `hidden` to restore.

Multiple selected formats share one translation pass, then export separately.
All original pages remain in each export; `first3` translates at most three
pages, safely including a shorter PDF. Interleaved files have twice the original
page count. Original page numbering is retained in the PDF reader. Page-based
progress is completed source pages, with the current page/block count shown in
the message; it is not a percentage of elapsed time or paragraphs. During layout
analysis the UI uses an indeterminate bar and the label `Analyzing`; an analyzer
page number must never be presented as completed translation percentage.

This adapter does not implement the Python backend's Zotero connector endpoints,
OCR, or structured paragraph/formula reader API. These remain
limits. No existing translator credential file is imported automatically.

## Provider profiles and UI terminology (2026-10-23)

The UI catalogue has twenty independently configured profiles. Existing six IDs
remain valid; saved credentials, defaults and hidden Library records are kept.
`src/service/profiles.json` maps each profile to a real protocol, not merely a
button. Google, Ollama, DeepL, DeepLX, Azure OpenAI, Azure Translator, Dify and
AnythingLLM use their existing native adapters. OpenAI Compatible, OpenCode Go,
DeepSeek, Gemini, Zhipu, SiliconFlow, ModelScope, Qwen / Model Studio, Tencent
Hunyuan, Groq, Grok / xAI and Xinference use the OpenAI-compatible adapter.
Tencent Hunyuan does not claim support for Tencent Machine Translation. Bing's
previous HTTP-401 endpoint remains unavailable in the UI.

New model-provider profiles require the user to select a model available to their
account. Qwen / Model Studio also needs its workspace endpoint. Azure OpenAI has
an API-version option and deployment-name field; Azure Translator has a region
option. These options are allowlisted, non-secret and included in the translation
cache key. Unknown options are rejected; keys are never returned. Optional
Xinference authentication does not fall back to the CLI's global key.

The product label **Dual** now means a parallel original/translation PDF.
**Interleaved** means alternating source/translation pages. Persisted/API keys
remain `side_by_side` and `dual` respectively for compatibility. Download names
use `-dual.pdf` for the parallel PDF and `-interleaved.pdf` for alternating pages.
The Reader's Export Dual button prefers the parallel file; if only alternating
pages exist, the label is Export Interleaved. About, its navigation entry and its
exclusive plugin download are removed. A legacy About tab falls back to Translate.

Endpoint references checked against the providers' primary documentation:
[Gemini](https://ai.google.dev/gemini-api/docs/openai),
[DeepSeek](https://api-docs.deepseek.com/),
[Zhipu](https://docs.bigmodel.cn/cn/guide/develop/openai/introduction),
[SiliconFlow](https://docs.siliconflow.cn/docs/userguide/quickstart),
[Groq](https://console.groq.com/docs/openai),
[xAI](https://docs.x.ai/developers/rest-api-reference/inference/chat-completions),
[Qwen / Model Studio](https://www.alibabacloud.com/help/en/model-studio/compatibility-of-openai-with-dashscope),
[Tencent Hunyuan](https://cloud.tencent.com/document/product/1729/111007),
[Xinference](https://inference.readthedocs.io/en/latest/user_guide/client_api.html),
[ModelScope](https://modelscope.cn/docs/model-service/API-Inference/intro).
Mock protocol/authentication tests do not imply paid live account verification.

## Native paragraph repair and controlled PDF reader (2026-10-24)

Intersecting extraction blocks and pieces of the same detected paragraph are typeset together; prose baselines keep stacked fractions and scripts on their visual rows. Original math and inline monospaced code are captured by exact glyph ownership, with covered fraction-vector paths moved alongside them. Whole monospaced code blocks retain their original line breaks/indentation and bypass translation; prose cannot be regrouped across them. This font-based protection does not claim universal code recognition for PDFs with indistinguishable prose/code fonts.

The reader uses pinned Mozilla PDF.js in the browser, with same-origin worker/resources prepared during the static build. It fills available space and provides 25–300% zoom, Fit width, page-number navigation and a full-window mode. Click the reading area then use arrow keys to pan (Shift uses a larger step); Esc leaves full-window mode. Text remains selectable; original/generated PDF downloads remain unchanged. No new backend endpoint or application runtime is introduced. Existing generated PDFs need regeneration to gain engine-layout corrections; a reader update alone does not rewrite them.

Primary renderer reference: [Mozilla PDF.js examples](https://mozilla.github.io/pdf.js/examples/).
