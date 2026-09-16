// 兼容已有 v4 导入；文案事实由 locale 资源文件持有。
import zh from "../platform/localization/locales/zh/legacy.json";
import en from "../platform/localization/locales/en/legacy.json";
import type { Locale } from "../platform/localization/locale";
import { translateText } from "../platform/localization/runtime";

export const localeMessages = { zh, en };
export type MessageKey = keyof typeof zh;
export function messagesForLocale(locale: Locale) { return localeMessages[locale]; }
export function createTranslator(locale: Locale) {
  return (key: MessageKey): string => translateText(locale, "legacy", key);
}
