import type { Lang } from "./i18n-core.mjs";

/** A form whose server texts get their own wording (SERVER_REWRITES). */
export type ServerTextContext = "signup";
export const SERVER_MESSAGES: Record<string, string>;
export const SERVER_PATTERNS: { re: RegExp; uk: string; en: string; vars: string[]; dates?: string[] }[];
export const SERVER_REWRITES: Record<string, Record<string, string>>;
export const SERVER_CODES: Record<string, string | null>;
export function canonicalServerText(raw: unknown): string | null;
export function isUnknownEnglish(raw: unknown): boolean;
export function serverText(lang: Lang, raw: unknown, where?: ServerTextContext): string;
