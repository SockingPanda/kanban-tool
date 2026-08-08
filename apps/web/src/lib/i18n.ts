import type { Locale } from "./preferences"
import type { MessagesByLocale, Overrides } from "@astryxdesign/core/i18n"

export const localeMessages = {
  zh: {
    productName: "Astryx 看板",
    productKicker: "KANBAN TOOL / WORKSPACE",
    navigation: "导航",
    workspace: "工作区",
    board: "看板",
    settings: "设置",
    skipToContent: "跳转到主要内容",
    expandSidebar: "展开侧栏",
    collapseSidebar: "收起侧栏",
    sidebarExpanded: "侧栏已展开",
    sidebarCollapsed: "侧栏已收起",
    theme: "主题",
    lightTheme: "浅色",
    darkTheme: "深色",
    language: "语言",
    chinese: "中文",
    english: "English",
    runtime: "运行时",
    actor: "执行者",
    api: "API",
    server: "服务版本",
    protocol: "协议版本",
    build: "Web 构建",
    defaultBoard: "默认看板选择器",
    homeLoading: "正在定位默认看板",
    homeLoadingDescription: "正在根据运行时选择器解析规范看板地址。",
    boardPlaceholder: "看板内容待 query 切片接入",
    boardPlaceholderDescription: "当前 shell 只提供可复制的路由、偏好和边界；不会伪造任务数据。",
    settingsHeading: "偏好与运行时",
    settingsDescription: "偏好只保存在当前浏览器的 kb:web:* localStorage 命名空间。",
    notFound: "页面不存在",
    notFoundDescription: "这个地址不属于 Astryx 当前 shell 路由。",
    loading: "加载中",
    error: "加载失败",
    offline: "当前离线",
    offlineDescription: "浏览器无法连接服务；恢复连接后可以重新加载。",
    retry: "重新加载",
    errorDescription: "shell 遇到未处理的错误。",
    invalidBoardSlug: "看板地址无效",
    invalidBoardSlugDescription: "该地址中的规范看板标识不符合服务约束。",
    routeBoundary: "路由边界",
    reported: "未报告",
  },
  en: {
    productName: "Astryx Kanban",
    productKicker: "KANBAN TOOL / WORKSPACE",
    navigation: "Navigation",
    workspace: "Workspace",
    board: "Board",
    settings: "Settings",
    skipToContent: "Skip to main content",
    expandSidebar: "Expand sidebar",
    collapseSidebar: "Collapse sidebar",
    sidebarExpanded: "Sidebar expanded",
    sidebarCollapsed: "Sidebar collapsed",
    theme: "Theme",
    lightTheme: "Light",
    darkTheme: "Dark",
    language: "Language",
    chinese: "中文",
    english: "English",
    runtime: "Runtime",
    actor: "Actor",
    api: "API",
    server: "Server version",
    protocol: "Protocol version",
    build: "Web build",
    defaultBoard: "Default board selector",
    homeLoading: "Resolving the default board",
    homeLoadingDescription: "Resolving the canonical board URL from the runtime selector.",
    boardPlaceholder: "Board content is waiting for the query slice",
    boardPlaceholderDescription: "This shell owns routes, preferences, and boundaries; it does not invent task data.",
    settingsHeading: "Preferences and runtime",
    settingsDescription: "Preferences stay in the current browser's kb:web:* localStorage namespace.",
    notFound: "Page not found",
    notFoundDescription: "This address is outside the current Astryx shell routes.",
    loading: "Loading",
    error: "Load failed",
    offline: "You are offline",
    offlineDescription: "The browser cannot reach the service. Reload after the connection returns.",
    retry: "Reload",
    errorDescription: "The shell encountered an unhandled error.",
    invalidBoardSlug: "Invalid board address",
    invalidBoardSlugDescription: "The canonical board identity in this address does not satisfy the service rules.",
    routeBoundary: "Route boundary",
    reported: "Not reported",
  },
} as const

export type MessageKey = keyof (typeof localeMessages)["zh"]

/** Static Astryx component catalogs kept beside the product copy. */
export const astryxMessages: MessagesByLocale = {
  zh: {
    "@astryx.appShell.mobileNavigation": { defaultMessage: "移动导航" },
    "@astryx.appShell.skipToContent": { defaultMessage: "跳转到主要内容" },
    "@astryx.sideNav.heading.dialogLabel": { defaultMessage: "导航" },
    "@astryx.sideNav.heading.openMenu": { defaultMessage: "打开菜单" },
    "@astryx.sideNav.label": { defaultMessage: "侧栏导航" },
    "@astryx.sideNav.resizeSidebar": { defaultMessage: "调整侧栏大小" },
    "@astryx.sideNavCollapseButton.collapseSidebar": { defaultMessage: "收起侧栏" },
    "@astryx.sideNavCollapseButton.expandSidebar": { defaultMessage: "展开侧栏" },
    "@astryx.sideNavItem.collapse": { defaultMessage: "收起 {label}" },
    "@astryx.sideNavItem.expand": { defaultMessage: "展开 {label}" },
  },
  en: {
    "@astryx.appShell.mobileNavigation": { defaultMessage: "Mobile navigation" },
    "@astryx.appShell.skipToContent": { defaultMessage: "Skip to main content" },
    "@astryx.sideNav.heading.dialogLabel": { defaultMessage: "Navigation" },
    "@astryx.sideNav.heading.openMenu": { defaultMessage: "Open menu" },
    "@astryx.sideNav.label": { defaultMessage: "Sidebar navigation" },
    "@astryx.sideNav.resizeSidebar": { defaultMessage: "Resize sidebar" },
    "@astryx.sideNavCollapseButton.collapseSidebar": { defaultMessage: "Collapse sidebar" },
    "@astryx.sideNavCollapseButton.expandSidebar": { defaultMessage: "Expand sidebar" },
    "@astryx.sideNavItem.collapse": { defaultMessage: "Collapse {label}" },
    "@astryx.sideNavItem.expand": { defaultMessage: "Expand {label}" },
  },
}

export const astryxOverrides: Overrides = {
  zh: {
    "@astryx.appShell.skipToContent": "跳转到主要内容",
    "@astryx.sideNav.label": "侧栏导航",
    "@astryx.sideNavCollapseButton.collapseSidebar": "收起侧栏",
    "@astryx.sideNavCollapseButton.expandSidebar": "展开侧栏",
  },
  en: {
    "@astryx.appShell.skipToContent": "Skip to main content",
    "@astryx.sideNav.label": "Sidebar navigation",
    "@astryx.sideNavCollapseButton.collapseSidebar": "Collapse sidebar",
    "@astryx.sideNavCollapseButton.expandSidebar": "Expand sidebar",
  },
}

export function messagesForLocale(locale: Locale) {
  return localeMessages[locale]
}

export function createTranslator(locale: Locale) {
  const messages = messagesForLocale(locale)
  return (key: MessageKey): string => messages[key]
}
