// TASK-303 G2: pure geo helpers (labels, the city filter, distances, the Maps link, the remembered city).
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  addressLabel, addressWhere, cityLabel, cityName, cityStoredName, distanceKm, geoLang, haversineKm, inCity, kmShown, mapsHref,
  parseStoredCity, roundCoord, trustedCityRow, sanitizePlace, sanitizePlaces, sortByDistance, storeCity,
} from "./geo-core.mjs";

const porto = { label: "Porto, Porto, Portugal", name: "Porto", street: null, housenumber: null, city: "Porto", state: "Porto", country: "Portugal", country_code: "PT", lat: 41.1496, lng: -8.611, kind: "city" };
const flores = { label: "Rua das Flores 12, Porto, Portugal", name: null, street: "Rua das Flores", housenumber: "12", city: "Porto", state: "Porto", country: "Portugal", country_code: "PT", lat: 41.1433, lng: -8.6125, kind: "address" };
const bar = { ...flores, name: "Café Majestic", street: "Rua de Santa Catarina", housenumber: "112" };

test("haversine agrees with the API (Kyiv–London ≈ 2133 km) and rounding drops -0", () => {
  assert.ok(Math.abs(haversineKm(50.4501, 30.5234, 51.5074, -0.1278) - 2133) < 20);
  assert.ok(haversineKm(1, 1, 1, 1) < 1e-9);
  assert.equal(roundCoord(50.45678, 2), 50.46);
  assert.ok(Object.is(roundCoord(-0.001, 1), 0));
});

test("UI language → API lang: two lowercase letters, else en", () => {
  for (const l of ["uk", "en", "es", "pt", "fr", "de", "it", "pl", "tr", "ja"]) assert.equal(geoLang(l), l);
  for (const l of ["", "EN", "eng", "u1", null, undefined]) assert.equal(geoLang(l), "en");
});

test("labels: City, Region, Country for cities; Name, Street N for the venue (no country)", () => {
  assert.equal(cityLabel(porto), "Porto, Portugal", "the region equal to the city is not repeated");
  assert.equal(cityLabel({ ...porto, name: "Kyoto", city: "Kyoto", state: "Kyoto Prefecture", country: "Japan" }), "Kyoto, Kyoto Prefecture, Japan");
  assert.equal(cityName(porto), "Porto");
  assert.equal(cityName(flores), "Porto");
  assert.equal(cityName({ ...flores, city: null }), "", "an address outside a known city names no city");
  assert.equal(addressLabel(flores), "Rua das Flores 12");
  assert.equal(addressLabel(bar), "Café Majestic, Rua de Santa Catarina 112");
  assert.ok(!addressLabel(bar).includes("Portugal"));
  assert.equal(addressLabel({ ...flores, street: null, housenumber: null }), "Porto", "nothing of its own → the locality");
  assert.equal(addressWhere(bar), "Porto, Portugal");
  assert.ok(addressLabel({ ...bar, name: "x".repeat(500) }).length <= 200);
});

test("[SEC] geocoder data is re-checked: bad points, kinds, codes and control characters never pass", () => {
  assert.equal(sanitizePlace({ ...porto, lat: "41" }), null);
  assert.equal(sanitizePlace({ ...porto, lat: 91 }), null);
  assert.equal(sanitizePlace({ ...porto, lng: NaN }), null);
  assert.equal(sanitizePlace(null), null);
  assert.equal(sanitizePlace({ lat: 1, lng: 1, kind: "city" }), null, "no text at all");
  const odd = sanitizePlace({ ...porto, kind: "<script>", country_code: "pt", name: "Por\u0000to\n<b>x</b>", state: 7 });
  assert.equal(odd.kind, "place");
  assert.equal(odd.country_code, "PT");
  assert.equal(odd.name, "Por to <b>x</b>", "control characters become spaces; markup stays inert text");
  assert.equal(odd.state, null);
  assert.equal(sanitizePlace({ ...porto, country_code: "P1" }).country_code, null);
  assert.deepEqual(sanitizePlaces([porto, { lat: 1 }, "x"]).map((p) => p.name), ["Porto"]);
  assert.deepEqual(sanitizePlaces({ error: "x" }), []);
});

