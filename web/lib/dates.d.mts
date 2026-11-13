import type { Lang } from "./i18n-core.mjs";

export type ZonedParts = { y: number; m: number; day: number; hh: number; mi: number; dow: number };
export function zonedParts(iso: string | null | undefined, tz?: string | null): ZonedParts | null;
export function timeOf(iso: string | null | undefined, tz?: string | null): string;
export function dateBadge(iso: string | null | undefined, lang: Lang, tz?: string | null): { day: string; mon: string } | null;
export function shortWhen(iso: string | null | undefined, lang: Lang, tz?: string | null): string;
export function longDate(iso: string | null | undefined, lang: Lang, tz?: string | null): string;
export function isoDateLabel(iso: string | null | undefined, lang: Lang): string;
export function zoneLabel(tz: string, lang: Lang): string;
