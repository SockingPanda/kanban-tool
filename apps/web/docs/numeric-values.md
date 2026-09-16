# Web 数值契约

浏览器和 Desktop 共用同一份数值规则。正式 Protobuf 的 `int64` / `uint64` 由生成客户端持有
`bigint`；字段 codec 在 `Number.MIN_SAFE_INTEGER` 到 `Number.MAX_SAFE_INTEGER` 之间返回
`number`，范围外保留 `bigint`。页面 `Integer` 类型表示这两种精确值。服务端的符号、范围与业务
约束继续有效，例如 `expected_lock_version` 是有符号 64 位整数。

## 类型和校验

`xtask web-contracts generate` 从正式 JSON Schema 生成 `ContractValue`。`int64`、`uint64`、
`uint` 和无 format 的整数保留 `number | bigint`；nullable、优先级和浮点字段保留各自类型。
无 format 的事件 `id` 和 `created_at` 采用其正式 Protobuf 的有符号 64 位范围。

构建插件读取精确的 schema 数字 token，给整数生成静态范围比较代码。`uint` 的显式上限也按
十进制整数比较，不能先用 `JSON.parse` 把上限舍入。canonical schema 文件保持原样。浏览器只
执行构建好的 validator，不加载 AJV codegen，也不依赖 `unsafe-eval`。

已舍入的非安全 `number`、非有限数和超范围整数仍被拒绝。调用者必须在丢失精度前保留整数；将
一个已舍入的 `number` 再转成 `bigint` 无法恢复原值。业务 DTO 的整数字段不接受数值字符串。

## 展示、查询与回传

- React 数值文本、任务计数、事件 ID 和维护报告保留完整十进制。超安全范围的附件大小显示精确
  字节数；日期超出 `Date` 或日期控件的范围时，展示并编辑原始毫秒时间戳。
- 修改标题或其他字段时，未改动的日期保留原值，包括正常日期的毫秒部分。无效日期或超 i64
  范围的文本阻止保存，保留草稿。
- 排序通过整数大小比较，不把大整数相减后转换为 `number`。本地数组下标、显示页码、限额和
  内存预算继续使用有界 `number`；服务返回的分页总数、offset 和事件 cursor 保持精确值。
- QueryRegistry 的身份来自完整 Protobuf 请求 bytes，相邻大整数查询不会共用错误的缓存。
  snapshot / delta 在原子提交时保留数据和 cursor 的整数精度。
- mutation 使用同一份经过校验的值重新编码，CAS、时间戳及动态 JSON 都不经过 JSON 字符串
  中转；重试意图比较也保留大整数身份。

## 动态 JSON

`metadata`、`result` 和 `evidence` 的 Protobuf oneof 区分字符串、布尔、null、整数、浮点数、
数组与对象。大整数解码成 `bigint`，字符串继续是字符串。JSON 展示和文本解析使用
`src/lib/lossless-json.ts`：整数输出为无引号的数字 token，字符串保留引号。例如
`{"integer":18446744073709551615,"text":"18446744073709551615"}` 中两个字段不会混淆。
不要对业务值直接调用原生 `JSON.stringify`，也不要修改 `BigInt.prototype` 或依赖浏览器新增的
JSON raw token 扩展。
