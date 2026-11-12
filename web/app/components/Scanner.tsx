"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { scanTicket, errorMessage, type ScanResult } from "../../lib/api";
import { useLang, tr } from "../../lib/i18n";
import { placesLabel } from "../../lib/ui";
import { Camera, CircleCheck, CircleX, Keyboard } from "./Icon";

type BarcodeDetectorLike = { detect: (src: CanvasImageSource) => Promise<{ rawValue: string }[]> };

/** Door scanner in the browser: the phone camera reads the guest's rotating QR and the API marks the
 *  ticket used. Native BarcodeDetector where the browser has it (Chrome on Android), jsQR otherwise
 *  (Safari). Full-screen green / red verdict with the guest's name and number of places. */
export default function Scanner({ eventId }: { eventId: string }) {
  const { lang } = useLang();
  const video = useRef<HTMLVideoElement>(null);
  const canvas = useRef<HTMLCanvasElement | null>(null);
  const stream = useRef<MediaStream | null>(null);
  const busy = useRef(false);
  const last = useRef<{ code: string; at: number } | null>(null);
  const [on, setOn] = useState(false);
  const [camError, setCamError] = useState<string | null>(null);
  const [result, setResult] = useState<ScanResult | null>(null);
  const [manual, setManual] = useState("");
  const [count, setCount] = useState(0);

  const stop = useCallback(() => {
    stream.current?.getTracks().forEach((t) => t.stop());
    stream.current = null;
    setOn(false);
  }, []);
  useEffect(() => stop, [stop]);

  const verdict = useCallback(async (code: string) => {
    const now = Date.now();
    if (busy.current) return;
    if (last.current && last.current.code === code && now - last.current.at < 4000) return; // same QR still in view
    busy.current = true;
    last.current = { code, at: now };
    try {
      const r = await scanTicket(eventId, code);
      setResult(r);
      if (r.valid) setCount((c) => c + 1);
      if ("vibrate" in navigator) navigator.vibrate(r.valid ? 80 : [60, 60, 60]);
    } catch (e) {
      setResult({ valid: false, reason: "error", event_title: errorMessage(e) });
    } finally {
      setTimeout(() => { busy.current = false; setResult(null); }, 2200);
    }
  }, [eventId]);

  async function start() {
    setCamError(null);
    try {
      const s = await navigator.mediaDevices.getUserMedia({ video: { facingMode: "environment" }, audio: false });
      stream.current = s;
      if (video.current) {
        video.current.srcObject = s;
        await video.current.play();
      }
      setOn(true);
      const Native = (window as unknown as { BarcodeDetector?: new (o: { formats: string[] }) => BarcodeDetectorLike }).BarcodeDetector;
      const detector = Native ? new Native({ formats: ["qr_code"] }) : null;
      const jsQR = detector ? null : (await import("jsqr")).default;
      canvas.current = canvas.current ?? document.createElement("canvas");
      const tick = async () => {
        if (!stream.current || !video.current) return;
        const v = video.current;
        if (v.readyState >= 2 && !busy.current) {
          try {
            if (detector) {
              const codes = await detector.detect(v);
              if (codes[0]?.rawValue) void verdict(codes[0].rawValue);
            } else if (jsQR && canvas.current) {
              const w = Math.min(640, v.videoWidth), h = Math.round((v.videoHeight / v.videoWidth) * w);
              canvas.current.width = w; canvas.current.height = h;
              const ctx = canvas.current.getContext("2d", { willReadFrequently: true });
              if (ctx) {
                ctx.drawImage(v, 0, 0, w, h);
                const img = ctx.getImageData(0, 0, w, h);
                const code = jsQR(img.data, w, h, { inversionAttempts: "dontInvert" });
                if (code?.data) void verdict(code.data);
              }
            }
          } catch { /* a bad frame — try the next one */ }
        }
        if (stream.current) requestAnimationFrame(() => { void tick(); });
      };
      void tick();
    } catch {
      setCamError(tr(lang, "Немає доступу до камери. Дозвольте камеру для цього сайту або введіть код вручну.", "No camera access. Allow the camera for this site or enter the code by hand."));
      stop();
    }
  }

  const REASON: Record<string, [string, string]> = {
    already_used: ["Цей квиток уже пройшов", "Already checked in"],
    wrong_event: ["Квиток на іншу подію", "Ticket for another event"],
    returned: ["Бронь скасована", "Booking was cancelled"],
    inactive: ["Квиток недійсний", "Ticket is not valid"],
    event_cancelled: ["Подію скасовано — вхід закрито", "The event is cancelled"],
    not_found: ["Код не розпізнано або застарів — попросіть оновити QR", "Unknown or expired code — ask the guest to refresh the QR"],
  };

  return (
    <div className="stack lg">
      <div className="scanner">
        <video ref={video} playsInline muted aria-label={tr(lang, "Камера сканера", "Scanner camera")} />
        {on && <div className="frame" aria-hidden />}
        {!on && (
          <div className="scan-result" style={{ background: "transparent" }}>
            <Camera className="ico lg" />
            <p className="muted">{tr(lang, "Наведіть камеру на QR гостя", "Point the camera at the guest's QR")}</p>
            <button className="btn lg" onClick={start}>{tr(lang, "Увімкнути камеру", "Start camera")}</button>
          </div>
        )}
        {result && (
          <div className={`scan-result ${result.valid ? "ok" : "bad"}`} role="status" aria-live="assertive">
            {result.valid ? <CircleCheck style={{ width: 64, height: 64 }} /> : <CircleX style={{ width: 64, height: 64 }} />}
            <div className="big">{result.valid ? tr(lang, "Проходьте!", "Welcome in!") : tr(lang, "Не пропускати", "Do not admit")}</div>
            {result.holder_name && <div style={{ fontSize: 20, fontWeight: 700 }}>{result.holder_name}</div>}
            {result.valid && result.places != null && result.places > 1 && (
              <div>{tr(lang, `${placesLabel(result.places, lang)} у броні · пройшли ${result.checked_in ?? 1}`, `${result.places} places booked · ${result.checked_in ?? 1} in`)}</div>
            )}
            {!result.valid && <div>{REASON[result.reason] ? tr(lang, ...REASON[result.reason]) : result.event_title}</div>}
          </div>
        )}
      </div>
      {camError && <div className="alert warn">{camError}</div>}
      <div className="row spread">
        <span className="muted small">{tr(lang, `Пропущено за цю сесію: ${count}`, `Admitted this session: ${count}`)}</span>
        {on && <button className="btn ghost sm" onClick={stop}>{tr(lang, "Вимкнути камеру", "Stop camera")}</button>}
      </div>
      <form className="row gap" onSubmit={(e) => { e.preventDefault(); if (manual.trim()) { void verdict(manual.trim()); setManual(""); } }}>
        <label className="input-icon grow">
          <span className="sr-only">{tr(lang, "Код з квитка", "Ticket code")}</span>
          <Keyboard className="ico" />
          <input className="input" value={manual} onChange={(e) => setManual(e.target.value)} placeholder={tr(lang, "Або вставте код з QR вручну", "Or paste the QR code by hand")} />
        </label>
        <button className="btn secondary" disabled={!manual.trim()}>{tr(lang, "Перевірити", "Check")}</button>
      </form>
    </div>
  );
}