test("the city filter: same name, or within 30 km when both have points", () => {
  const kyiv = { name: "Kyiv", lat: 50.4501, lng: 30.5234 };
  assert.equal(inCity({ city: "Київ", lat: 50.45, lng: 30.52 }, kyiv), true, "another language, same place");
  assert.equal(inCity({ city: "Бровари", lat: 50.51, lng: 30.79 }, kyiv), true, "a suburb ~20 km away");
  assert.equal(inCity({ city: "Lviv", lat: 49.84, lng: 24.03 }, kyiv), false);
  assert.equal(inCity({ city: " kyiv ", lat: null, lng: null }, kyiv), true, "name match without a point");
  assert.equal(inCity({ city: "Київ", lat: null, lng: null }, kyiv), false, "different name and no point");
  assert.equal(inCity({ city: "Lviv" }, { name: "Lviv" }), true);
  assert.equal(inCity({ city: "" }, { name: "" }), false, "an empty name matches nothing");
  assert.equal(inCity({ city: "X" }, null), true, "no city chosen → everything");
});

test("the city filter: a shared name counts only up to 150 km when both have points (G2 fix F6)", () => {
  const springfieldIL = { name: "Springfield", lat: 39.78, lng: -89.65 };
  assert.equal(inCity({ city: "Springfield", lat: 37.21, lng: -93.29 }, springfieldIL), false, "Springfield, MO is ~380 km away");
  assert.equal(inCity({ city: "Springfield", lat: 39.9, lng: -89.2 }, springfieldIL), true, "same name, ~40 km: the city's edge");
  assert.equal(inCity({ city: "Elsewhere", lat: 39.9, lng: -89.2 }, springfieldIL), false, "another name ~40 km away");
  assert.equal(inCity({ city: "Kyiv" }, { name: "Київ", label: "Kyiv" }), true, "the shown name matches too");
});

test("the stored city name is the OSM default-language one; labels stay localized (G2 fix F2)", () => {
  const kyiv = { ...porto, name: "Kyiv", city: "Kyiv", name_default: "Київ", city_default: "Київ", state: null, country: "Ukraine", country_code: "UA" };
  assert.equal(cityStoredName(kyiv), "Київ");
  assert.equal(cityLabel(kyiv), "Kyiv, Ukraine");
  assert.equal(cityStoredName({ ...flores, city_default: "Porto" }), "Porto");
  assert.equal(cityStoredName({ ...porto }), "Porto", "an older API without default names");
  assert.equal(cityStoredName({ ...flores, city: null }), "", "an address outside a city names no city");
  const s = sanitizePlace({ ...kyiv, city_default: "Київ\u0000", name_default: 5 });
  assert.equal(s.city_default, "Київ");
  assert.equal(s.name_default, null);
  assert.equal(sanitizePlace(porto).city_default, null);
});

test("distances: nearest first, events without a point last in their order; ~n km is at least 1", () => {
  const me = { lat: 50.45, lng: 30.52 };
  const list = [
    { id: "far", lat: 50.6, lng: 30.9 }, { id: "none1" }, { id: "near", lat: 50.451, lng: 30.521 }, { id: "none2", lat: null, lng: null },
  ];
  assert.deepEqual(sortByDistance(list, me).map((e) => e.id), ["near", "far", "none1", "none2"]);
  assert.equal(distanceKm({ lat: null, lng: null }, me), null);
  assert.equal(distanceKm(me, null), null);
  assert.equal(kmShown(0.2), 1);
  assert.equal(kmShown(12.6), 13);
});

