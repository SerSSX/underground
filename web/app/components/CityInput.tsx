"use client";

import PlaceSuggest, { type ExtraOption } from "./PlaceSuggest";
import { cityLabel, cityName, type GeoPlace } from "../../lib/geo-core.mjs";
import { useLang, tr } from "../../lib/i18n";

/** City field with worldwide suggestions from the geocoder (TASK-303 G2), "City, Region, Country".
 *  Picking sets the text to the city's name; `onPick` also gets the place (point, country, code). Any typed
 *  name is accepted as is. */
export default function CityInput({
  value, onChange, onPick, id, placeholder, ariaLabel, extra, popEnd, className, ariaInvalid, onEnter,
}: {
  value: string;
  onChange: (v: string) => void;
  /** Called instead of `onChange` when a suggestion is picked; the consumer sets the text to `name`. */
  onPick?: (place: GeoPlace, name: string) => void;
  id?: string;
  placeholder?: string;
  ariaLabel?: string;
  extra?: ExtraOption[];
  popEnd?: boolean;
  className?: string;
  ariaInvalid?: boolean;
  /** Enter with no suggestion highlighted (the typed text as is). */
  onEnter?: () => void;
}) {
  const { lang } = useLang();
  return (
    <PlaceSuggest
      id={id}
      kind="city"
      value={value}
      onChange={onChange}
      onPick={(p) => {
        const name = cityName(p) || p.name || p.label;
        if (onPick) onPick(p, name);
        else onChange(name);
      }}
      describe={(p) => ({ primary: cityLabel(p) })}
      placeholder={placeholder ?? tr(lang, "Почни вводити місто…", "Start typing a city…")}
      ariaLabel={ariaLabel}
      autoComplete="address-level2"
      maxLength={100}
      extra={extra}
      popEnd={popEnd}
      className={className}
      ariaInvalid={ariaInvalid}
      onEnter={onEnter}
    />
  );
}
