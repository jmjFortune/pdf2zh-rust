"use client";

import React, { useEffect, useRef, useState } from "react";

let library;
function pdfLibrary() {
  library ||= import("pdfjs-dist/build/pdf.mjs").then((pdfjs) => {
    pdfjs.GlobalWorkerOptions.workerSrc = "/pdfjs/pdf.worker.min.mjs";
    return pdfjs;
  });
  return library;
}

export default function PdfPage({ url, pageNumber, width, label }) {
  const [document, setDocument] = useState(null);
  const [error, setError] = useState("");
  useEffect(() => {
    let active = true;
    let loading;
    setDocument(null); setError("");
    pdfLibrary().then((pdfjs) => {
      if (!active) return;
      loading = pdfjs.getDocument({ url, cMapUrl: "/pdfjs/cmaps/", cMapPacked: true,
        standardFontDataUrl: "/pdfjs/standard_fonts/", wasmUrl: "/pdfjs/wasm/", isEvalSupported: false });
      return loading.promise;
    }).then((pdf) => { if (active && pdf) setDocument(pdf); })
      .catch((err) => { if (active) setError(err.message); });
    return () => { active = false; loading?.destroy(); };
  }, [url]);
  if (error) return <p role="alert" className="p-6 text-sm text-red-700">Unable to display {label}: {error}. <a href={url} target="_blank" rel="noreferrer" className="underline">Open PDF</a></p>;
  if (!document) return <p role="status" className="p-6 text-sm text-[#6E6961]">Loading {label}…</p>;
  return <PdfSurface key={`${pageNumber}-${Math.round(width)}`} document={document} pageNumber={pageNumber} width={width} label={label} />;
}

function PdfSurface({ document, pageNumber, width, label }) {
  const surface = useRef(null);
  const [error, setError] = useState("");
  const [ready, setReady] = useState(false);
  useEffect(() => {
    let active = true;
    let render;
    let textLayer;
    const host = surface.current;
    (async () => {
      const pdfjs = await pdfLibrary();
      const page = await document.getPage(pageNumber);
      if (!active) return;
      const scale = width / page.getViewport({ scale: 1 }).width;
      const viewport = page.getViewport({ scale });
      const ratio = Math.min(window.devicePixelRatio || 1, 2, Math.sqrt(16000000 / (viewport.width * viewport.height)));
      const canvas = window.document.createElement("canvas");
      canvas.width = Math.ceil(viewport.width * ratio);
      canvas.height = Math.ceil(viewport.height * ratio);
      canvas.style.width = `${viewport.width}px`;
      canvas.style.height = `${viewport.height}px`;
      canvas.setAttribute("aria-label", `${label}, page ${pageNumber}`);
      const text = window.document.createElement("div");
      text.className = "textLayer";
      host.style.width = `${viewport.width}px`;
      host.style.height = `${viewport.height}px`;
      host.style.setProperty("--total-scale-factor", scale);
      host.replaceChildren(canvas, text);
      render = page.render({ canvasContext: canvas.getContext("2d"), viewport,
        transform: ratio === 1 ? undefined : [ratio, 0, 0, ratio, 0, 0] });
      await render.promise;
      if (!active) return;
      textLayer = new pdfjs.TextLayer({ textContentSource: page.streamTextContent(), container: text, viewport });
      await textLayer.render();
      if (active) setReady(true);
    })().catch((err) => { if (active && err.name !== "RenderingCancelledException") setError(err.message); });
    return () => { active = false; render?.cancel(); textLayer?.cancel(); };
  }, [document, pageNumber, width, label]);
  return <div className="relative shrink-0" style={{ width }}>
    {!ready && !error && <p role="status" className="p-3 text-sm text-[#6E6961]">Rendering {label}…</p>}
    {error && <p role="alert" className="p-3 text-sm text-red-700">{error}</p>}
    <div ref={surface} className="pdf-page relative bg-white shadow-sm" data-rendered={ready ? "true" : "false"} />
  </div>;
}
