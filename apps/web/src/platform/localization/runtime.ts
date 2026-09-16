import { createInstance } from "i18next";
import { resources, namespaces } from "./resources.generated";
import { messageParameters, type MessageKey, type TranslateArgs, type Translator } from "./contracts.generated";
import { SOURCE_LOCALE, type Locale } from "./locale";

const parameterSets = new Map(Object.entries(messageParameters).map(([key, parameters]) => [key, new Set(parameters)]));

export type LocalizationIssue = {
  readonly kind: "missing-translation" | "missing-key";
  readonly locale: Locale;
  readonly key: string;
};
let reportIssue: ((issue: LocalizationIssue) => void) | undefined;
const reported = new Set<string>();
/** 可选的本地诊断，只记录语言、key 和错误类别。 */
export function setLocalizationReporter(reporter?: (issue: LocalizationIssue) => void): void {
  reportIssue = reporter;
  reported.clear();
}
function report(issue: LocalizationIssue): void {
  const id = `${issue.kind}:${issue.locale}:${issue.key}`;
  if (reported.has(id)) return;
  // 动态 key 的诊断去重缓存保持有界。
  if (reported.size >= 512) reported.clear();
  reported.add(id);
  try { reportIssue?.(issue); } catch { /* 诊断失败不能中断渲染。 */ }
}

const engine = createInstance();
// 资源同步打包，不加载翻译后端，也不修改全局语言状态。
void engine.init({
  resources,
  ns: namespaces,
  defaultNS: "common",
  lng: SOURCE_LOCALE,
  fallbackLng: SOURCE_LOCALE,
  supportedLngs: ["zh", "en"],
  load: "currentOnly",
  initAsync: false,
  keySeparator: false,
  nsSeparator: ":",
  returnNull: false,
  returnEmptyString: false,
  returnObjects: false,
  saveMissing: false,
  interpolation: { escapeValue: false, skipOnVariables: true },
});
if (!engine.isInitialized) throw new Error("Bundled localization did not initialize synchronously");

/** 仅供兼容层使用；新界面通过 createTranslator / useI18n 使用生成的 key。 */
export function translateText(locale: Locale, namespace: string, key: string, values: Readonly<Record<string, string | number>> = {}): string {
  const fullKey = `${namespace}:${key}`;
  const count = values.count;
  if (count !== undefined && (typeof count !== "number" || !Number.isFinite(count))) throw new TypeError("Translation count must be finite");
  if (!engine.exists(key, { lng: locale, ns: namespace, fallbackLng: false, count: typeof count === "number" ? count : undefined })) {
    report({ kind: "missing-translation", locale, key: fullKey });
  }
  if (!engine.exists(key, { lng: locale, ns: namespace, count: typeof count === "number" ? count : undefined })) {
    report({ kind: "missing-key", locale, key: fullKey });
    return `⟦${fullKey}⟧`;
  }
  const text = engine.t(key, {
    lng: locale,
    ns: namespace,
    // 插值参数不能覆盖 lng、ns、interpolation 等引擎选项。
    replace: values,
    count: typeof count === "number" ? count : undefined,
  });
  return typeof text === "string" ? text : `⟦${fullKey}⟧`;
}

export function createTranslator(locale: Locale): Translator {
  return function translate<K extends MessageKey>(key: K, ...args: TranslateArgs<K>): string {
    const required = parameterSets.get(key);
    if (!required) throw new TypeError(`Unknown translation key: ${String(key)}`);
    const values = (args[0] ?? {}) as Readonly<Record<string, string | number>>;
    for (const name of required) {
      const value = values[name];
      if (name === "count" ? typeof value !== "number" || !Number.isFinite(value) : typeof value !== "string") throw new TypeError(`Invalid translation parameter: ${name}`);
    }
    for (const name of Object.keys(values)) if (!required.has(name)) throw new TypeError(`Unexpected translation parameter: ${name}`);
    const separator = key.indexOf(":");
    return translateText(locale, key.slice(0, separator), key.slice(separator + 1), values);
  };
}
