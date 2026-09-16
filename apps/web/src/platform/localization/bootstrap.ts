import zh from "./locales/zh/common.json";
import en from "./locales/en/common.json";
import type { Locale } from "./locale";

/** Safe before React and before runtime configuration fetch. Same catalog, no network or engine. */
export function bootstrapLoadingCopy(locale: Locale): string {
  return (locale === "en" ? en : zh).bootstrapLoading;
}
