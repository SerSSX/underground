"use client";

import { useEffect, useRef, type ReactNode } from "react";
import { X } from "./Icon";

// Accessible dialog: focus moves in on open, Escape / scrim click close, role=dialog + aria-modal.
// Under 560 px it renders as a bottom sheet (globals.css).
export default function Modal({ title, onClose, children }: { title: string; onClose: () => void; children: ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);
  const close = useRef(onClose);
  close.current = onClose;
  // Runs once per opening: focus in, Escape closes, Tab stays inside, body does not scroll, focus goes back after.
  useEffect(() => {
    const back = document.activeElement as HTMLElement | null;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close.current();
      if (e.key === "Tab" && ref.current) {
        const f = ref.current.querySelectorAll<HTMLElement>("button, [href], input, select, textarea, [tabindex]:not([tabindex='-1'])");
        if (f.length === 0) return;
        const first = f[0], lastEl = f[f.length - 1];
        if (e.shiftKey && document.activeElement === first) { e.preventDefault(); lastEl.focus(); }
        else if (!e.shiftKey && document.activeElement === lastEl) { e.preventDefault(); first.focus(); }
      }
    };
    document.addEventListener("keydown", onKey);
    const prev = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    ref.current?.querySelector<HTMLElement>("input, textarea, select, [data-autofocus]")?.focus();
    return () => {
      document.removeEventListener("keydown", onKey);
      document.body.style.overflow = prev;
      back?.focus?.();
    };
  }, []);

  return (
    <div className="modal-scrim" onClick={onClose}>
      <div className="modal" ref={ref} role="dialog" aria-modal aria-label={title} onClick={(e) => e.stopPropagation()}>
        <div className="modal-head">
          <h3>{title}</h3>
          <button className="btn quiet icon sm" onClick={onClose} aria-label="Закрити"><X className="ico" /></button>
        </div>
        {children}
      </div>
    </div>
  );
}
