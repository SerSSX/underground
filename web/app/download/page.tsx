"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import QRCode from "qrcode";
import { getAndroidRelease, type AndroidRelease } from "../../lib/app";
import { useLang, tr, tf } from "../../lib/i18n";
import { Download, Smartphone, ShieldIcon, Compass } from "./icons";

/** Download page for the Android app served from our own server (no Google Play yet). */
export default function AppPage() {
  const { lang } = useLang();
  const [rel, setRel] = useState<AndroidRelease | null | undefined>(undefined);
  const [qr, setQr] = useState<string | null>(null);
  const [android, setAndroid] = useState(false);

  useEffect(() => {
    setAndroid(/android/i.test(navigator.userAgent));
    getAndroidRelease().then(setRel);
    QRCode.toDataURL(`${window.location.origin}/download`, { margin: 0, width: 360, color: { dark: "#08090d", light: "#ffffff" } }).then(setQr).catch(() => {});
  }, []);

  const mb = rel?.size_bytes ? tf(lang, "{size} МБ", "{size} MB", { size: (rel.size_bytes / 1024 / 1024).toFixed(1) }) : null;

  return (
    <main className="container narrow">
      <div className="card" style={{ marginTop: 16, padding: 28, borderRadius: "var(--r-xl)", background: "radial-gradient(120% 120% at 0% 0%, rgba(76,111,255,.22), transparent 60%), var(--surface)" }}>
        <div className="row gap" style={{ marginBottom: 10 }}><Smartphone className="ico lg" /><span className="muted strong">Android</span></div>
        <h1 style={{ fontSize: 32 }}>{tr(lang, "Underground у телефоні", "Underground on your phone")}</h1>
        <p className="muted" style={{ marginTop: 10, maxWidth: 540 }}>{tr(lang, "Афіша, бронювання, квитки з QR і сканер для організаторів — у застосунку.", "Events, booking, QR tickets and a scanner for organisers — in an app.")}</p>

        {rel === undefined ? (
          <div className="skeleton" style={{ height: 52, width: 240, marginTop: 22 }} />
        ) : rel === null ? (
          <div className="alert" style={{ marginTop: 22 }}>{tr(lang, "Застосунок готується. А поки все працює і в браузері телефона — просто відкрийте цей сайт.", "The app is on its way. Meanwhile everything works in your phone's browser — just open this site.")}</div>
        ) : (
          <div className="row gap wrap" style={{ marginTop: 22, alignItems: "center" }}>
            <a className="btn lg primary-glow" href={rel.url} download><Download className="ico sm" />{tr(lang, "Завантажити APK", "Download APK")}</a>
            <span className="muted small">{tr(lang, "Версія", "Version")} {rel.version}{mb ? ` · ${mb}` : ""}{rel.min_android ? ` · Android ${rel.min_android}+` : ""}</span>
          </div>
        )}
      </div>

      {rel && (
        <div className={android ? "stack" : "form-layout narrow-aside"} style={{ marginTop: 24 }}>
          <section className="card">
            <h2 style={{ fontSize: 18, marginBottom: 12 }}>{tr(lang, "Як встановити", "How to install")}</h2>
            <ol className="stack" style={{ paddingLeft: 20, margin: 0 }}>
              <li>{tr(lang, "Натисніть «Завантажити APK» на телефоні з Android.", "Tap Download APK on your Android phone.")}</li>
              <li>{tr(lang, "Відкрийте завантажений файл. Якщо телефон спитає — дозвольте встановлення з цього джерела (браузера).", "Open the downloaded file. If asked, allow installs from this source (your browser).")}</li>
              <li>{tr(lang, "Натисніть «Встановити» й увійдіть тим самим акаунтом, що й на сайті.", "Tap Install and sign in with the same account as on the site.")}</li>
            </ol>
            <div className="alert" style={{ marginTop: 16 }}><ShieldIcon className="ico" />{tr(lang, "Файл завантажується лише з нашого сервера. Google Play поки немає — коли зʼявиться, оновлення прийдуть звідти.", "The file comes only from our server. There is no Google Play listing yet.")}</div>
            {rel.sha256 && <p className="subtle xs" style={{ marginTop: 10, wordBreak: "break-all" }}>SHA-256: {rel.sha256}</p>}
          </section>
          {!android && qr && (
            <aside className="card" style={{ textAlign: "center" }}>
              <div className="qr-box" style={{ width: 180, padding: 12 }}><img src={qr} alt={tr(lang, "QR-код сторінки завантаження", "QR code for this page")} /></div>
              <p className="muted small" style={{ marginTop: 12 }}>{tr(lang, "Наведіть камеру телефона, щоб відкрити цю сторінку на ньому", "Scan with your phone to open this page there")}</p>
            </aside>
          )}
        </div>
      )}
      <div style={{ marginTop: 24 }}><Link className="btn quiet" href="/"><Compass className="ico sm" />{tr(lang, "До афіші", "Browse events")}</Link></div>
    </main>
  );
}
