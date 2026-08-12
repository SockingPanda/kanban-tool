import { addons } from "storybook/manager-api"
import { create } from "storybook/theming"

const kanbanTheme = create({
  base: "dark",
  brandTitle: "kanban-tool · Design System",
  brandUrl: "/?path=/docs/design-system-philosophy--docs",
  colorPrimary: "#6ea8ff",
  colorSecondary: "#4f8fe8",
  appBg: "#0f1113",
  appContentBg: "#14181b",
  appPreviewBg: "#0f1113",
  appBorderColor: "#30363b",
  appBorderRadius: 6,
  textColor: "#f3f6f8",
  textMutedColor: "#9ba7b0",
  barTextColor: "#b8c2ca",
  barSelectedColor: "#8bb9ff",
  barHoverColor: "#dceaff",
  inputBg: "#191e22",
  inputBorder: "#3a4248",
  inputTextColor: "#f3f6f8",
  inputBorderRadius: 6,
})

addons.setConfig({
  theme: kanbanTheme,
  sidebar: { showRoots: true },
  toolbar: { title: { hidden: false } },
})
