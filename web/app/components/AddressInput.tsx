"use client";

import PlaceSuggest from "./PlaceSuggest";
import { addressLabel, addressWhere, type GeoPlace, type Point } from "../../lib/geo-core.mjs";
import { useLang, tr } from "../../lib/i18n";

/** Venue address with suggestions from the geocoder (TASK-303 G2), biased to the event's city. */
export default function AddressInput({ id, value, onChange, onPick, bias }: {
  id?: string;
  value: string;
  onChange: (v: string) => void;
  onPick: (place: GeoPlace) => void;
  bias?: Point | null;
}) {
  const { lang } = useLang();
  return (
    <PlaceSuggest
      id={id}
      kind="address"
      value={value}
      onChange={onChange}
      onPick={onPick}
      bias={bias}
      describe={(p) => ({ primary: addressLabel(p), secondary: addressWhere(p) || undefined })}
      placeholder={tr(lang, "Бар «Пульт», вул. Нижній Вал 21", "Venue, street and number")}
      maxLength={200}
    />
  );
}
