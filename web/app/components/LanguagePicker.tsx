"use client";

import { useLang, tr, LANGS, ENDONYMS, isLang } from "../../lib/i18n";
import { Globe } from "./Icon";

/** The interface language, offered by each language's own name (TASK-303 L1). A native select: keyboard and
 *  screen-reader friendly, and the phone's own picker on mobile.
 *  - "compact": a globe and a short code ("UA", "EN", "JA") with the select laid invisibly over it, for the top bar;
 *  - "full": a visible select with the names, for the profile menu and /account. */
export default function LanguagePicker({ variant = "full", onPicked }: { variant?: "compact" | "full"; onPicked?: () => void }) {
  const { lang, setLang } = useLang();
  // Named in the current language and in English, so someone who cannot read the current one still finds it.
  const label = lang === "en" ? "Language" : `${tr(lang, "Мова", "Language")} / Language`;
  const select = (
    <select
      className={variant === "full" ? "input" : undefined}
      aria-label={label}
      title={label}
      value={lang}
      onChange={(e) => {
        const v = e.target.value;
        if (isLang(v)) {
          setLang(v);
          onPicked?.();
        }
      }}
    >
      {LANGS.map((l) => <option key={l} value={l} lang={l}>{ENDONYMS[l]}</option>)}
    </select>
  );
  if (variant === "compact") {
    return (
      <span className="lang-pick">
        <Globe className="ico sm" aria-hidden />
        {/* "UA", not "UK": next to a globe, "UK" reads as the United Kingdom (the old toggle said UA too). */}
        <span aria-hidden>{lang === "uk" ? "UA" : lang.toUpperCase()}</span>
        {select}
      </span>
    );
  }
  return <span className="lang-select">{select}</span>;
}
