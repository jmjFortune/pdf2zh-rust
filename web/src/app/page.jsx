"use client";

import React, { useState, useEffect, useCallback } from "react";
import Sidebar from "../components/Sidebar";
import Navbar from "../components/Navbar";
import TranslateView from "../components/TranslateView";
import ReaderView from "../components/ReaderView";
import LibraryView from "../components/LibraryView";
import SettingsView from "../components/SettingsView";
import { api } from "../lib/api";
import { CARING_QUOTES } from "../data/mockData";

export default function Home() {
  const [currentTab, setCurrentTab] = useState("translate");
  const [quoteIndex] = useState(0);
  const [settings, setSettings] = useState(null);
  const [health, setHealth] = useState(null);
  const [error, setError] = useState("");
  const [paperId, setPaperId] = useState(null);
  const [libraryVersion, setLibraryVersion] = useState(0);
  const [resumeJobId, setResumeJobId] = useState(null);
  const [removedPaperId, setRemovedPaperId] = useState(null);

  const loadSettings = useCallback(async () => {
    try {
      const [nextSettings, nextHealth] = await Promise.all([api.settings(), api.health()]);
      setSettings(nextSettings);
      setHealth(nextHealth);
      setError("");
    } catch (err) {
      setError(err.message);
    }
  }, []);

  useEffect(() => { loadSettings(); }, [loadSettings]);

  const selectTab = (tab) => {
    setCurrentTab(tab);
    sessionStorage.setItem("pdf2zh.tab", tab);
  };

  const openReader = (id) => {
    const selectedId = typeof id === "string" ? id : paperId;
    if (selectedId) {
      setPaperId(selectedId);
      sessionStorage.setItem("pdf2zh.paper", selectedId);
    }
    selectTab("reader");
  };

  const service = settings?.services.find((item) => item.id === settings.defaults.service);
  const statusLabel = error ? "Backend unavailable" : !health ? "Connecting…"
    : health.mock ? "Mock backend ready"
    : service?.needs_key && !service.has_key ? `${service.name} needs an API key`
    : `${service?.name || "Backend"} ready`;

  React.useEffect(() => {
    if (typeof window !== "undefined") {
      const params = new URLSearchParams(window.location.search);
      setPaperId(params.get("paper") || sessionStorage.getItem("pdf2zh.paper"));
      const tab = params.get("tab") || sessionStorage.getItem("pdf2zh.tab");
      if (tab && ["translate", "reader", "library", "settings"].includes(tab)) {
        setCurrentTab(tab);
      } else if (tab) {
        sessionStorage.setItem("pdf2zh.tab", "translate");
      }
    }
  }, []);

  const caringQuote = CARING_QUOTES[quoteIndex % CARING_QUOTES.length];

  return (
    <div className="flex h-screen overflow-hidden bg-[#FAF9F5] text-[#1F1E1D]">
      {/* Claude Style Left Sidebar */}
      <Sidebar
        currentTab={currentTab}
        onSelectTab={selectTab}
        caringQuote={caringQuote}
        statusLabel={statusLabel}
        ready={Boolean(health) && !error}
      />

      {/* Main Content Area */}
      <div className="flex-1 flex flex-col min-w-0 overflow-hidden">
        <Navbar currentTab={currentTab} onSelectTab={selectTab} onOpenReader={() => openReader()} hasPaper={Boolean(paperId)} />

        {/* Scrollable View Area */}
        <main className={`flex-1 min-h-0 claude-paper-texture ${currentTab === "reader" ? "overflow-hidden" : "overflow-y-auto p-6 lg:p-8"}`}>
          <div hidden={currentTab !== "translate"}>
            <TranslateView
              settings={settings}
              settingsError={error}
              onRetry={loadSettings}
              onOpenReader={openReader}
              onAddToHistory={() => setLibraryVersion((value) => value + 1)}
              removedPaperId={removedPaperId}
              resumeJobId={resumeJobId}
            />
          </div>

          {currentTab === "reader" && <ReaderView paperId={paperId} onSelectLibrary={() => selectTab("library")} />}

          {currentTab === "library" && (
            <LibraryView onResume={(id) => { setResumeJobId(null); setTimeout(() => setResumeJobId(id), 0); selectTab("translate"); }} version={libraryVersion} onOpenReader={openReader} onDeleted={(id) => {
              setRemovedPaperId(id);
              if (id === paperId) { setPaperId(null); sessionStorage.removeItem("pdf2zh.paper"); }
            }} />
          )}

          {currentTab === "settings" && <SettingsView settings={settings} onSettingsChange={setSettings} loadError={error} onRetry={loadSettings} />}

        </main>
      </div>
    </div>
  );
}
