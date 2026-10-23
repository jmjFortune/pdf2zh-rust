# HTTP API and configuration

The independent service defaults to `127.0.0.1:7861`. These endpoints are not claimed to be a Gradio API compatibility layer.

| Method | Path | Behavior |
| --- | --- | --- |
| GET | `/api/health` | Service health and engine version |
| POST | `/api/jobs` | Multipart upload, fields `file` and optional JSON `options` |
| GET | `/api/jobs` | Latest 100 public task records |
| GET | `/api/jobs/{id}` | Public state, progress, error and output page count |
| GET | `/api/jobs/{id}/events` | SSE `job` events until terminal state |
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
