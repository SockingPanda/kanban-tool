import { lstatSync, readFileSync, realpathSync } from "node:fs"
import path from "node:path"

import Ajv2020 from "ajv/dist/2020"
import standaloneCode from "ajv/dist/standalone"

const virtualModulePrefix = "virtual:kanban-contract-validator/"
const resolvedVirtualModulePrefix = `\0${virtualModulePrefix}`
const generatedSchemaDirectory = path.resolve(import.meta.dirname, "../src/lib/api/generated/schemas")
const contractSlugPattern = /^[a-z0-9]+(?:-[a-z0-9]+)*$/

/**
 * 在 Node 构建阶段把任一 generated contract schema 编译成静态 validator。
 * 浏览器只执行生成的函数，不加载 AJV codegen，也不触发 `unsafe-eval`。
 */
export function createContractValidatorPlugin(options: { schemaDirectory?: string } = {}) {
  const schemaDirectory = path.resolve(options.schemaDirectory ?? generatedSchemaDirectory)
  return {
    name: "kanban-contract-validator",
    resolveId(id: string) {
      if (!id.startsWith(virtualModulePrefix)) return undefined
      const slug = id.slice(virtualModulePrefix.length)
      schemaPathForSlug(slug, schemaDirectory)
      return `${resolvedVirtualModulePrefix}${slug}`
    },
    load(id: string) {
      if (!id.startsWith(resolvedVirtualModulePrefix)) return undefined

      const slug = id.slice(resolvedVirtualModulePrefix.length)
      const schemaPath = schemaPathForSlug(slug, schemaDirectory)
      const schema = JSON.parse(readFileSync(schemaPath, "utf8")) as object
      const ajv = new Ajv2020({ allErrors: true, strict: true, validateFormats: false, code: { esm: true, source: true } })
      const validator = ajv.compile(schema)
      return browserEsmStandaloneCode(standaloneCode(ajv, validator))
    },
  }
}

/**
 * AJV's ESM standalone output can still emit CommonJS `require()` calls for
 * runtime helpers such as unicode length. Production bundling rewrites those
 * calls, but Vite serves virtual modules directly during Storybook dev. Keep
 * the validator genuinely browser-native by expressing helper dependencies as
 * ESM imports at the virtual-module boundary.
 */
function browserEsmStandaloneCode(source: string): string {
  return source.replace(
    /const ([A-Za-z_$][\w$]*) = require\(("ajv\/dist\/runtime\/[^"]+")\)\.default;/g,
    "import $1 from $2;",
  )
}

function schemaPathForSlug(slug: string, schemaDirectory: string): string {
  if (!contractSlugPattern.test(slug)) {
    throw new Error(`Invalid generated contract validator slug: ${slug}`)
  }

  const schemaPath = path.resolve(schemaDirectory, `${slug}.schema.json`)
  if (path.dirname(schemaPath) !== schemaDirectory) {
    throw new Error(`Invalid generated contract validator path: ${slug}`)
  }
  let canonicalDirectory: string
  try {
    canonicalDirectory = realpathSync(schemaDirectory)
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") {
      throw new Error(`Unknown generated contract validator slug: ${slug}`)
    }
    throw error
  }
  if (canonicalDirectory !== schemaDirectory) {
    throw new Error(`Generated contract validator schema directory must not contain a symlink: ${schemaDirectory}`)
  }
  let metadata
  try {
    metadata = lstatSync(schemaPath)
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") {
      throw new Error(`Unknown generated contract validator slug: ${slug}`)
    }
    throw error
  }
  if (metadata.isSymbolicLink()) {
    throw new Error(`Generated contract validator schema must not be a symlink: ${slug}`)
  }
  if (!metadata.isFile()) {
    throw new Error(`Unknown generated contract validator slug: ${slug}`)
  }
  let canonicalSchemaPath: string
  try {
    canonicalSchemaPath = realpathSync(schemaPath)
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") {
      throw new Error(`Unknown generated contract validator slug: ${slug}`)
    }
    throw error
  }
  if (canonicalSchemaPath !== schemaPath) {
    throw new Error(`Generated contract validator schema path must not contain a symlink: ${slug}`)
  }
  return schemaPath
}
