"use client";

import React from "react";
import { Download } from "lucide-react";

export default function AboutView() {
  return (
    <div className="max-w-2xl mx-auto space-y-8 py-6">
      {/* Header */}
      <div>
        <h1 className="font-serif text-3xl font-bold text-[#1F1E1D] tracking-tight">
          About translator
        </h1>
        <p className="text-sm text-[#78716A] mt-2 font-serif leading-relaxed">
          translator interface with the independent Rust / MuPDF engine. Development preview; OCR and structured paragraph reading are not yet available.
        </p>
      </div>

      {/* Zotero Card */}
      <div className="p-6 rounded-2xl bg-white border border-[#E8E4DA] space-y-4">
        <div className="flex items-center justify-between">
          <div>
            <h3 className="font-serif font-bold text-base text-[#1F1E1D]">
              Zotero Integration
            </h3>
            <p className="text-xs text-[#78716A] mt-1">
              The Zotero connector requires the original translator backend. This Rust service does not yet implement its integration endpoints.
            </p>
          </div>

          <a
            href="/pdf2zh-connector-v1.0.21.xpi"
            download
            className="flex items-center space-x-1.5 px-4 py-2 rounded-xl bg-[#FAF9F5] hover:bg-[#FAF0EB] text-[#1F1E1D] hover:text-[#D97757] border border-[#E2DBD0] text-xs font-medium transition-all"
          >
            <Download className="w-3.5 h-3.5" />
            <span>Original Backend Plugin</span>
          </a>
        </div>

        <div className="grid grid-cols-3 gap-3 pt-2 text-xs">
          <div className="p-3 bg-[#FAF9F5] rounded-xl border border-[#E8E4DA] space-y-1">
            <div className="font-medium text-[#1F1E1D]">1. Install</div>
            <p className="text-[#78716A]">Install from Zotero Add-ons</p>
          </div>
          <div className="p-3 bg-[#FAF9F5] rounded-xl border border-[#E8E4DA] space-y-1">
            <div className="font-medium text-[#1F1E1D]">2. Right Click</div>
            <p className="text-[#78716A]">Select paper to translate</p>
          </div>
          <div className="p-3 bg-[#FAF9F5] rounded-xl border border-[#E8E4DA] space-y-1">
            <div className="font-medium text-[#1F1E1D]">3. Auto Sync</div>
            <p className="text-[#78716A]">Bilingual PDF attached</p>
          </div>
        </div>
      </div>

      {/* Footer */}
      <div className="text-center pt-8 text-xs text-[#8C8479] font-serif">
        translator · Open Source Scholarly Translation
      </div>
    </div>
  );
}
