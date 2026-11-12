"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import QRCode from "qrcode";
import { useLang, tr } from "../../lib/i18n";
import { getAndroidRelease, type AndroidRelease } from "../../lib/app";
import { Smartphone, Download } from "./Icon";

/** "Underground for Android" with a QR to the download page. Renders nothing until the server
 *  actually serves an APK (/uploads/app/latest.json, published by deploy/deploy-dev.sh): the site never offers an app it lacks. */
export default function AppBanner() {
  const { lang } = useLang();
  const [rel, setRel] = useState<AndroidRelease | null>(null);
  const [qr, setQr] = useState<string | null>(null);

  useEffect(() => {
    getAndroidRelease().then(setRel);
  }, []);
  useEffect(() => {
    if (!rel) return;
    QRCode.toDataURL(`${window.location.origin}/download`, { margin: 0, width: 240, color: { dark: "#08090d", light: "#ffffff" } })
      .then(setQr)
      .catch(() => setQr(null));
  }, [rel]);

  if (!rel) return null;
  return (
    <section className="appbanner" aria-label={tr(lang, "Застосунок для Android", "Android app")}>
      {qr ? <div className="qr"><img src={qr} alt={tr(lang, "QR-код для завантаження", "Download QR code")} /></div> : <span />}
      <div>
        <div className="row gap" style={{ marginBottom: 6 }}><Smartphone className="ico" /><h3>{tr(lang, "Underground для Android", "Underground for Android")}</h3></div>
        <p className="muted">{tr(lang, "Афіша, квитки з QR і сканер на вході — у застосунку. Наведіть камеру на QR або натисніть «Завантажити».", "Events, QR tickets and the door scanner in an app. Point your camera at the QR or tap Download.")}</p>
      </div>
      <Link href="/download" className="btn"><Download className="ico sm" />{tr(lang, "Завантажити", "Download")}</Link>
    </section>
  );
}
