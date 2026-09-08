"use client";

import { useEffect, useState, type FormEvent } from "react";
import {
  errorMessage,
  getReadiness,
  testStripe,
  updateSettings,
  type ConnectionCheck,
  type PaymentReadiness,
  type PaymentSettings,
} from "../../lib/api";

/**
 * Connecting Stripe from the platform admin.
 *
 * Two rules shape this screen. Secrets are write-only: the server returns whether a key is
 * stored, never the key, so the inputs start empty and an empty input means "leave as is" —
 * otherwise saving any other field would silently wipe the credentials. And the state that
 * decides whether money can move (key kind vs mode, webhook secret present, provider
 * recognised) is shown as a checklist rather than inferred by the operator from a form.
 */
export default function StripeSection({
  settings,
  onSaved,
}: {
  settings: PaymentSettings;
  onSaved: (s: PaymentSettings) => void;
}) {
  const [secret, setSecret] = useState("");
  const [publishable, setPublishable] = useState(settings.stripe_publishable_key ?? "");
  const [webhook, setWebhook] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const [readiness, setReadiness] = useState<PaymentReadiness | null>(null);
  const [check, setCheck] = useState<ConnectionCheck | null>(null);

  useEffect(() => {
    getReadiness().then(setReadiness).catch(() => setReadiness(null));
  }, [settings.updated_at]);

  async function save(e: FormEvent) {
    e.preventDefault();
    setError(null);
    setSaved(false);
    setBusy(true);
    try {
      const body: Record<string, unknown> = { stripe_publishable_key: publishable.trim() };
      // Only send a secret when one was typed. Sending "" would clear a working key.
      if (secret.trim()) body.stripe_secret_key = secret.trim();
      if (webhook.trim()) body.stripe_webhook_secret = webhook.trim();
      const s = await updateSettings(body);
      onSaved(s);
      setSecret("");
      setWebhook("");
      setSaved(true);
      setCheck(null);
      setReadiness(await getReadiness());
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  async function runCheck() {
    setBusy(true);
    setCheck(null);
    try {
      setCheck(await testStripe());
    } catch (err) {
      setCheck({ ok: false, account_id: null, message: errorMessage(err) });
    } finally {
      setBusy(false);
    }
  }

  // Revocation. "Empty means leave as is" protects a working key from an accidental save,
  // but it also meant a leaked key could only be removed by hand in the database. The server
  // clears on an explicit "" — this is the only path that sends one, and it asks first.
  async function clearKey(field: "stripe_secret_key" | "stripe_webhook_secret") {
    const what = field === "stripe_secret_key" ? "секретний ключ" : "секрет вебхука";
    if (!window.confirm(`Видалити ${what}? Платежі через Stripe зупиняться, доки не введете новий.`)) return;
    setBusy(true);
    setError(null);
    try {
      const s = await updateSettings({ [field]: "" });
      onSaved(s);
      setCheck(null);
      setReadiness(await getReadiness());
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  const lastWebhook = readiness?.stripe_last_webhook_at
    ? new Date(readiness.stripe_last_webhook_at).toLocaleString("uk-UA")
    : null;
  const rejections = readiness?.stripe_webhook_rejections ?? 0;

  const mismatch =
    settings.stripe_key_kind !== null &&
    settings.stripe_key_kind !== "unknown" &&
    settings.stripe_key_kind !== settings.mode;

  const rows: { label: string; ok: boolean; hint?: string }[] = [
    {
      label: "Секретний ключ збережено",
      ok: settings.stripe_secret_key_set,
      hint: settings.stripe_key_kind ? `ключ ${settings.stripe_key_kind}` : undefined,
    },
    {
      label: "Секрет вебхука збережено",
      ok: settings.stripe_webhook_secret_set,
      hint: "без нього ми не дізнаємось, що покупець заплатив",
    },
    {
      // A stored secret that is WRONG rejects every delivery with 401 — silently, forever.
      // Only a delivery that actually passed verification proves the endpoint works.
      label: "Вебхук підтверджений",
      ok: lastWebhook !== null,
      hint: lastWebhook
        ? `остання валідна доставка ${lastWebhook}` + (rejections ? `, відхилено після неї: ${rejections}` : "")
        : rejections
          ? `жодної валідної доставки, відхилено ${rejections} — секрет не той або годинник поїхав`
          : "ще не було жодної доставки від Stripe",
    },
    {
      label: "Провайдер розпізнаний",
      ok: readiness?.provider_recognised ?? false,
      hint: "інакше кожен checkout повертає 409",
    },
    {
      label: "Готово до live",
      ok: readiness?.ready_for_live ?? false,
      hint: settings.mode === "test" ? "у тестовому режимі не обов'язково" : undefined,
    },
  ];

  return (
    <section className="card" style={{ marginTop: 24 }}>
      <h2 style={{ margin: "0 0 4px", fontSize: 18 }}>Stripe</h2>
      <p className="muted" style={{ marginTop: 0 }}>
        Ключі беруться в Stripe Dashboard → Developers → API keys. Секрет вебхука — там же, у
        розділі Webhooks, після додавання ендпоінта <code>/api/webhooks/stripe</code>.
      </p>

      {mismatch && (
        <div className="error">
          Ключ <b>{settings.stripe_key_kind}</b>, а режим <b>{settings.mode}</b>. Live-режим із
          тестовим ключем не спише з покупця нічого, хоча всі екрани показуватимуть, що оплата
          пройшла.
        </div>
      )}

      <ul className="checklist" style={{ listStyle: "none", padding: 0, margin: "16px 0" }}>
        {rows.map((r) => (
          <li key={r.label} style={{ display: "flex", gap: 10, alignItems: "baseline", padding: "6px 0" }}>
            <span aria-hidden="true" style={{ color: r.ok ? "var(--ok, #2FAE5A)" : "var(--muted, #888)" }}>
              {r.ok ? "✔" : "○"}
            </span>
            <span>
              {r.label}
              {r.hint && <span className="muted"> — {r.hint}</span>}
            </span>
            <span className="sr-only">{r.ok ? "виконано" : "не виконано"}</span>
          </li>
        ))}
      </ul>

      {error && <div className="error">{error}</div>}
      {saved && <p className="muted">Збережено.</p>}

      <form className="form" onSubmit={save}>
        <label className="lbl">
          Публічний ключ (pk_…)
          <input
            className="input"
            value={publishable}
            onChange={(e) => setPublishable(e.target.value)}
            placeholder="pk_test_…"
            autoComplete="off"
          />
        </label>
        <label className="lbl">
          Секретний ключ (sk_…)
          <input
            className="input"
            type="password"
            value={secret}
            onChange={(e) => setSecret(e.target.value)}
            placeholder={settings.stripe_secret_key_set ? "збережено — введіть, щоб замінити" : "sk_test_…"}
            autoComplete="off"
          />
        </label>
        <label className="lbl">
          Секрет вебхука (whsec_…)
          <input
            className="input"
            type="password"
            value={webhook}
            onChange={(e) => setWebhook(e.target.value)}
            placeholder={settings.stripe_webhook_secret_set ? "збережено — введіть, щоб замінити" : "whsec_…"}
            autoComplete="off"
          />
        </label>
        <div style={{ display: "flex", gap: 10, flexWrap: "wrap" }}>
          <button className="btn" type="submit" disabled={busy}>
            {busy ? "Зберігаю…" : "Зберегти"}
          </button>
          <button
            className="btn secondary"
            type="button"
            onClick={runCheck}
            disabled={busy || !settings.stripe_secret_key_set}
            title={settings.stripe_secret_key_set ? undefined : "спершу збережіть секретний ключ"}
          >
            Перевірити зв'язок
          </button>
          {settings.stripe_secret_key_set && (
            <button className="btn secondary" type="button" onClick={() => clearKey("stripe_secret_key")} disabled={busy}>
              Видалити секретний ключ
            </button>
          )}
          {settings.stripe_webhook_secret_set && (
            <button className="btn secondary" type="button" onClick={() => clearKey("stripe_webhook_secret")} disabled={busy}>
              Видалити секрет вебхука
            </button>
          )}
        </div>
      </form>

      {check && (
        <p className={check.ok ? "muted" : "error"} style={{ marginTop: 12 }} role="status">
          {check.ok ? "✔ " : "✖ "}
          {check.message}
        </p>
      )}
    </section>
  );
}
