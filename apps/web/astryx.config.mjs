/**
 * Astryx CLI 项目配置。
 *
 * 这里的对象只使用已发布的 AstryxConfig schema。主题接线刻意记录在
 * package.json 的 `astryx.theme` 字段中，因为它是 CLI 0.3.0 当前由
 * doctor 检测的稳定入口。
 *
 * @type {import('@astryxdesign/cli/authoring').AstryxConfig}
 */
export default {
  integrations: [],
  issuesUrl: "https://github.com/facebook/astryx/issues",
}
