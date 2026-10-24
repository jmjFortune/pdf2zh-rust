"use client";

import React, { useEffect, useRef, useState } from "react";
import PdfPage from "./PdfPage";

export default function PdfViewport({ documents, zoom, page }) {
  const viewport = useRef(null);
  const [available, setAvailable] = useState(800);
  useEffect(() => {
    const element = viewport.current;
    const observer = new ResizeObserver(([entry]) => setAvailable(Math.max(1, entry.contentRect.width)));
    observer.observe(element);
    return () => observer.disconnect();
  }, []);
  useEffect(() => { viewport.current.scrollTo({ top: 0, left: 0 }); }, [page, documents.length]);
  const width = Math.max(100, (available - 32 - (documents.length - 1) * 16) / documents.length) * zoom / 100;
  const pan = (event) => {
    if (event.altKey || event.metaKey || event.ctrlKey || event.target.closest("input, textarea, select, button, a, [contenteditable=true]")) return;
    const step = event.shiftKey ? 160 : 64;
    const direction = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] }[event.key];
    if (!direction) return;
    event.preventDefault();
    event.currentTarget.scrollBy({ left: direction[0], top: direction[1], behavior: "instant" });
  };
  return <div ref={viewport} role="region" aria-label="PDF reading area" tabIndex={0}
    onPointerDown={(event) => { if (!event.target.closest("button, a")) event.currentTarget.focus({ preventScroll: true }); }}
    onKeyDown={pan} aria-describedby="pdf-pan-help"
    className="flex-1 min-h-0 min-w-0 overflow-auto focus-visible:outline focus-visible:outline-2 focus-visible:outline-[#D97757] focus-visible:outline-offset-[-2px]">
    <div className="flex items-start gap-4 p-4 mx-auto" style={{ width: width * documents.length + 32 + (documents.length - 1) * 16 }}>
      {documents.map((document) => <PdfPage key={document.kind} url={document.url} label={document.label} pageNumber={document.page} width={width} />)}
    </div>
  </div>;
}
