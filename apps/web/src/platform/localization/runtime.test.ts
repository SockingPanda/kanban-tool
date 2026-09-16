import { afterEach, describe, expect, it } from "vitest";
import { createTranslator, setLocalizationReporter, translateText, type LocalizationIssue } from "./runtime";
import { taskStatusLabel, errorLabel } from "./presentation";

afterEach(() => setLocalizationReporter());
describe("bundled i18next integration", () => {
  it("selects English one/other and Chinese other plurals", () => {
    const en = createTranslator("en"), zh = createTranslator("zh");
    expect(en("tasks:count", { count: 1 })).toBe("1 task");
    expect(en("tasks:count", { count: 0 })).toBe("0 tasks");
    expect(en("tasks:count", { count: 2 })).toBe("2 tasks");
    expect(zh("tasks:count", { count: 1 })).toBe("1 个任务");
  });
  it("language-fixed translators do not share mutable current language", () => {
    const en = createTranslator("en"), zh = createTranslator("zh");
    expect(en("common:save")).toBe("Save");
    expect(zh("common:save")).toBe("保存");
    expect(en("common:save")).toBe("Save");
  });
  it("user names remain literal text and do not resolve nested translation syntax", () => {
    const name = '<img src=x onerror=alert(1)> $t(common:save) {{name}}';
    expect(createTranslator("en")("shell:archivedProject", { name })).toBe(`${name} · Archived`);
    // React/textContent performs output escaping. Do not use this string with innerHTML.
  });
  it("missing keys render a diagnostic marker without disclosing values", () => {
    const issues: LocalizationIssue[] = [];
    setLocalizationReporter(issue => issues.push(issue));
    expect(translateText("en", "unknown", "missing", { name: "private text" })).toBe("⟦unknown:missing⟧");
    expect(JSON.stringify(issues)).not.toContain("private text");
    expect(issues.some(i => i.kind === "missing-key")).toBe(true);
  });
  it("rejects invalid numeric count at runtime", () => {
    expect(() => createTranslator("en")("tasks:count", { count: NaN })).toThrow();
  });
  it("display helpers keep unknown user-defined values literal", () => {
    const en = createTranslator("en");
    expect(taskStatusLabel("done", en)).toBe("Done");
    expect(taskStatusLabel("Custom 用户状态", en)).toBe("Custom 用户状态");
    expect(errorLabel("UNKNOWN_REMOTE_STRING", en)).toBe(en("common:errorUnknown"));
  });
});
