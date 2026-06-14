"use client";

import { createContext, useContext, useEffect, useState, type ReactNode } from "react";
import { syncUserLocale } from "./api";

export type Lang = "uk" | "en";
const KEY = "ug_lang";

export function getLang(): Lang {
  if (typeof window === "undefined") return "uk";
  return window.localStorage.getItem(KEY) === "en" ? "en" : "uk";
}

const LangContext = createContext<{ lang: Lang; setLang: (l: Lang) => void }>({
  lang: "uk",
  setLang: () => {},
});

export function LangProvider({ children }: { children: ReactNode }) {
  const [lang, setLangState] = useState<Lang>("uk");
  useEffect(() => {
    setLangState(getLang());
  }, []);
  function setLang(l: Lang) {
    setLangState(l);
    window.localStorage.setItem(KEY, l);
    document.documentElement.lang = l;
    // Best-effort: persist to backend prefs so server notifications match the UI language.
    void syncUserLocale(l);
  }
  return <LangContext.Provider value={{ lang, setLang }}>{children}</LangContext.Provider>;
}

export function useLang() {
  return useContext(LangContext);
}

/** Pick one of a Ukrainian / English pair by the active language. Co-located i18n:
 *  components keep their own strings, e.g. `tr(lang, "Афіша", "Discover")`. */
export function tr(lang: Lang, uk: string, en: string): string {
  return lang === "en" ? en : uk;
}

const MONTHS: Record<Lang, string[]> = {
  uk: ["січ", "лют", "бер", "кві", "тра", "чер", "лип", "сер", "вер", "жов", "лис", "гру"],
  en: ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"],
};

/** Locale-aware date: "15 чер · 19:00" / "15 Jun · 19:00". */
export function fmtDateL(iso: string | null, lang: Lang): string {
  if (!iso || iso.length < 10) return "";
  const m = Number(iso.slice(5, 7));
  const d = Number(iso.slice(8, 10));
  const day = iso.length >= 16 ? ` · ${iso.slice(11, 16)}` : "";
  return `${d} ${MONTHS[lang][m - 1] ?? ""}${day}`;
}

/** A compact UA/EN toggle for the nav. */
export function LanguageSwitcher() {
  const { lang, setLang } = useLang();
  return (
    <button
      className="btn ghost sm"
      title="Language / Мова"
      onClick={() => setLang(lang === "uk" ? "en" : "uk")}
    >
      {lang === "uk" ? "EN" : "UA"}
    </button>
  );
}
