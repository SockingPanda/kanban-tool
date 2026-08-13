import { defineTheme } from "@astryxdesign/core/theme"
import { neutralIconRegistry, neutralTheme } from "@astryxdesign/theme-neutral"

/** kanban-tool 的语义色覆盖；间距、字体和组件规则继续继承 neutral。 */
export const astryxTheme = defineTheme({
  name: "astryx",
  extends: neutralTheme,
  tokens: {
    "--color-background-body": ["#f6f7f8", "#0f1113"],
    "--color-background-surface": ["#ffffff", "#14171a"],
    "--color-background-muted": ["#f0f2f4", "#191d21"],
    "--color-background-card": ["#ffffff", "#171b1f"],
    "--color-background-popover": ["#ffffff", "#1b2025"],
    "--color-accent": ["#0877bd", "#4ba9e8"],
    "--color-accent-muted": ["#e4f3fc", "#123247"],
    "--color-text-primary": ["#15181c", "#f4f6f8"],
    "--color-text-secondary": ["#59616a", "#a7afb8"],
    "--color-text-disabled": ["#9ca4ac", "#626b75"],
    // active/selected 文本使用更强语义色；填充操作继续使用 --color-accent。
    "--color-text-accent": ["#075d93", "#4ba9e8"],
    "--color-icon-accent": ["#0877bd", "#4ba9e8"],
    "--color-icon-primary": ["#15181c", "#f4f6f8"],
    "--color-icon-secondary": ["#59616a", "#a7afb8"],
    "--color-icon-disabled": ["#9ca4ac", "#626b75"],
    "--color-on-accent": ["#ffffff", "#0f1113"],
    "--color-border": ["#dfe3e7", "#2a2f35"],
    "--color-border-emphasized": ["#c6cdd4", "#3a424a"],
    "--color-success": ["#147a3d", "#46c878"],
    "--color-success-muted": ["#e8f7ed", "#163a26"],
    "--color-on-success": ["#ffffff", "#0f1113"],
    "--color-error": ["#b42332", "#ef6570"],
    "--color-error-muted": ["#fdebed", "#411d23"],
    "--color-on-error": ["#ffffff", "#0f1113"],
    "--color-warning": ["#9a6500", "#e9ac38"],
    "--color-warning-muted": ["#fff4d9", "#3b2c12"],
    "--color-on-warning": ["#ffffff", "#0f1113"],
    // Operate shell type scale: 12 / 14 / 16 / 20px for metadata → title.
    "--font-size-lg": "1rem",
  },
  icons: neutralIconRegistry,
})
