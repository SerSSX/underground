import type { Lang } from "./i18n-core.mjs";

export const SERVER_MESSAGES: Record<string, string>;
export const SERVER_PATTERNS: { re: RegExp; uk: string; en: string; vars: string[] }[];
export const SERVER_CODES: Record<string, string | null>;
export function canonicalServerText(raw: unknown): string | null;
export function serverText(lang: Lang, raw: unknown): string;
