"use client";

// React side of i18n (TASK-303 L1): the active language, its catalog, <html lang>. The lookup functions
// live in lib/i18n-core.mjs (pure, shared with server code and node:test) and are re-exported here so
// pages keep importing { useLang, tr } from one place.

import { createContext, Fragment, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from "react";
import { syncUserLocale } from "./api";
import {
  DEFAULT_LANG, hasCatalog, isLang, pickLang, serverLocale, setCatalog, setCurrentLang, templateParts,
  type CatalogLang, type Lang,
} from "./i18n-core.mjs";
import { isoDateLabel } from "./dates.mjs";

export { LANGS, ENDONYMS, isLang, tr, tf, tp } from "./i18n-core.mjs";
export type { Lang, PluralForms, Catalog } from "./i18n-core.mjs";
export { serverText } from "./server-messages.mjs";

const KEY = "ug_lang";

/** One loader per catalog: each is its own chunk, so a visitor downloads only the language they read. */
const LOADERS: Record<CatalogLang, () => Promise<{ default: unknown }>> = {
  es: () => import("../locales/es.json"),
  pt: () => import("../locales/pt.json"),
  fr: () => import("../locales/fr.json"),
  de: () => import("../locales/de.json"),
  it: () => import("../locales/it.json"),
  pl: () => import("../locales/pl.json"),
  tr: () => import("../locales/tr.json"),
  ja: () => import("../locales/ja.json"),
};

async function ensureCatalog(l: Lang): Promise<void> {
  if (l === "uk" || l === "en" || hasCatalog(l)) return;
  try {
    setCatalog(l, (await LOADERS[l]()).default);
  } catch {
    // Chunk failed to load (offline, new deploy): register nothing, so text falls back to English now and the
    // next pick or visit downloads it again (an empty catalog registered here stayed until a reload).
  }
}

function readStored(): Lang | null {
  try {
    const v = window.localStorage.getItem(KEY);
    return isLang(v) ? v : null;
  } catch {
    return null;
  }
}
function store(l: Lang) {
  try { window.localStorage.setItem(KEY, l); } catch { /* private mode: the choice lasts for this page */ }
}

/** The stored language, else the browser's (first visit). Client only. */
export function getLang(): Lang {
  if (typeof window === "undefined") return DEFAULT_LANG;
  return readStored() ?? pickLang(navigator.languages?.length ? navigator.languages : [navigator.language]);
}

const LangContext = createContext<{ lang: Lang; setLang: (l: Lang) => void }>({
  lang: DEFAULT_LANG,
  setLang: () => {},
});

export function LangProvider({ children }: { children: ReactNode }) {
  // The server renders Ukrainian; after hydration the stored or browser language takes over.
  const [lang, setLangState] = useState<Lang>(DEFAULT_LANG);
  // Only the latest request applies: picking es then de quickly must not end on es when es loads last.
  const latest = useRef(0);

  const apply = useCallback(async (l: Lang) => {
    const req = ++latest.current;
    await ensureCatalog(l);
    if (req !== latest.current) return;
    setCurrentLang(l);
    setLangState(l);
    document.documentElement.lang = l;
  }, []);

  useEffect(() => {
    const stored = readStored();
    const l = getLang();
    if (!stored) store(l); // first visit: remember what the browser asked for
    void apply(l);
  }, [apply]);

  const setLang = useCallback((l: Lang) => {
    if (!isLang(l)) return;
    store(l);
    void apply(l);
    // Best-effort: persist to backend prefs so server notifications match the UI language.
    void syncUserLocale(serverLocale(l));
  }, [apply]);

  return <LangContext.Provider value={{ lang, setLang }}>{children}</LangContext.Provider>;
}

export function useLang() {
  return useContext(LangContext);
}

/** A calendar date as written in an ISO string: "15 черв · 19:00" / "15 Jun · 19:00" / "6月15日 · 19:00". */
export function fmtDateL(iso: string | null, lang: Lang): string {
  return isoDateLabel(iso, lang);
}

/** A translated sentence with elements inside it: rich(tf(lang, "Read the {terms}.", …, {}), { terms: <Link…/> }).
 *  Slots without an element stay visible as "{name}" (loud, not silent). */
export function rich(text: string, slots: Record<string, ReactNode>): ReactNode {
  return templateParts(text).map((p, i) => (
    <Fragment key={i}>{typeof p === "string" ? p : Object.prototype.hasOwnProperty.call(slots, p.slot) ? slots[p.slot] : `{${p.slot}}`}</Fragment>
  ));
}
