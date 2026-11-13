export type Lang = "uk" | "en" | "es" | "pt" | "fr" | "de" | "it" | "pl" | "tr" | "ja";
export type CatalogLang = Exclude<Lang, "uk" | "en">;
/** Plural forms by CLDR category. The Ukrainian source always gives one/few/many; English one/other. */
export type PluralForms = { zero?: string; one: string; two?: string; few?: string; many?: string; other?: string };
/** web/locales/<lang>.json: Ukrainian source → translated string, or (plural) category → form. */
export type Catalog = Record<string, string | Partial<Record<"zero" | "one" | "two" | "few" | "many" | "other", string>>>;
export type Vars = Record<string, string | number | null | undefined>;

export const LANGS: readonly Lang[];
export const DEFAULT_LANG: "uk";
export const CATALOG_LANGS: readonly CatalogLang[];
export const ENDONYMS: Record<Lang, string>;
export const INTL_LOCALES: Record<Lang, string>;
export function isLang(x: unknown): x is Lang;
export function pickLang(list: readonly string[] | null | undefined): Lang;
export function setCatalog(lang: Lang, catalog: unknown): void;
export function hasCatalog(lang: Lang): boolean;
export function serverLocale(lang: Lang): "uk" | "en";
export function setCurrentLang(lang: Lang): void;
export function currentLang(): Lang;
export function tr(lang: Lang, uk: string, en: string): string;
export function tf(lang: Lang, uk: string, en: string, vars?: Vars): string;
export function tp(lang: Lang, n: number, uk: PluralForms, en: PluralForms, vars?: Vars): string;
export function interpolate(template: string, vars?: Vars): string;
export function placeholders(template: string): string[];
export function pluralCategory(lang: Lang, n: number): string;
export function templateParts(text: string): Array<string | { slot: string }>;
