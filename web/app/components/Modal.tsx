"use client";

import { useEffect, useRef, type ReactNode } from "react";
import { X } from "./Icon";

// Accessible dialog: focus moves in on open, Escape / scrim click close, role=dialog + aria-modal.
// Under 560 px it renders as a bottom sheet (globals.css).
export default function Modal({ title, onClose, children }: { title: string; onClose: () => void; children: ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    document.addEventListener("keydown", onKey);
    const prev = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    ref.current?.querySelector<HTMLElement>("input, textarea, select, [data-autofocus]")?.focus();
    return () => {
      document.removeEventListener("keydown", onKey);
      document.body.style.overflow = prev;
    };
  }, [onClose]);

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
