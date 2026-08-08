import { readFileSync } from "node:fs"
import path from "node:path"

import Ajv2020 from "ajv/dist/2020"
import standaloneCode from "ajv/dist/standalone"

const virtualModuleId = "virtual:kanban-runtime-validator"
const resolvedVirtualModuleId = `\0${virtualModuleId}`

/**
 * 在 Node 构建阶段把 generated runtime schema 编译成静态 validator。
 * 浏览器只执行生成的函数，不加载 AJV codegen，也不触发 `unsafe-eval`。
 */
export function createRuntimeValidatorPlugin() {
  return {
    name: "kanban-runtime-validator",
    resolveId(id: string) {
      return id === virtualModuleId ? resolvedVirtualModuleId : undefined
    },
    load(id: string) {
      if (id !== resolvedVirtualModuleId) return undefined

      const schemaPath = path.resolve(import.meta.dirname, "../src/lib/api/generated/schemas/runtime-web-config-output.schema.json")
      const schema = JSON.parse(readFileSync(schemaPath, "utf8")) as object
      const ajv = new Ajv2020({ allErrors: true, strict: true, validateFormats: false, code: { esm: true, source: true } })
      const validator = ajv.compile(schema)
      return standaloneCode(ajv, validator)
    },
  }
}
