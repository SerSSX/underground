"use client";

import { useEffect, useState } from "react";
import { searchCities, type CityDto } from "../../lib/api";
import { useLang, tr } from "../../lib/i18n";

/** City input with gazetteer autocomplete via a native datalist (TASK-017 M2). */
export default function CityInput({ value, onChange }: { value: string; onChange: (v: string) => void }) {
  const [suggestions, setSuggestions] = useState<CityDto[]>([]);
  const { lang } = useLang();

  useEffect(() => {
    if (value.length < 2) {
      setSuggestions([]);
      return;
    }
    let active = true;
    searchCities(value).then((c) => active && setSuggestions(c)).catch(() => {});
    return () => {
      active = false;
    };
  }, [value]);

  return (
    <>
      <input className="input" list="city-gazetteer" value={value} onChange={(e) => onChange(e.target.value)} placeholder={tr(lang, "Почни вводити місто…", "Start typing a city…")} />
      <datalist id="city-gazetteer">
        {suggestions.map((c) => (
          <option key={c.name} value={c.name}>{c.country}</option>
        ))}
      </datalist>
    </>
  );
}
