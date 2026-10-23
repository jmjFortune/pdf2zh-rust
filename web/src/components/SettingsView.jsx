"use client";

import React, { useState, useEffect, useRef } from "react";
import { Plus, Trash2, Save, CheckCircle2 } from "lucide-react";
import { api } from "../lib/api";

export default function SettingsView({ settings, onSettingsChange, loadError, onRetry }) {
  const [activeTab, setActiveTab] = useState("api");
  const [serviceId, setServiceId] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [clearKey, setClearKey] = useState(false);
  const [baseUrl, setBaseUrl] = useState("");
  const [selectedModel, setSelectedModel] = useState("");
  const [providerOptions, setProviderOptions] = useState({});
  const [glossary, setGlossary] = useState([]);
  const [newTermEn, setNewTermEn] = useState("");
  const [newTermZh, setNewTermZh] = useState("");
  const [savedNotification, setSavedNotification] = useState(false);
  const [busy, setBusy] = useState(false);
  const [loadingGlossary, setLoadingGlossary] = useState(true);
  const [error, setError] = useState("");
  const timer = useRef(null);
  const service = settings?.services.find((item) => item.id === serviceId);
  const termsFrom = (payload) => Object.entries(payload.terms).map(([term, zh]) => ({ term, zh }));

  useEffect(() => {
    if (!settings) return;
    setServiceId(settings.defaults.service);
  }, [settings]);

  useEffect(() => {
    if (!service) return;
    setApiKey("");
    setClearKey(false);
    setBaseUrl(service.base_url || "");
    setSelectedModel(service.model || "");
    setProviderOptions(service.options || {});
  }, [service]);

  useEffect(() => {
    let active = true;
    api.glossary().then((payload) => { if (active) setGlossary(termsFrom(payload)); })
      .catch((err) => { if (active) setError(err.message); })
      .finally(() => { if (active) setLoadingGlossary(false); });
    return () => { active = false; clearTimeout(timer.current); };
  }, []);

  const handleAddTerm = async () => {
    if (!newTermEn.trim() || !newTermZh.trim() || busy) return;
    setBusy(true); setError("");
    try {
      setGlossary(termsFrom(await api.addTerm(newTermEn.trim(), newTermZh.trim())));
      setNewTermEn(""); setNewTermZh("");
    } catch (err) { setError(err.message); }
    finally { setBusy(false); }
  };

  const handleDeleteTerm = async (term) => {
    setBusy(true); setError("");
    try { setGlossary(termsFrom(await api.deleteTerm(term))); }
    catch (err) { setError(err.message); }
    finally { setBusy(false); }
  };

  const handleSaveAll = async () => {
    if (!service || busy) return;
    setBusy(true); setError(""); setSavedNotification(false);
    try {
      const patch = { id: serviceId, base_url: baseUrl, model: selectedModel, options: providerOptions };
      if (clearKey) patch.api_key = "";
      else if (apiKey.trim()) patch.api_key = apiKey.trim();
      const next = await api.saveSettings({
        services: [patch], defaults: { ...settings.defaults, service: serviceId },
      });
      onSettingsChange(next);
      setApiKey("");
      setSavedNotification(true);
      clearTimeout(timer.current);
      timer.current = setTimeout(() => setSavedNotification(false), 2000);
    } catch (err) { setError(err.message); }
    finally { setBusy(false); }
  };

  return (
    <div className="max-w-3xl mx-auto space-y-6 py-4">
      {/* Header */}
      <div className="flex items-end justify-between">
        <div>
          <h1 className="font-serif text-3xl font-bold text-[#1F1E1D] tracking-tight">
            Settings
          </h1>
          <p className="text-sm text-[#78716A] mt-2 font-serif">
            Configure model credentials and terminology glossary.
          </p>
        </div>

        <button
          onClick={handleSaveAll}
          disabled={busy || !settings}
          className="flex items-center space-x-1.5 px-5 py-2.5 rounded-xl bg-[#D97757] hover:bg-[#C15F3C] text-white text-xs font-medium transition-all"
        >
          {savedNotification ? <CheckCircle2 className="w-4 h-4" /> : <Save className="w-4 h-4" />}
          <span>{savedNotification ? "Saved" : "Save Settings"}</span>
        </button>
      </div>

      {(error || loadError) && <p role="alert" className="text-sm text-[#DC2626]">{error || loadError}{loadError && <button onClick={onRetry} className="ml-3 underline">Retry connection</button>}</p>}
      {!settings && !loadError && <p role="status" className="text-sm text-[#78716A]">Loading settings…</p>}
      {/* Tabs */}
      <div className="flex space-x-2 border-b border-[#E8E4DA] pb-1 text-sm">
        {[
          { id: "api", label: "API Credentials" },
          { id: "glossary", label: "Glossary" },
        ].map((tab) => {
          const isActive = activeTab === tab.id;
          return (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              className={`px-4 py-2 rounded-xl transition-all ${
                isActive
                  ? "bg-white text-[#1F1E1D] font-medium shadow-2xs border border-[#E2DBD0]"
                  : "text-[#6E6961] hover:text-[#1F1E1D]"
              }`}
            >
              {tab.label}
            </button>
          );
        })}
      </div>

      {/* Tab: API */}
      {activeTab === "api" && (
        <div className="p-6 rounded-2xl bg-white border border-[#E8E4DA] space-y-5">
          <div className="space-y-1.5">
            <label className="text-xs text-[#78716A] font-medium" htmlFor="settings-service">Translation Service / Default</label>
            <select id="settings-service" value={serviceId} onChange={(e) => setServiceId(e.target.value)} disabled={busy || !settings} className="w-full text-sm bg-[#FAF9F5] border border-[#DCD6C9] rounded-xl px-3 py-2 text-[#1F1E1D]">
              {(settings?.services || []).map((item) => <option key={item.id} value={item.id}>{item.name}</option>)}
            </select>
            {service?.note && <p className="text-xs text-[#78716A]">{service.note}</p>}
          </div>
          <div className="space-y-1.5">
            <label className="text-xs text-[#78716A] font-medium" htmlFor="settings-key">{service?.name || "Service"} API Key</label>
            <input
              id="settings-key"
              type="password"
              autoComplete="new-password"
              value={apiKey}
              placeholder={service?.has_key && (service?.needs_key || service?.api_key_optional) ? "Key configured — leave blank to keep" : service?.needs_key ? "Enter API key" : service?.api_key_optional ? "Optional API key" : "No API key required"}
              disabled={(!service?.needs_key && !service?.api_key_optional) || busy}
              onChange={(e) => { setApiKey(e.target.value); setClearKey(false); }}
              className="w-full text-sm font-mono bg-[#FAF9F5] border border-[#DCD6C9] rounded-xl px-3 py-2 text-[#1F1E1D] focus:outline-none focus:ring-1 focus:ring-[#D97757]"
            />
          </div>

          {(service?.needs_key || service?.api_key_optional) && service?.has_key && <button type="button" disabled={busy} onClick={() => { setClearKey((value) => !value); setApiKey(""); }} className="text-xs text-[#78716A] underline">{clearKey ? "Keep stored key" : "Remove stored key on save"}</button>}
          <div className="space-y-1.5">
            <label className="text-xs text-[#78716A] font-medium" htmlFor="settings-url">API Base URL</label>
            <input
              type="text"
              id="settings-url"
              placeholder={service?.default_base_url || "Provider default"}
              disabled={busy}
              value={baseUrl}
              onChange={(e) => setBaseUrl(e.target.value)}
              className="w-full text-sm font-mono bg-[#FAF9F5] border border-[#DCD6C9] rounded-xl px-3 py-2 text-[#1F1E1D] focus:outline-none focus:ring-1 focus:ring-[#D97757]"
            />
          </div>

          {service?.requires_model && <div className="space-y-1.5">
            <label className="text-xs text-[#78716A] font-medium" htmlFor="settings-model">{serviceId === "azure-openai" ? "Deployment Name" : "Default Model"}</label>
            <input
              id="settings-model"
              type="text"
              value={selectedModel}
              disabled={busy}
              placeholder={service?.default_model || "Enter an available model ID"}
              onChange={(e) => setSelectedModel(e.target.value)}
              className="w-full text-sm bg-[#FAF9F5] border border-[#DCD6C9] rounded-xl px-3 py-2 text-[#1F1E1D] focus:outline-none focus:ring-1 focus:ring-[#D97757]"
            />
          </div>}
          {(service?.option_fields || []).map((field) => <div key={field.key} className="space-y-1.5">
            <label className="text-xs text-[#78716A] font-medium" htmlFor={`settings-${field.key}`}>{field.label}</label>
            <input id={`settings-${field.key}`} type="text" value={providerOptions[field.key] || ""} disabled={busy}
              placeholder={field.default || ""} onChange={(e) => setProviderOptions((current) => ({...current, [field.key]: e.target.value}))}
              className="w-full text-sm font-mono bg-[#FAF9F5] border border-[#DCD6C9] rounded-xl px-3 py-2 text-[#1F1E1D] focus:outline-none focus:ring-1 focus:ring-[#D97757]" />
          </div>)}
        </div>
      )}

      {/* Tab: Glossary */}
      {activeTab === "glossary" && (
        <div className="p-6 rounded-2xl bg-white border border-[#E8E4DA] space-y-4">
          <div className="flex items-center space-x-2">
            <input
              type="text"
              placeholder="Source Term"
              value={newTermEn}
              onChange={(e) => setNewTermEn(e.target.value)}
              className="flex-1 text-xs bg-[#FAF9F5] border border-[#DCD6C9] rounded-xl px-3 py-2 text-[#1F1E1D]"
            />
            <input
              type="text"
              placeholder="Target Translation"
              value={newTermZh}
              onChange={(e) => setNewTermZh(e.target.value)}
              className="flex-1 text-xs bg-[#FAF9F5] border border-[#DCD6C9] rounded-xl px-3 py-2 text-[#1F1E1D]"
            />
            <button
              onClick={handleAddTerm}
              disabled={busy || !newTermEn.trim() || !newTermZh.trim()}
              className="flex items-center space-x-1 px-4 py-2 bg-[#D97757] hover:bg-[#C15F3C] text-white text-xs font-medium rounded-xl transition-all"
            >
              <Plus className="w-3.5 h-3.5" />
              <span>Add</span>
            </button>
          </div>

          <div className="border border-[#E8E4DA] rounded-xl overflow-hidden divide-y divide-[#F0ECE1]">
            {loadingGlossary && <p role="status" className="px-4 py-3 text-xs text-[#78716A]">Loading glossary…</p>}
            {!loadingGlossary && !glossary.length && <p className="px-4 py-3 text-xs text-[#78716A]">No terms yet.</p>}
            {glossary.map((item, idx) => (
              <div
                key={idx}
                className="px-4 py-3 flex items-center justify-between text-xs hover:bg-[#FAF9F5] transition-colors"
              >
                <div className="flex items-center space-x-4">
                  <span className="font-mono text-[#1F1E1D]">{item.term}</span>
                  <span className="text-[#8C8479]">→</span>
                  <span className="font-serif font-bold text-[#2D6A4F]">{item.zh}</span>
                </div>
                <button
                  onClick={() => handleDeleteTerm(item.term)}
                  disabled={busy}
                  aria-label={`Delete term ${item.term}`}
                  className="text-[#9B9488] hover:text-[#DC2626] p-1"
                >
                  <Trash2 className="w-3.5 h-3.5" />
                </button>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
