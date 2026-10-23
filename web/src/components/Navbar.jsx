"use client";

import React from "react";
import { BookOpen } from "lucide-react";

export default function Navbar({ currentTab, onSelectTab, onOpenReader, hasPaper }) {
  const tabNames = {
    translate: "Translate",
    reader: "Reader",
    library: "Library",
    settings: "Settings",
    about: "About",
  };

  return (
    <header className="h-14 bg-[#FAF9F5] border-b border-[#E8E4DA] px-8 flex items-center justify-between shrink-0 select-none">
      <div className="flex items-center space-x-2 text-sm">
        <span className="text-[#8C8479]">translator</span>
        <span className="text-[#D5CFC2]">/</span>
        <span className="font-serif font-semibold text-[#1F1E1D]">
          {tabNames[currentTab] || "Workspace"}
        </span>
      </div>

      <div className="flex items-center space-x-4">
        {currentTab !== "reader" && (
          <button
            onClick={onOpenReader}
            disabled={!hasPaper}
            className="flex items-center space-x-1.5 px-3.5 py-1.5 rounded-xl bg-white hover:bg-[#FAF9F5] text-xs font-medium text-[#1F1E1D] border border-[#E2DBD0] shadow-2xs transition-colors"
          >
            <BookOpen className="w-3.5 h-3.5 text-[#D97757]" />
            <span>Open Reader</span>
          </button>
        )}
      </div>
    </header>
  );
}
