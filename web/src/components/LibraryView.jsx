"use client";

import React, { useState, useEffect } from "react";
import { Search, BookOpen, Download, Trash2 } from "lucide-react";
import { api } from "../lib/api";

export default function LibraryView({ onOpenReader, version, onDeleted, onResume }) {
  const [papers, setPapers] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [deletingId, setDeletingId] = useState(null);

  useEffect(() => {
    let active = true;
    setLoading(true);
    api.papers().then((items) => { if (active) { setPapers(items); setError(""); } })
      .catch((err) => { if (active) setError(err.message); })
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [version]);

  const removePaper = async (id) => {
    if (!window.confirm("Remove this paper from the Library? Files and translation cache will be retained.")) return;
    setDeletingId(id); setError("");
    try {
      await api.deletePaper(id);
      setPapers((items) => items.filter((item) => item.id !== id));
      onDeleted?.(id);
    } catch (err) { setError(err.message); }
    finally { setDeletingId(null); }
  };

  const downloadPaper = async (id) => {
    try {
      const job = await api.job(id);
      const url = job.files.side_by_side || job.files.dual || job.files.mono;
      if (!url) throw new Error("No generated PDF is available.");
      window.location.assign(url);
    } catch (err) { setError(err.message); }
  };
  const [searchTerm, setSearchTerm] = useState("");

  const filteredPapers = papers.filter(
    (p) =>
      p.title.toLowerCase().includes(searchTerm.toLowerCase()) ||
      p.zhTitle.toLowerCase().includes(searchTerm.toLowerCase())
  );

  return (
    <div className="max-w-3xl mx-auto space-y-6 py-4">
      {/* Editorial Header */}
      <div className="flex items-end justify-between">
        <div>
          <h1 className="font-serif text-3xl font-bold text-[#1F1E1D] tracking-tight">
            Library
          </h1>
          <p className="text-sm text-[#78716A] mt-2 font-serif">
            Archive of translated academic manuscripts.
          </p>
        </div>

        {/* Search */}
        <div className="relative w-64">
          <Search className="w-4 h-4 text-[#9B9488] absolute left-3 top-1/2 -translate-y-1/2" />
          <input
            type="text"
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            placeholder="Search papers..."
            className="w-full text-xs bg-white border border-[#E2DBD0] rounded-xl pl-9 pr-3 py-2 text-[#1F1E1D] focus:outline-none focus:ring-1 focus:ring-[#D97757]"
          />
        </div>
      </div>

      {error && <p role="alert" className="text-sm text-[#DC2626]">{error}</p>}
      {loading && <p role="status" className="text-sm text-[#78716A]">Loading library…</p>}
      {!loading && !filteredPapers.length && <p className="text-sm text-[#78716A]">{searchTerm ? "No matching papers." : "No translated papers yet."}</p>}
      {/* Papers List */}
      <div className="space-y-3 pt-2">
        {filteredPapers.map((paper) => (
          <div
            key={paper.id}
            className="p-5 rounded-2xl bg-white border border-[#E8E4DA] hover:border-[#D97757]/50 transition-all flex items-center justify-between group"
          >
            <div className="space-y-1">
              <div className="flex items-center space-x-2">
                <h3 className="font-serif font-bold text-base text-[#1F1E1D] group-hover:text-[#D97757] transition-colors">
                  {paper.title}
                </h3>
              </div>
              <p className="text-sm text-[#78716A] font-serif">
                {paper.status === "done" ? paper.zhTitle : `Task status: ${paper.status}`}
              </p>
              <div className="flex items-center space-x-3 text-xs text-[#8C8479] pt-1">
                <span>{paper.pages} pages</span>
                <span>·</span>
                <span>{paper.size}</span>
                <span>·</span>
                <span>{paper.authors}</span>
                <span>·</span>
                <span>{paper.date}</span>
              </div>
            </div>

            <div className="flex items-center space-x-2">
              <button
                onClick={() => paper.status === "done" ? onOpenReader(paper.id) : onResume(paper.id)}
                className="flex items-center space-x-1.5 px-4 py-2 rounded-xl bg-[#FAF9F5] hover:bg-[#FAF0EB] text-[#1F1E1D] hover:text-[#D97757] border border-[#E2DBD0] text-xs font-medium transition-all"
              >
                <BookOpen className="w-3.5 h-3.5" />
                <span>{paper.status === "done" ? "Read" : "View task"}</span>
              </button>
              <button
                title="Download"
                disabled={!paper.downloadable}
                disabled={!paper.downloadable}
                onClick={() => downloadPaper(paper.id)}
                className="p-2 rounded-xl bg-[#FAF9F5] hover:bg-[#EAE5D9] text-[#6E6961] border border-[#E2DBD0]"
              >
                <Download className="w-4 h-4" />
              </button>
              <button
                onClick={() => removePaper(paper.id)}
                disabled={deletingId === paper.id}
                title="Delete"
                className="p-2 rounded-xl bg-[#FAF9F5] hover:bg-[#FDF2F2] text-[#6E6961] hover:text-[#DC2626] border border-[#E2DBD0]"
              >
                <Trash2 className="w-4 h-4" />
              </button>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