test("Google Maps link: own point → coordinates; typed address on the city centre → address query", () => {
  const center = { lat: 50.4501, lng: 30.5234 };
  const picked = { venue_address: "Хрещатик 1", city: "Київ", lat: 50.44711, lng: 30.5226 };
  assert.equal(mapsHref(picked, center), "https://www.google.com/maps/search/?api=1&query=50.447110,30.522600");
  const typed = { venue_address: "Хрещатик 1", city: "Київ", lat: 50.4501, lng: 30.5234 };
  assert.equal(mapsHref(typed, center), `https://www.google.com/maps/search/?api=1&query=${encodeURIComponent("Хрещатик 1, Київ")}`);
  assert.equal(mapsHref(typed, undefined), mapsHref(typed, center), "centre not known yet → the address query");
  assert.equal(mapsHref(picked, null), "https://www.google.com/maps/search/?api=1&query=50.447110,30.522600", "no gazetteer row → the point is the event's own");
  assert.equal(mapsHref({ city: "Київ", lat: 50.4501, lng: 30.5234 }, undefined), "https://www.google.com/maps/search/?api=1&query=50.450100,30.523400", "no address → the point");
  assert.equal(mapsHref({ venue_address: "a&b?c#d", city: null }, null), "https://www.google.com/maps/search/?api=1&query=a%26b%3Fc%23d");
  assert.equal(mapsHref({ venue_address: "  ", city: "" }, null), null);
});

test("remembered city: JSON round trip, legacy plain names, junk ignored, near-me kept coarse", () => {
  assert.deepEqual(parseStoredCity("Київ"), { name: "Київ" }, "the old plain value");
  assert.deepEqual(parseStoredCity("123"), { name: "123" });
  assert.equal(parseStoredCity(""), null);
  assert.equal(parseStoredCity(null), null);
  assert.equal(parseStoredCity('{"lat":1}'), null);
  const porto = { name: "Porto", lat: 41.149612, lng: -8.610993, country_code: "PT" };
  assert.deepEqual(parseStoredCity(storeCity(porto)), { name: "Porto", lat: 41.1496, lng: -8.611, country_code: "PT" });
  const near = parseStoredCity(storeCity({ name: "Київ", lat: 50.456789, lng: 30.512345, near: true }));
  assert.deepEqual(near, { name: "Київ", lat: 50.46, lng: 30.51, near: true }, "the visitor's position is stored at ~1 km");
  assert.deepEqual(parseStoredCity('{"name":"X","lat":200,"lng":0,"near":true,"country_code":"<x>"}'), { name: "X" });
});

test("typed city lookup trusts seeded rows, or learned rows only in the country the form knows", () => {
  const seed = { name: "Porto", lat: 41.15, lng: -8.61, country_code: "PT", source: "seed" };
  const osmBR = { name: "Porto", lat: -8.9, lng: -36.5, country_code: "BR", source: "osm" };
  const osmIT = { name: "porto", lat: 43.07, lng: 11.94, country_code: "IT", source: "osm" };
  assert.equal(trustedCityRow([osmBR, seed], "Porto", null), seed, "no country known → the seeded row");
  assert.equal(trustedCityRow([seed, osmBR], "porto ", "br"), osmBR, "the learned row in the known country");
  assert.equal(trustedCityRow([osmBR, osmIT], "Porto", null), null, "learned rows alone answer no bare name");
  assert.equal(trustedCityRow([osmBR], "Porto", "IT"), null, "a learned row in another country is not trusted");
  assert.equal(trustedCityRow([{ name: "Lisbon", lat: 38.7, lng: -9.1 }], "Lisbon", null)?.name, "Lisbon", "an older API without source");
  assert.equal(trustedCityRow([{ ...seed, lat: 99 }], "Porto", null), null, "a row without a valid point");
  assert.equal(trustedCityRow(null, "Porto", null), null);
  // N4: a known country never falls back to a row elsewhere, seeded or not.
  assert.equal(trustedCityRow([seed], "Porto", "BR"), null, "the seeded Porto (PT) is not used for a Brazilian form");
  assert.equal(trustedCityRow([seed, osmBR], "Porto", "PT"), seed, "the seeded row in the known country");
});
