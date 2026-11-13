/** One geocoder result as the API sends it (`GET /api/geo/search|reverse`, TASK-303 G1). Untrusted text. */
export interface GeoPlace {
  label: string;
  name: string | null;
  street: string | null;
  housenumber: string | null;
  city: string | null;
  /** OSM default-language names (absent from older APIs → null). */
  name_default: string | null;
  city_default: string | null;
  state: string | null;
  country: string | null;
  /** ISO 3166-1 alpha-2, upper case. */
  country_code: string | null;
  lat: number;
  lng: number;
  kind: "city" | "address" | "place";
}
export interface Point { lat: number; lng: number }
/** The home page's city filter. `near`: chosen with "Near me" (the point is the visitor's, rounded). */
export interface ChosenCity {
  /** The stored (default-language) name events are matched by. */
  name: string;
  /** The name as the visitor read it in the suggestion, when it differs. */
  label?: string;
  lat?: number;
  lng?: number;
  country_code?: string;
  near?: boolean;
}
type MaybePoint = { lat?: number | null; lng?: number | null };

export const CITY_RADIUS_KM: number;
export const SAME_NAME_RADIUS_KM: number;
export function hasPoint(o: unknown): o is Point;
export function haversineKm(lat1: number, lng1: number, lat2: number, lng2: number): number;
export function roundCoord(v: number, decimals: number): number;
export function geoLang(lang: string): string;
export function sanitizePlace(raw: unknown): GeoPlace | null;
export function sanitizePlaces(list: unknown): GeoPlace[];
export function cityName(place: GeoPlace | null | undefined): string;
export function cityStoredName(place: GeoPlace | null | undefined): string;
export function cityLabel(place: GeoPlace): string;
export function addressLabel(place: GeoPlace): string;
export function addressWhere(place: GeoPlace): string;
export function inCity(event: MaybePoint & { city?: string | null }, chosen: ChosenCity | null): boolean;
export function distanceKm(event: MaybePoint, origin: MaybePoint | null | undefined): number | null;
export function kmShown(d: number): number;
export function sortByDistance<T extends MaybePoint>(events: T[], origin: MaybePoint | null | undefined): T[];
export function mapsHref(
  event: MaybePoint & { venue_address?: string | null; city?: string | null },
  cityCenter: Point | null | undefined,
): string | null;
export function parseStoredCity(raw: string | null | undefined): ChosenCity | null;
export function storeCity(chosen: ChosenCity): string;
