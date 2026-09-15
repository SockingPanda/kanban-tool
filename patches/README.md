# 依赖补丁

`@bufbuild__protobuf@2.15.0.patch` 保留 Protobuf 字符串 map 中的 `__proto__` 数据键。
固定版本的 `create()` 和 `fromBinary()` 会把该键赋给普通对象的原型 setter，造成动态
metadata/result/evidence 在 binary 往返时丢字段。补丁只为该键定义可枚举的自有数据属性，
其他键保持原行为，同时覆盖 ESM 和 CommonJS 入口。

补丁由根 `pnpm-workspace.yaml` 登记，`pnpm-lock.yaml` 固定补丁哈希。实际生成协议的往返回归
位于 [`codec.test.ts`](../apps/web/src/lib/rpc/codec.test.ts)。升级此依赖时，先以同一回归确认上游
已经保留该键，再删除补丁及登记。
