"use client";

import React, { useState, useEffect } from "react";
import {
  Download,
  ZoomIn,
  ZoomOut,
  ChevronLeft,
  ChevronRight,
  Copy,
  Check,
  Info
} from "lucide-react";
import { api, previewUrl, translatedPages } from "../lib/api";

export default function ReaderView({ paperId, onSelectLibrary }) {
  const [currentPage, setCurrentPage] = useState(1);
  const [zoom, setZoom] = useState(100);
  const [viewMode, setViewMode] = useState("split");
  const [hoveredParagraph, setHoveredParagraph] = useState(null);
  const [activeFormula, setActiveFormula] = useState(null);
  const [selectedTerm, setSelectedTerm] = useState(null);
  const [copiedFormula, setCopiedFormula] = useState(false);
  const [showTooltip, setShowTooltip] = useState(false);

  const [data, setData] = useState(null);
  const [error, setError] = useState("");
  const [glossary, setGlossary] = useState([]);

  useEffect(() => {
    if (!paperId) return;
    let active = true;
    setData(null); setError(""); setCurrentPage(1);
    Promise.all([api.paper(paperId), api.content(paperId), api.job(paperId), api.glossary()])
      .then(([metadata, content, job, terms]) => {
        if (!active) return;
        const pages = translatedPages(job);
        setData({ job, pages, paper: { ...content, totalPages: content.has_content ? content.totalPages : pages.length,
          fileSize: metadata.size, modelUsed: metadata.service } });
        setGlossary(Object.entries(terms.terms).map(([term, zh]) => ({ term, zh })));
      }).catch((err) => { if (active) setError(err.message); });
    return () => { active = false; };
  }, [paperId]);

  const paper = data?.paper;
  const job = data?.job;
  const spreadOnly = job?.files.side_by_side && !job.files.mono && !job.files.dual;
  const originalPage = data?.pages[currentPage - 1] || currentPage;
  const translationKind = job?.files.mono ? "mono" : "dual";
  const translationPage = translationKind === "dual" ? currentPage * 2 : currentPage;
  const dualDownload = job?.files.dual || job?.files.side_by_side;

  if (!paperId || error || !paper) {
    return <div className="max-w-3xl mx-auto space-y-6 py-4">
      <h1 className="font-serif text-3xl font-bold text-[#1F1E1D]">Reader</h1>
      <p role={error ? "alert" : "status"} className="text-sm text-[#78716A]">{error || (paperId ? "Loading paper…" : "Choose a translated paper from the Library to start reading.")}</p>
      <button onClick={onSelectLibrary} className="px-5 py-2.5 rounded-xl bg-[#D97757] text-white text-xs font-medium">Open Library</button>
    </div>;
  }

  const handleCopyFormula = (latex) => {
    navigator.clipboard.writeText(latex);
    setCopiedFormula(true);
    setTimeout(() => setCopiedFormula(false), 2000);
  };

  return (
    <div className="flex flex-col h-[calc(100vh-3.5rem)] bg-[#F5F2EB] -m-6 lg:-m-8 overflow-hidden">
      {/* Reader Top Action Bar */}
      <div className="relative z-20 h-14 bg-white border-b border-[#E8E4DA] px-6 lg:px-8 flex items-center justify-between shrink-0 gap-4">
        {/* Document Info with Tooltip */}
        <div
          className="relative min-w-0 shrink"
          onMouseEnter={() => setShowTooltip(true)}
          onMouseLeave={() => setShowTooltip(false)}
        >
          <button
            type="button"
            onClick={() => setShowTooltip((v) => !v)}
            className="flex items-center space-x-2 py-1 px-2.5 -mx-2.5 rounded-xl cursor-pointer hover:bg-[#FAF9F5] border border-transparent hover:border-[#E8E4DA] transition-all text-left group"
            title="点击或悬浮查看完整文献详情"
          >
            <span className="font-serif font-bold text-sm text-[#1F1E1D] whitespace-nowrap truncate max-w-[180px] lg:max-w-xs group-hover:text-[#D97757] transition-colors">
              {paper.title}
            </span>
            <span className="text-xs text-[#78716A] whitespace-nowrap hidden md:inline truncate">
              {paper.conference}
            </span>
            <Info className="w-3.5 h-3.5 text-[#A8A29E] group-hover:text-[#D97757] shrink-0 transition-colors" />
          </button>

          {/* Floating Tooltip Card */}
          <div
            className={`absolute left-0 top-full mt-2 w-80 sm:w-96 p-4 bg-white/95 backdrop-blur-xs border border-[#E2DBD0] rounded-2xl shadow-xl transition-all duration-200 z-50 ${
              showTooltip
                ? "opacity-100 translate-y-0 pointer-events-auto"
                : "opacity-0 translate-y-1.5 pointer-events-none"
            }`}
          >
            {/* Arrow Pointer */}
            <div className="absolute -top-1.5 left-6 w-3 h-3 bg-white border-t border-l border-[#E2DBD0] rotate-45" />

            <div className="space-y-3 relative">
              <div>
                <span className="inline-block px-2 py-0.5 text-[10px] font-medium bg-[#F0EEE6] text-[#6E6961] rounded-md mb-1.5">
                  {paper.conference}
                </span>
                <h4 className="font-serif font-bold text-sm text-[#1F1E1D] leading-snug">
                  {paper.title}
                </h4>
                {paper.titleZh && (
                  <p className="font-sans text-xs text-[#6E6961] mt-1 font-medium">
                    {paper.titleZh}
                  </p>
                )}
              </div>

              <div className="pt-2 border-t border-[#F0EEE6] text-[11px] space-y-1.5 text-[#78716A]">
                {paper.authors && (
                  <p className="line-clamp-2">
                    <span className="font-medium text-[#1F1E1D]">作者：</span>
                    {paper.authors}
                  </p>
                )}
                <div className="flex items-center justify-between text-[#8C827A] pt-0.5">
                  <span>共 {paper.totalPages} 页 · {paper.fileSize}</span>
                  {paper.modelUsed && (
                    <span className="text-[#D97757] font-medium">
                      {paper.modelUsed} 翻译
                    </span>
                  )}
                </div>
              </div>
            </div>
          </div>
        </div>

        {/* Center Controls */}
        <div className="flex items-center space-x-3 lg:space-x-4 shrink-0">
          {/* Page Switcher */}
          <div className="flex items-center bg-[#FAF9F5] border border-[#E2DBD0] rounded-xl px-2.5 py-1 text-xs whitespace-nowrap shrink-0">
            <button
              disabled={currentPage <= 1}
              onClick={() => setCurrentPage((p) => Math.max(1, p - 1))}
              className="p-1 text-[#6E6961] hover:text-[#1F1E1D] disabled:opacity-30"
            >
              <ChevronLeft className="w-3.5 h-3.5" />
            </button>
            <span className="mx-2 font-mono text-[#1F1E1D] whitespace-nowrap select-none">
              {currentPage} / {paper.totalPages}
            </span>
            <button
              disabled={currentPage >= paper.totalPages}
              onClick={() => setCurrentPage((p) => Math.min(paper.totalPages, p + 1))}
              className="p-1 text-[#6E6961] hover:text-[#1F1E1D] disabled:opacity-30"
            >
              <ChevronRight className="w-3.5 h-3.5" />
            </button>
          </div>

          {/* View Mode Switcher */}
          <div className="flex items-center bg-[#FAF9F5] border border-[#E2DBD0] rounded-xl p-0.5 text-xs whitespace-nowrap shrink-0">
            <button
              onClick={() => setViewMode("split")}
              className={`px-3 py-1.5 rounded-lg whitespace-nowrap transition-all ${
                viewMode === "split"
                  ? "bg-white text-[#1F1E1D] font-medium shadow-2xs"
                  : "text-[#6E6961] hover:text-[#1F1E1D]"
              }`}
            >
              Side by Side
            </button>
            <button
              onClick={() => setViewMode("zh")}
              disabled={Boolean(spreadOnly)}
              title={spreadOnly ? "This job only generated a bilingual PDF" : undefined}
              className={`px-3 py-1.5 rounded-lg whitespace-nowrap transition-all ${
                viewMode === "zh"
                  ? "bg-white text-[#1F1E1D] font-medium shadow-2xs"
                  : "text-[#6E6961] hover:text-[#1F1E1D]"
              }`}
            >
              Translation
            </button>
            <button
              onClick={() => setViewMode("en")}
              className={`px-3 py-1.5 rounded-lg whitespace-nowrap transition-all ${
                viewMode === "en"
                  ? "bg-white text-[#1F1E1D] font-medium shadow-2xs"
                  : "text-[#6E6961] hover:text-[#1F1E1D]"
              }`}
            >
              Original
            </button>
          </div>

          {/* Zoom */}
          <div className="hidden sm:flex items-center space-x-1 text-xs text-[#6E6961] whitespace-nowrap shrink-0">
            <button
              onClick={() => setZoom((z) => Math.max(80, z - 10))}
              className="p-1 hover:bg-[#EAE5D9] rounded"
            >
              <ZoomOut className="w-3.5 h-3.5" />
            </button>
            <span className="font-mono w-9 text-center select-none">{zoom}%</span>
            <button
              onClick={() => setZoom((z) => Math.min(140, z + 10))}
              className="p-1 hover:bg-[#EAE5D9] rounded"
            >
              <ZoomIn className="w-3.5 h-3.5" />
            </button>
          </div>
        </div>

        {/* Export Buttons */}
        <div className="flex items-center space-x-2 shrink-0 whitespace-nowrap">
          <button disabled={!dualDownload} onClick={() => window.location.assign(dualDownload)} className="flex items-center space-x-1.5 px-3.5 py-1.5 text-xs rounded-xl bg-[#FAF9F5] hover:bg-[#EAE5D9] border border-[#E2DBD0] text-[#1F1E1D] font-medium whitespace-nowrap shrink-0">
            <Download className="w-3.5 h-3.5 text-[#D97757]" />
            <span>Export Dual</span>
          </button>
          <button disabled={!job.files.mono} onClick={() => window.location.assign(job.files.mono)} className="flex items-center space-x-1.5 px-3.5 py-1.5 text-xs rounded-xl bg-[#D97757] hover:bg-[#C15F3C] text-white font-medium whitespace-nowrap shrink-0">
            <span>Export Translation</span>
          </button>
        </div>
      </div>

      {/* Reader Body */}
      <div className="flex-1 overflow-y-auto p-8 flex justify-center">
        <div
          className={`w-full transition-all duration-200 ${
            viewMode === "split" && !spreadOnly
              ? "max-w-6xl grid grid-cols-2 gap-8"
              : "max-w-2xl mx-auto"
          }`}
          style={{ transform: `scale(${zoom / 100})`, transformOrigin: "top center" }}
        >
          {!paper.has_content ? <>
            {spreadOnly && viewMode === "split" && <iframe key={`spread-${currentPage}`} title="Bilingual PDF" src={previewUrl(job, "side_by_side", currentPage)} className="w-full h-[75vh] bg-white border border-[#E2DBD0] rounded-2xl shadow-sm" />}
            {((viewMode === "split" && !spreadOnly) || viewMode === "en") && <iframe key={`original-${currentPage}`} title="Original PDF" src={previewUrl(job, "original", originalPage)} className="w-full h-[75vh] bg-white border border-[#E2DBD0] rounded-2xl shadow-sm" />}
            {((viewMode === "split" && !spreadOnly) || viewMode === "zh") && (job.files.mono || job.files.dual ? <iframe key={`translation-${currentPage}`} title="Translated PDF" src={previewUrl(job, translationKind, translationPage)} className="w-full h-[75vh] bg-white border border-[#E2DBD0] rounded-2xl shadow-sm" /> : <p className="text-sm text-[#78716A]">A separate translation PDF was not generated for this job.</p>)}
          </> : <>
          {/* Left: Original (English) */}
          {(viewMode === "split" || viewMode === "en") && (
            <div className="bg-white border border-[#E2DBD0] rounded-2xl p-8 space-y-6 shadow-sm select-text">
              <div className="border-b border-[#EAE5D9] pb-6 text-center space-y-2">
                <h1 className="font-serif text-2xl font-bold text-[#1F1E1D] leading-snug">
                  {paper.title}
                </h1>
                <p className="text-xs text-[#6E6961]">
                  {paper.authors}
                </p>
              </div>

              {/* Abstract */}
              <div
                onMouseEnter={() => setHoveredParagraph("abstract")}
                onMouseLeave={() => setHoveredParagraph(null)}
                className={`p-4 rounded-xl border transition-all ${
                  hoveredParagraph === "abstract"
                    ? "bg-[#FAF2ED] border-[#D97757]/40"
                    : "bg-[#FDFBF7] border-[#E8E4DA]"
                }`}
              >
                <h3 className="font-serif font-bold text-xs uppercase tracking-wider text-[#1F1E1D] mb-1.5 text-center">
                  Abstract
                </h3>
                <p className="text-xs text-[#2E2824] leading-relaxed font-serif text-justify">
                  {paper.abstract.en}
                </p>
              </div>

              {/* Sections */}
              {paper.sections.map((sec) => (
                <div key={sec.id} className="space-y-3 pt-2">
                  <h2 className="font-serif font-bold text-sm text-[#1F1E1D] border-b border-[#F0ECE1] pb-1">
                    {sec.headingEn}
                  </h2>

                  {sec.paragraphs.map((p) => (
                    <div
                      key={p.id}
                      onMouseEnter={() => setHoveredParagraph(p.id)}
                      onMouseLeave={() => setHoveredParagraph(null)}
                      className={`p-2.5 rounded-xl border transition-all text-xs text-[#2E2824] font-serif leading-relaxed text-justify ${
                        hoveredParagraph === p.id
                          ? "bg-[#FAF2ED] border-[#D97757]/50"
                          : "border-transparent hover:border-[#E8E4DA]"
                      }`}
                    >
                      {p.en}
                    </div>
                  ))}

                  {sec.formula && (
                    <div
                      onClick={() => setActiveFormula(sec.formula)}
                      className="p-4 rounded-xl bg-[#FAF9F5] border border-[#E2DBD0] hover:border-[#D97757] cursor-pointer text-center"
                    >
                      <div className="font-mono text-xs py-2 bg-white rounded-lg border border-[#EAE5D9] text-[#1F1E1D]">
                        {sec.formula.latex}
                      </div>
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}

          {/* Right: Translation (Chinese) */}
          {(viewMode === "split" || viewMode === "zh") && (
            <div className="bg-white border border-[#E2DBD0] rounded-2xl p-8 space-y-6 shadow-sm select-text">
              <div className="border-b border-[#EAE5D9] pb-6 text-center space-y-2">
                <h1 className="font-serif text-2xl font-bold text-[#1F1E1D] leading-snug">
                  {paper.titleZh}
                </h1>
                <p className="text-xs text-[#6E6961] font-serif">
                  {paper.authors}
                </p>
              </div>

              {/* Abstract */}
              <div
                onMouseEnter={() => setHoveredParagraph("abstract")}
                onMouseLeave={() => setHoveredParagraph(null)}
                className={`p-4 rounded-xl border transition-all ${
                  hoveredParagraph === "abstract"
                    ? "bg-[#FAF2ED] border-[#D97757]/40"
                    : "bg-[#FDFBF7] border-[#E8E4DA]"
                }`}
              >
                <h3 className="font-serif font-bold text-xs uppercase tracking-wider text-[#1F1E1D] mb-1.5 text-center">
                  摘要
                </h3>
                <p className="text-xs text-[#2E2824] leading-relaxed font-serif text-justify indent-6">
                  {paper.abstract.zh}
                </p>
              </div>

              {/* Sections */}
              {paper.sections.map((sec) => (
                <div key={sec.id} className="space-y-3 pt-2">
                  <h2 className="font-serif font-bold text-sm text-[#1F1E1D] border-b border-[#F0ECE1] pb-1">
                    {sec.headingZh}
                  </h2>

                  {sec.paragraphs.map((p) => (
                    <div
                      key={p.id}
                      onMouseEnter={() => setHoveredParagraph(p.id)}
                      onMouseLeave={() => setHoveredParagraph(null)}
                      className={`p-2.5 rounded-xl border transition-all text-xs text-[#2E2824] font-serif leading-relaxed text-justify indent-6 ${
                        hoveredParagraph === p.id
                          ? "bg-[#FAF2ED] border-[#D97757]/50"
                          : "border-transparent hover:border-[#E8E4DA]"
                      }`}
                    >
                      {p.zh}
                    </div>
                  ))}

                  {sec.formula && (
                    <div
                      onClick={() => setActiveFormula(sec.formula)}
                      className="p-4 rounded-xl bg-[#FAF9F5] border border-[#E2DBD0] hover:border-[#D97757] cursor-pointer text-center"
                    >
                      <div className="font-mono text-xs py-2 bg-white rounded-lg border border-[#EAE5D9] text-[#1F1E1D]">
                        {sec.formula.latex}
                      </div>
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}
          </>}
        </div>
      </div>

      {/* Clean Bottom Terminology Bar */}
      <div className="h-10 bg-[#FAF9F5] border-t border-[#E8E4DA] px-8 flex items-center space-x-3 text-xs text-[#6E6961] shrink-0">
        <span className="text-[#1F1E1D] font-medium">Terms:</span>
        <div className="flex items-center space-x-2">
          {glossary.map((g, idx) => (
            <button
              key={idx}
              onClick={() => setSelectedTerm(g)}
              className="px-2.5 py-0.5 rounded-lg bg-white border border-[#E2DBD0] hover:border-[#D97757] text-xs text-[#4A453E] hover:text-[#D97757] transition-all"
            >
              {g.term} · {g.zh}
            </button>
          ))}
        </div>
      </div>

      {/* Formula Modal */}
      {activeFormula && (
        <div className="fixed inset-0 bg-black/25 flex items-center justify-center p-4 z-50">
          <div className="bg-[#FAF9F5] border border-[#E2DBD0] rounded-2xl max-w-md w-full p-6 shadow-xl space-y-4">
            <div className="flex items-center justify-between border-b border-[#E8E4DA] pb-3">
              <h3 className="font-serif font-bold text-base text-[#1F1E1D]">
                {activeFormula.name}
              </h3>
              <button
                onClick={() => setActiveFormula(null)}
                className="text-xs text-[#8C8479] hover:text-[#1F1E1D]"
              >
                Close
              </button>
            </div>

            <div className="p-3 bg-white border border-[#E2DBD0] rounded-xl font-mono text-xs text-[#1F1E1D] break-all">
              {activeFormula.latex}
            </div>

            <p className="text-xs text-[#4A453E] leading-relaxed font-serif bg-[#F4EFE6] p-3 rounded-xl">
              {activeFormula.explanation}
            </p>

            <div className="flex justify-end pt-2">
              <button
                onClick={() => handleCopyFormula(activeFormula.latex)}
                className="flex items-center space-x-1.5 px-4 py-2 text-xs font-medium rounded-xl bg-[#D97757] hover:bg-[#C15F3C] text-white transition-all"
              >
                {copiedFormula ? <Check className="w-3.5 h-3.5" /> : <Copy className="w-3.5 h-3.5" />}
                <span>{copiedFormula ? "Copied" : "Copy Code"}</span>
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Term Modal */}
      {selectedTerm && (
        <div className="fixed inset-0 bg-black/25 flex items-center justify-center p-4 z-50">
          <div className="bg-[#FAF9F5] border border-[#E2DBD0] rounded-2xl max-w-sm w-full p-6 shadow-xl space-y-4">
            <div className="flex items-center justify-between border-b border-[#E8E4DA] pb-3">
              <h3 className="font-serif font-bold text-base text-[#1F1E1D]">
                {selectedTerm.term}
              </h3>
              <button
                onClick={() => setSelectedTerm(null)}
                className="text-xs text-[#8C8479] hover:text-[#1F1E1D]"
              >
                Close
              </button>
            </div>

            <div className="p-3 bg-white rounded-xl border border-[#E2DBD0] space-y-1.5">
              <div className="text-sm font-semibold text-[#2D6A4F]">
                {selectedTerm.zh}
              </div>
              <p className="text-xs text-[#4A453E] leading-relaxed font-serif">
                {selectedTerm.desc}
              </p>
            </div>

            <div className="flex justify-end pt-2">
              <button
                onClick={() => setSelectedTerm(null)}
                className="px-4 py-1.5 text-xs rounded-xl bg-white border border-[#E2DBD0] text-[#1F1E1D]"
              >
                Close
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
