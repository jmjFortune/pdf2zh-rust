"use client";

import React from "react";
import {
  Languages,
  BookOpen,
  Library,
  Settings
} from "lucide-react";

export default function Sidebar({ currentTab, onSelectTab, caringQuote, statusLabel, ready }) {
  const navItems = [
    { id: "translate", label: "Translate", icon: Languages },
    { id: "reader", label: "Reader", icon: BookOpen },
    { id: "library", label: "Library", icon: Library },
    { id: "settings", label: "Settings", icon: Settings },
  ];

  return (
    <aside className="w-60 bg-[#F5F3ED] border-r border-[#E8E4DA] flex flex-col justify-between h-screen select-none shrink-0">
      <div>
        {/* Navigation Items */}
        <nav className="px-3.5 space-y-1 pt-5">
          {navItems.map((item) => {
            const Icon = item.icon;
            const isActive = currentTab === item.id;
            return (
              <button
                key={item.id}
                onClick={() => onSelectTab(item.id)}
                className={`w-full flex items-center space-x-3 px-3.5 py-2.5 rounded-xl text-sm font-medium transition-all ${
                  isActive
                    ? "bg-[#FFFFFF] text-[#1F1E1D] shadow-[0_1px_3px_rgba(40,30,20,0.06)] border border-[#E2DBD0]"
                    : "text-[#6E6961] hover:text-[#1F1E1D] hover:bg-[#EAE5D9]/60"
                }`}
              >
                <Icon
                  className={`w-4 h-4 ${
                    isActive ? "text-[#D97757]" : "text-[#78716A]"
                  }`}
                />
                <span>{item.label}</span>
              </button>
            );
          })}
        </nav>
      </div>

      {/* Bottom Status */}
      <div className="p-4 border-t border-[#E8E4DA] space-y-3">
        <div className="flex items-center space-x-2 text-xs text-[#6E6961]">
          <span className={`w-2 h-2 rounded-full ${ready ? "bg-[#10B981]" : "bg-[#9B9488]"}`} />
          <span>{statusLabel}</span>
        </div>

        {caringQuote && (
          <p className="text-xs font-serif text-[#78716A] leading-relaxed italic border-l-2 border-[#D97757]/40 pl-2.5">
            "{caringQuote.text}"
          </p>
        )}
      </div>
    </aside>
  );
}
