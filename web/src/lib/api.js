// Same-origin URLs work through Next's proxy and the FastAPI static export.
export async function request(path, options = {}) {
  const response = await fetch(`/api/ui${path}`, {
    cache: "no-store",
    ...options,
    headers: {
      ...(options.body && !(options.body instanceof FormData) ? { "Content-Type": "application/json" } : {}),
      ...options.headers,
    },
  });
  if (!response.ok) {
    const body = await response.json().catch(() => null);
    const detail = body?.detail || body?.error;
    const message = typeof detail === "string" ? detail : Array.isArray(detail)
      ? detail.map((item) => item.msg).join("; ") : `Request failed (${response.status})`;
    const error = new Error(message);
    error.status = response.status;
    throw error;
  }
  return response.json();
}

export const api = {
  health: () => request("/health"),
  settings: () => request("/settings"),
  saveSettings: (patch) => request("/settings", { method: "PUT", body: JSON.stringify(patch) }),
  glossary: () => request("/glossary"),
  addTerm: (term, zh) => request("/glossary/terms", { method: "POST", body: JSON.stringify({ term, zh }) }),
  deleteTerm: (term) => request(`/glossary/terms?term=${encodeURIComponent(term)}`, { method: "DELETE" }),
  papers: () => request("/papers"),
  paper: (id) => request(`/papers/${encodeURIComponent(id)}`),
  content: (id) => request(`/papers/${encodeURIComponent(id)}/content`),
  deletePaper: (id) => request(`/papers/${encodeURIComponent(id)}`, { method: "DELETE" }),
  createJob: (body) => request("/jobs", { method: "POST", body }),
  activeJob: () => request("/jobs/active"),
  retryJob: (id, body) => request(`/jobs/${encodeURIComponent(id)}/retry`, { method: "POST", body }),
  job: (id) => request(`/jobs/${encodeURIComponent(id)}`),
  cancelJob: (id) => request(`/jobs/${encodeURIComponent(id)}/cancel`, { method: "POST" }),
};

export const terminal = (status) => ["done", "error", "cancelled"].includes(status);

export function previewUrl(job, kind, page = 1) {
  const url = kind === "original"
    ? `/api/ui/jobs/${encodeURIComponent(job.id)}/download/original`
    : job.files[kind];
  return url ? `${url}?inline=true#page=${page}&zoom=page-width&toolbar=0&navpanes=0` : null;
}

export function translatedPages(job) {
  const count = job.pages || 0;
  // Rust retains all original pages in each export.
  return Array.from({ length: count }, (_, i) => i + 1);
}
