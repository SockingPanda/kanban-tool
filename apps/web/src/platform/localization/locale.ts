/** Preference IDs deliberately stay compatible with existing v4 consumers. */
export type Locale = "zh" | "en";
export const SOURCE_LOCALE: Locale = "zh";
export const LOCALE_DESCRIPTORS = {
  zh: { tag: "zh-CN", nativeName: "简体中文", direction: "ltr" },
  en: { tag: "en", nativeName: "English", direction: "ltr" },
} as const;

/** Only aliases with shipped resources are accepted. Traditional Chinese is not shipped. */
export function parseLocale(value: unknown): Locale | null {
  if (typeof value !== "string") return null;
  const tag = value.trim().replaceAll("_", "-").toLowerCase();
  if (tag === "zh" || tag === "zh-cn" || tag === "zh-sg" || tag === "zh-hans" || /^zh-hans-(cn|sg)$/.test(tag)) return "zh";
  if (tag === "en" || /^en-[a-z]{2}$/.test(tag) || /^en-[0-9]{3}$/.test(tag)) return "en";
  return null;
}
export function localeTag(locale: Locale): string { return LOCALE_DESCRIPTORS[locale].tag; }
