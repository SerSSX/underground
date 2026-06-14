"use client";

import { useEffect, useState, type FormEvent } from "react";
import Shell from "../components/Shell";
import { getSettings, updateSettings, errorMessage } from "../../lib/api";

export default function PaymentsPage() {
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);

  const [mode, setMode] = useState("test");
  const [provider, setProvider] = useState("test");
  const [fee, setFee] = useState(1000);
  const [storeId, setStoreId] = useState("");
  const [variantId, setVariantId] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [keySet, setKeySet] = useState(false);

  useEffect(() => {
    getSettings()
      .then((s) => {
        setMode(s.mode);
        setProvider(s.provider);
        setFee(s.platform_fee_bps);
        setStoreId(s.ls_store_id ?? "");
        setVariantId(s.ls_variant_id ?? "");
        setKeySet(s.ls_api_key_set);
      })
      .catch((e) => setError(errorMessage(e)))
      .finally(() => setLoading(false));
  }, []);

  async function save(e: FormEvent) {
    e.preventDefault();
    setError(null);
    setSaved(false);
    const body: Record<string, unknown> = {
      mode,
      provider,
      platform_fee_bps: Number(fee),
      ls_store_id: storeId,
      ls_variant_id: variantId,
    };
    if (apiKey.trim()) body.ls_api_key = apiKey.trim();
    try {
      const s = await updateSettings(body);
      setKeySet(s.ls_api_key_set);
      setApiKey("");
      setSaved(true);
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  return (
    <Shell title="Платіжки">
      <p className="muted" style={{ marginTop: 4 }}>
        Режим: <span className="badge">{mode === "test" ? "ТЕСТОВИЙ" : "LIVE"}</span>
      </p>
      {error && <div className="error">{error}</div>}
      {loading && <p className="muted">Завантаження…</p>}
      <form className="form" onSubmit={save}>
        <label className="lbl">
          Режим
          <select className="input" value={mode} onChange={(e) => setMode(e.target.value)}>
            <option value="test">test (симуляція оплати)</option>
            <option value="live">live</option>
          </select>
        </label>
        <label className="lbl">
          Провайдер
          <select className="input" value={provider} onChange={(e) => setProvider(e.target.value)}>
            <option value="test">test</option>
            <option value="lemonsqueezy">lemonsqueezy</option>
          </select>
        </label>
        <label className="lbl">
          Комісія платформи (bps, 1000 = 10%)
          <input className="input" type="number" value={fee} onChange={(e) => setFee(Number(e.target.value))} />
        </label>
        <label className="lbl">
          Lemon Squeezy Store ID
          <input className="input" value={storeId} onChange={(e) => setStoreId(e.target.value)} />
        </label>
        <label className="lbl">
          Lemon Squeezy Variant ID
          <input className="input" value={variantId} onChange={(e) => setVariantId(e.target.value)} />
        </label>
        <label className="lbl">
          Lemon Squeezy API Key {keySet && <span className="muted">— встановлено</span>}
          <input
            className="input"
            type="password"
            placeholder={keySet ? "•••••• (залиш порожнім, щоб не змінювати)" : "sk_..."}
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
          />
        </label>
        {saved && <div className="ok">Збережено ✓</div>}
        <button className="btn">Зберегти</button>
      </form>
    </Shell>
  );
}
