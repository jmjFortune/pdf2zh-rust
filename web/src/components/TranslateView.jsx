"use client";

import React, { useState, useEffect, useRef } from "react";
import {
  UploadCloud,
  FileText,
  ArrowRight,
  RefreshCw,
  BookOpen
} from "lucide-react";
import { api, terminal } from "../lib/api";

export default function TranslateView({ settings, settingsError, onRetry, onOpenReader, onAddToHistory, removedPaperId, resumeJobId }) {
  const [selectedFile, setSelectedFile] = useState(null);
  const [isDragging, setIsDragging] = useState(false);
  const [sourceLang, setSourceLang] = useState("en");
  const [targetLang, setTargetLang] = useState("zh");
  const [engine, setEngine] = useState("google");
  const [pageRange, setPageRange] = useState("all");
  const [customRange, setCustomRange] = useState("1-3");
  const [layoutModes, setLayoutModes] = useState(["side_by_side"]);
  const [job, setJob] = useState(null);
  const [uploading, setUploading] = useState(false);
  const [restoring, setRestoring] = useState(true);
  const [error, setError] = useState("");
  const [connectionError, setConnectionError] = useState("");
  const inputRef = useRef(null);
  const callbacks = useRef({ onAddToHistory });
  callbacks.current = { onAddToHistory };
  const notified = useRef(new Set());
  const services = settings?.services || [];
  const selectedService = services.find((item) => item.id === engine);
  const isTranslating = uploading || Boolean(job && !terminal(job.status));
  const isCompleted = job?.status === "done";
  const progress = job?.progress?.total ? Math.min(100, Math.max(0, Math.round(job.progress.current / job.progress.total * 100))) : 0;
  const analyzing = job?.stage === "layout";
  const currentStep = isCompleted ? 4 : ({ analyze: 0, layout: 0, extract: 1, translate: 2, save: 3 }[job?.stage] ?? 0);
  const steps = ["Layout Analysis", "Text Extraction", "Translation", "PDF Export", "Complete"];

  useEffect(() => {
    if (!settings || job?.options) return;
    setEngine(settings.defaults.service);
    setSourceLang(settings.defaults.lang_in);
    setTargetLang(settings.defaults.lang_out);
    const formats = settings.defaults.output_formats;
    setLayoutModes(formats.length ? formats : ["mono"]);
  }, [settings]);

  useEffect(() => {
    let active = true;
    const id = sessionStorage.getItem("pdf2zh.activeJob");
    (id ? api.job(id) : api.activeJob()).then((next) => { if (active && next) { setJob(next); sessionStorage.setItem("pdf2zh.activeJob", next.id); } }).catch((err) => {
      if (err.status === 404) sessionStorage.removeItem("pdf2zh.activeJob");
      else if (active) setError(err.message);
    }).finally(() => { if (active) setRestoring(false); });
    return () => { active = false; };
  }, []);

  useEffect(() => {
    const options = job?.options;
    if (!options) return;
    setEngine(options.service); setSourceLang(options.lang_in); setTargetLang(options.lang_out);
    setPageRange(!options.page_range ? "all" : options.page_range === "first3" ? "preview3" : "custom");
    setCustomRange(options.page_range || "1-3"); setLayoutModes(options.output_formats || ["mono"]);
  }, [job?.id]);

  useEffect(() => {
    if (!resumeJobId) return;
    let active = true;
    setRestoring(true); setError(""); setSelectedFile(null);
    api.job(resumeJobId).then((next) => {
      if (active) { setJob(next); sessionStorage.setItem("pdf2zh.activeJob", next.id); }
    }).catch((err) => { if (active) setError(err.message); }).finally(() => { if (active) setRestoring(false); });
    return () => { active = false; };
  }, [resumeJobId]);

  const jobId = job?.id;
  useEffect(() => {
    if (removedPaperId && removedPaperId === jobId) {
      setJob(null);
      setSelectedFile(null);
      sessionStorage.removeItem("pdf2zh.activeJob");
      if (inputRef.current) inputRef.current.value = "";
    }
  }, [removedPaperId, jobId]);

  useEffect(() => {
    if (!jobId) return;
    let active = true;
    let source;
    let lastSeq = 0;
    const update = (next) => {
      if (!active) return;
      setJob(next);
      if (terminal(next.status)) {
        source?.close();
        setConnectionError("");
        if (!notified.current.has(jobId)) {
          notified.current.add(jobId);
          callbacks.current.onAddToHistory?.(next);
        }
      }
    };
    source = new EventSource(`/api/ui/jobs/${encodeURIComponent(jobId)}/events`);
    source.onopen = () => { if (active) setConnectionError(""); };
    source.onmessage = (message) => {
      if (!active) return;
      try {
        const event = JSON.parse(message.data);
        if (event.seq <= lastSeq) return;
        lastSeq = event.seq;
        setJob((previous) => ({
          ...previous,
          ...(event.job || {}),
          stage: event.stage ?? previous.stage,
          message: event.message ?? previous.message,
          progress: { current: event.current ?? previous.progress.current, total: event.total ?? previous.progress.total },
          ...(event.data?.files ? { files: event.data.files } : {}),
          ...(terminal(event.type) ? { status: event.type, error: event.data?.error } : {}),
        }));
        if (terminal(event.type)) {
          source.close();
          if (!notified.current.has(jobId)) {
            notified.current.add(jobId);
            callbacks.current.onAddToHistory?.();
          }
          api.job(jobId).then(update).catch(() => {
            if (active) setConnectionError("Result saved. Reconnect to update details.");
          });
        }
      } catch { setConnectionError("Unable to read progress. Reconnecting…"); }
    };
    source.onerror = () => {
      if (!active) return;
      setConnectionError("Connection interrupted. Reconnecting…");
      // Native EventSource retries the stream; one snapshot resolves finished
      // jobs after a server restart without starting a polling loop.
      api.job(jobId).then(update).catch((err) => {
        if (!active) return;
        if (err.status === 404) { source.close(); setJob(null); setError(err.message); }
      });
    };
    const refresh = () => api.job(jobId).then(update).catch((err) => { if (active) setError(err.message); });
    window.addEventListener("online", refresh);
    refresh();
    return () => { active = false; source.close(); window.removeEventListener("online", refresh); };
  }, [jobId]);

  const selectFile = (file) => {
    if (!file || isTranslating) return;
    if (!file.name.toLowerCase().endsWith(".pdf")) { setError("Please choose a PDF file."); return; }
    setSelectedFile(file);
    setJob(null);
    setError("");
    setConnectionError("");
    sessionStorage.removeItem("pdf2zh.activeJob");
  };

  const handleStartTranslate = async () => {
    if ((!selectedFile && !job) || !settings || isTranslating) return;
    if (pageRange === "custom" && !/^\s*\d+(?:\s*-\s*\d+)?(?:\s*,\s*\d+(?:\s*-\s*\d+)?)*\s*$/.test(customRange)) {
      setError("Enter a page range such as 1-3, 5."); return;
    }
    setUploading(true);
    setError("");
    try {
      const body = new FormData();
      if (selectedFile) body.append("file", selectedFile);
      body.append("lang_in", sourceLang);
      body.append("lang_out", targetLang);
      body.append("service", engine);
      body.append("page_range", pageRange === "all" ? "" : pageRange === "preview3" ? "first3" : customRange);
      body.append("thread", String(settings.defaults.thread));
      body.append("output_formats", layoutModes.join(","));
      const next = selectedFile ? await api.createJob(body) : await api.retryJob(job.id, body);
      sessionStorage.setItem("pdf2zh.activeJob", next.id);
      setJob(next);
    } catch (err) { setError(err.message); }
    finally { setUploading(false); }
  };

  const handleCancel = async () => {
    try { setJob(await api.cancelJob(job.id)); }
    catch (err) { setError(err.message); }
  };

  return (
    <div className="max-w-3xl mx-auto space-y-8 py-4">
      {/* Editorial Header */}
      <div>
        <h1 className="font-serif text-3xl font-bold text-[#1F1E1D] tracking-tight">
          Translate Paper
        </h1>
        <p className="text-sm text-[#78716A] mt-2 font-serif leading-relaxed">
          Preserve paper layout, mathematical formulas, and citations with neural precision.
        </p>
      </div>

      {(error || settingsError) && <div role="alert" className="text-sm text-[#DC2626]">{error || settingsError}{settingsError && <button onClick={onRetry} className="ml-3 underline">Retry connection</button>}</div>}
      {selectedService?.needs_key && !selectedService.has_key && <p role="status" className="text-sm text-[#78716A]">Configure {selectedService.name} credentials in Settings before translating.</p>}
      {/* Main Upload Dropzone */}
      <div
        onDragOver={(e) => {
          e.preventDefault();
          setIsDragging(true);
        }}
        onDragLeave={() => setIsDragging(false)}
        onDrop={(e) => {
          e.preventDefault();
          setIsDragging(false);
          selectFile(e.dataTransfer.files?.[0]);
        }}
        className={`border-2 border-dashed rounded-2xl p-8 text-center transition-all ${
          isDragging
            ? "border-[#D97757] bg-[#FBF2ED]"
            : "border-[#DCD6C9] bg-white hover:border-[#BDB5A4]"
        }`}
      >
        <div className="max-w-sm mx-auto space-y-3">
          <div className="w-12 h-12 mx-auto rounded-2xl bg-[#FAF6EE] flex items-center justify-center text-[#D97757]">
            <UploadCloud className="w-6 h-6" />
          </div>

          <div className="text-sm text-[#1F1E1D]">
            Drop a PDF paper here, or{" "}
            <label className="text-[#D97757] hover:underline cursor-pointer font-medium">
              browse file
              <input
                type="file"
                accept=".pdf"
                className="hidden"
                ref={inputRef}
                disabled={isTranslating}
                aria-label="Choose PDF"
                onChange={(e) => selectFile(e.target.files?.[0])}
              />
            </label>
          </div>

          <div className="pt-1">
            <button
              onClick={() => inputRef.current?.click()}
              disabled={isTranslating}
              className="text-xs font-serif px-3 py-1.5 rounded-lg bg-[#FAF9F5] text-[#78716A] hover:text-[#1F1E1D] border border-[#E2DBD0] transition-colors"
            >
              Choose a PDF to start
            </button>
          </div>
        </div>
      </div>

      {/* Selected File Card */}
      {(selectedFile || job) && (
        <div className="p-4 rounded-xl bg-white border border-[#E2DBD0] flex items-center justify-between">
          <div className="flex items-center space-x-3">
            <div className="w-9 h-9 rounded-lg bg-[#FAF0EB] text-[#D97757] flex items-center justify-center">
              <FileText className="w-5 h-5" />
            </div>
            <div>
              <div className="text-sm font-medium text-[#1F1E1D]">
                {selectedFile?.name || job.filename}
              </div>
              <div className="text-xs text-[#78716A] mt-0.5">
                {job?.pages ? `${job.pages} pages · ` : ""}{((selectedFile?.size ?? job.size) / 1024 / 1024).toFixed(1)} MB
              </div>
            </div>
          </div>

          <span className="text-xs text-[#2D6A4F] bg-[#E8F5EE] px-3 py-1 rounded-full font-medium">
            {job ? {done: "Complete", cancelled: "Cancelled", error: "Failed"}[job.status] || "Processing" : "Ready"}
          </span>
        </div>
      )}

      {job && !job.options && <p role="status" className="text-sm text-[#78716A]">Previous task settings were not recorded. Choose a service below before retrying the retained PDF.</p>}
      {/* Settings Form */}
      <div className="p-6 rounded-2xl bg-white border border-[#E8E4DA] space-y-6">
        {/* Languages */}
        <div className="grid grid-cols-2 gap-4">
          <div className="space-y-1.5">
            <label className="text-xs text-[#78716A] font-medium">Source Language</label>
            <select
              aria-label="Source Language"
              value={sourceLang}
              disabled={isTranslating}
              onChange={(e) => setSourceLang(e.target.value)}
              className="w-full text-sm bg-[#FAF9F5] border border-[#DCD6C9] rounded-xl px-3 py-2 text-[#1F1E1D] focus:outline-none focus:ring-1 focus:ring-[#D97757]"
            >
              <option value="auto">Auto Detect</option>
              <option value="en">English</option>
              <option value="ja">Japanese</option>
              <option value="de">German</option>
              <option value="fr">French</option>
              {!['auto', 'en', 'ja', 'de', 'fr'].includes(sourceLang) && <option value={sourceLang}>{sourceLang}</option>}
            </select>
          </div>

          <div className="space-y-1.5">
            <label className="text-xs text-[#78716A] font-medium">Target Language</label>
            <select
              aria-label="Target Language"
              value={targetLang}
              disabled={isTranslating}
              onChange={(e) => setTargetLang(e.target.value)}
              className="w-full text-sm bg-[#FAF9F5] border border-[#DCD6C9] rounded-xl px-3 py-2 text-[#1F1E1D] focus:outline-none focus:ring-1 focus:ring-[#D97757]"
            >
              <option value="zh-CN">Simplified Chinese</option>
              <option value="zh-TW">Traditional Chinese</option>
              <option value="en">English</option>
              <option value="es">Spanish</option>
              {!['zh-CN', 'zh-TW', 'en', 'es'].includes(targetLang) && <option value={targetLang}>{targetLang}</option>}
            </select>
          </div>
        </div>

        {/* Translation Engine */}
        <div className="space-y-2">
          <label className="text-xs text-[#78716A] font-medium">Translation Service</label>
          <div className="grid grid-cols-2 sm:grid-cols-4 gap-2">
            {services.map((m) => (
              <button
                key={m.id}
                disabled={isTranslating}
                onClick={() => setEngine(m.id)}
                className={`py-2.5 px-3 rounded-xl border text-xs font-medium text-center transition-all ${
                  engine === m.id
                    ? "border-[#D97757] bg-[#FBF2ED] text-[#D97757]"
                    : "border-[#E8E4DA] bg-[#FAF9F5] text-[#4A453E] hover:bg-[#F2ECE1]"
                }`}
              >
                {m.name}
              </button>
            ))}
          </div>
        </div>

        {/* Page Range & Layout */}
        <div className="grid grid-cols-2 gap-4">
          <div className="space-y-2">
            <label className="text-xs text-[#78716A] font-medium">Page Range</label>
            <div className="grid grid-cols-3 gap-2">
              {[
                { id: "all", label: "All" },
                { id: "preview3", label: "First 3" },
                { id: "custom", label: "Custom" },
              ].map((r) => (
                <button
                  key={r.id}
                  disabled={isTranslating}
                  onClick={() => setPageRange(r.id)}
                  className={`py-2 rounded-xl border text-xs text-center transition-all ${
                    pageRange === r.id
                      ? "border-[#D97757] bg-[#FBF2ED] text-[#D97757] font-medium"
                      : "border-[#E8E4DA] bg-[#FAF9F5] text-[#6E6961] hover:bg-[#F2ECE1]"
                  }`}
                >
                  {r.label}
                </button>
              ))}
            </div>
            {pageRange === "custom" && (
              <input
                type="text"
                value={customRange}
                onChange={(e) => setCustomRange(e.target.value)}
                placeholder="e.g. 1-3, 5"
                className="w-full text-xs bg-[#FAF9F5] border border-[#DCD6C9] rounded-xl px-3 py-1.5 text-[#1F1E1D] mt-2 focus:outline-none focus:ring-1 focus:ring-[#D97757]"
              />
            )}
          </div>

          <div className="space-y-2">
            <label className="text-xs text-[#78716A] font-medium">Layout Modes (select one or more)</label>
            <div className="grid grid-cols-3 gap-2">
              {[
                { id: "side_by_side", label: "Side by Side" },
                { id: "dual", label: "Interleaved" },
                { id: "mono", label: "Mono" },
              ].map((l) => (
                <button
                  key={l.id}
                  aria-pressed={layoutModes.includes(l.id)}
                  disabled={isTranslating || (layoutModes.length === 1 && layoutModes.includes(l.id))}
                  onClick={() => setLayoutModes((current) => current.includes(l.id)
                    ? current.filter((id) => id !== l.id)
                    : [...current, l.id])}
                  className={`py-2 rounded-xl border text-xs text-center transition-all ${
                    layoutModes.includes(l.id)
                      ? "border-[#D97757] bg-[#FBF2ED] text-[#D97757] font-medium"
                      : "border-[#E8E4DA] bg-[#FAF9F5] text-[#6E6961] hover:bg-[#F2ECE1]"
                  }`}
                >
                  {l.label}
                </button>
              ))}
            </div>
          </div>
        </div>
      </div>

      {/* Stepper */}
      {(isTranslating || job) && (
        <div className="p-6 rounded-2xl bg-white border border-[#E8E4DA] space-y-4">
          <div className="flex items-center justify-between text-sm">
            <span className="font-medium text-[#1F1E1D]">
              {isCompleted ? "Translation Complete" : job?.status === "cancelled" ? "Translation Cancelled" : job?.status === "error" ? "Translation Failed" : "Translating Paper"}
            </span>
            <span className="font-mono text-[#D97757] font-semibold">{analyzing ? "Analyzing" : `${progress}%`}</span>
          </div>

          <div className="w-full bg-[#EAE5D9] h-1.5 rounded-full overflow-hidden">
            <div
              className={`bg-[#D97757] h-full transition-all duration-300 rounded-full ${analyzing ? "animate-pulse" : ""}`}
              role="progressbar"
              aria-label={analyzing ? "Layout analysis in progress" : "Translated pages"}
              aria-valuenow={analyzing ? undefined : progress}
              aria-valuemin={0}
              aria-valuemax={100}
              style={{ width: analyzing ? "33%" : `${progress}%` }}
            />
          </div>

          <p role="status" className="text-xs text-[#78716A]">{connectionError || job?.message || "Uploading PDF…"}</p>
          <div className="flex justify-between pt-1">
            {steps.map((st, idx) => (
              <span
                key={idx}
                className={`text-xs ${
                  currentStep >= idx || isCompleted
                    ? "text-[#1F1E1D] font-medium"
                    : "text-[#BDB5A4]"
                }`}
              >
                {st}
              </span>
            ))}
          </div>
        </div>
      )}

      {job?.error && <p role="alert" className="text-sm text-[#DC2626]">{job.error}</p>}
      {/* Action Button */}
      <div className="flex justify-end pt-2">
        {isCompleted && <button onClick={() => { setJob(null); setSelectedFile(null); sessionStorage.removeItem("pdf2zh.activeJob"); if (inputRef.current) inputRef.current.value = ""; }} className="px-5 py-3 mr-3 rounded-xl bg-white border border-[#E2DBD0] text-sm text-[#6E6961]">Translate another</button>}
        {job && !terminal(job.status) && <button onClick={handleCancel} className="px-5 py-3 mr-3 rounded-xl bg-white border border-[#E2DBD0] text-sm text-[#6E6961]">Cancel</button>}
        {isCompleted ? (
          <button
            onClick={() => onOpenReader(job.id)}
            className="flex items-center space-x-2 px-8 py-3 rounded-xl bg-[#2D6A4F] hover:bg-[#24543E] text-white font-medium text-sm transition-all"
          >
            <BookOpen className="w-4 h-4" />
            <span>Open in Reader</span>
            <ArrowRight className="w-4 h-4" />
          </button>
        ) : (
          <button
            disabled={isTranslating || restoring || (!selectedFile && !job) || !settings || (selectedService?.needs_key && !selectedService.has_key)}
            onClick={handleStartTranslate}
            className={`flex items-center space-x-2 px-8 py-3 rounded-xl font-medium text-sm transition-all ${
              isTranslating || restoring || (!selectedFile && !job) || !settings || (selectedService?.needs_key && !selectedService.has_key)
                ? "bg-[#DCD6C9] text-[#8C8479] cursor-not-allowed"
                : "bg-[#D97757] hover:bg-[#C15F3C] text-white shadow-sm"
            }`}
          >
            {isTranslating ? (
              <>
                <RefreshCw className="w-4 h-4 animate-spin" />
                <span>Translating</span>
              </>
            ) : (
              <span>{job && !selectedFile ? "Retry retained PDF" : "Start Translation"}</span>
            )}
          </button>
        )}
      </div>
    </div>
  );
}
