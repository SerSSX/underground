"use client";

// A text input with place suggestions from the geocoder (TASK-303 G2): ARIA combobox + listbox, debounced
// requests that abort when the text changes, keyboard and mouse picking, OpenStreetMap attribution.
// Geocoder strings are rendered as React text only. Typing without picking always works: the text is the value.

import { useEffect, useId, useRef, useState } from "react";
import { searchPlaces, type GeoKind, type GeoPlace, type Point } from "../../lib/geo";
import { roundCoord } from "../../lib/geo-core.mjs";
import { useLang, tr } from "../../lib/i18n";

/** A suggestion that is not from the geocoder (e.g. cities that already have events), shown on an empty field. */
export interface ExtraOption {
  key: string;
  primary: string;
  secondary?: string;
  onSelect: () => void;
}

type Status = "idle" | "loading" | "done" | "error";
const DEBOUNCE_MS = 300;

export default function PlaceSuggest({
  id, value, onChange, onPick, kind, describe, bias = null, placeholder, ariaLabel, autoComplete = "off",
  extra = [], ariaInvalid, maxLength = 200, popEnd = false, className = "input", onEnter,
}: {
  id?: string;
  value: string;
  onChange: (text: string) => void;
  onPick: (place: GeoPlace) => void;
  kind: GeoKind;
  /** The two lines a suggestion shows. */
  describe: (place: GeoPlace) => { primary: string; secondary?: string };
  /** Prefer results near this point (sent rounded to ~11 km). */
  bias?: Point | null;
  placeholder?: string;
  ariaLabel?: string;
  autoComplete?: string;
  extra?: ExtraOption[];
  ariaInvalid?: boolean;
  maxLength?: number;
  /** Align the popup to the right edge (a field at the right of a bar). */
  popEnd?: boolean;
  className?: string;
  /** Enter with no suggestion highlighted; when absent, Enter does what it does in a form. */
  onEnter?: () => void;
}) {
  const { lang } = useLang();
  const listId = useId();
  const [open, setOpen] = useState(false);
  // The text to search: set only when the visitor types, so a picked value does not search again.
  const [query, setQuery] = useState<string | null>(null);
  const [items, setItems] = useState<GeoPlace[]>([]);
  const [status, setStatus] = useState<Status>("idle");
  const [active, setActive] = useState(-1);
  const biasRef = useRef(bias);
  biasRef.current = bias;
  const biasKey = bias ? `${roundCoord(bias.lat, 1)},${roundCoord(bias.lng, 1)}` : "";

  useEffect(() => {
    if (query === null) return;
    const q = query.trim();
    if (q.length < 2) {
      setItems([]);
      setStatus("idle");
      return;
    }
    const ctl = new AbortController();
    setStatus("loading");
    const t = setTimeout(() => {
      searchPlaces(q, kind, lang, biasRef.current, ctl.signal)
        .then((list) => {
          if (ctl.signal.aborted) return;
          setItems(list);
          setActive(-1);
          setStatus("done");
        })
        .catch(() => {
          if (ctl.signal.aborted) return;
          setItems([]);
          setStatus("error");
        });
    }, DEBOUNCE_MS);
    return () => {
      clearTimeout(t);
      ctl.abort();
    };
  }, [query, kind, lang, biasKey]);

  const short = (query ?? value).trim().length < 2;
  const options = short
    ? extra.map((o) => ({ key: o.key, primary: o.primary, secondary: o.secondary, select: o.onSelect }))
    : items.map((p, i) => ({ key: `${i}:${p.lat},${p.lng}`, ...describe(p), select: () => onPick(p) }));
  const fromGeocoder = !short && status !== "idle";
  const show = open && (options.length > 0 || fromGeocoder);

  function choose(i: number) {
    const o = options[i];
    if (!o) return;
    o.select();
    setOpen(false);
    setQuery(null);
    setActive(-1);
  }

  function onKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setOpen(true);
      setActive((a) => Math.min(a + 1, options.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive((a) => Math.max(a - 1, -1));
    } else if (e.key === "Enter" && show && active >= 0) {
      e.preventDefault(); // pick the suggestion, do not submit the form
      choose(active);
    } else if (e.key === "Enter" && onEnter) {
      e.preventDefault();
      setOpen(false);
      onEnter();
    } else if (e.key === "Escape" && show) {
      e.preventDefault();
      setOpen(false);
    }
  }

  return (
    <div className="suggest">
      <input
        id={id}
        className={className}
        role="combobox"
        aria-autocomplete="list"
        aria-expanded={show}
        aria-controls={listId}
        aria-activedescendant={show && active >= 0 ? `${listId}-${active}` : undefined}
        aria-label={ariaLabel}
        aria-invalid={ariaInvalid || undefined}
        autoComplete={autoComplete}
        maxLength={maxLength}
        value={value}
        placeholder={placeholder}
        onChange={(e) => {
          onChange(e.target.value);
          setQuery(e.target.value);
          setOpen(true);
          setActive(-1);
        }}
        onFocus={() => setOpen(true)}
        onBlur={() => setOpen(false)}
        onKeyDown={onKeyDown}
      />
      {show && (
        // mousedown would blur the input and close the list before the click lands.
        <div className={`suggest-pop${popEnd ? " end" : ""}`} onMouseDown={(e) => e.preventDefault()}>
          <ul role="listbox" id={listId} aria-label={ariaLabel ?? placeholder}>
            {options.map((o, i) => (
              <li key={o.key} id={`${listId}-${i}`} role="option" aria-selected={i === active} className="suggest-opt" onClick={() => choose(i)} onMouseEnter={() => setActive(i)}>
                <span className="p">{o.primary}</span>
                {o.secondary && <span className="s">{o.secondary}</span>}
              </li>
            ))}
          </ul>
          {fromGeocoder && (
            <div className="suggest-foot">
              {status === "loading" && options.length === 0 && <span>{tr(lang, "Шукаємо…", "Searching…")}</span>}
              {status === "error" && <span>{tr(lang, "Підказки зараз недоступні — можна ввести вручну", "Suggestions are unavailable right now — you can type it in")}</span>}
              {status === "done" && options.length === 0 && <span>{tr(lang, "Підказок немає — можна ввести вручну", "No suggestions — you can type it in")}</span>}
              <a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noreferrer">{tr(lang, "© учасники OpenStreetMap", "© OpenStreetMap contributors")}</a>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
