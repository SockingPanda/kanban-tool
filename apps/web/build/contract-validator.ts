import { lstatSync, readFileSync, realpathSync } from "node:fs"
import path from "node:path"

import Ajv2020 from "ajv/dist/2020"
import standaloneCode from "ajv/dist/standalone"
import { _, type KeywordCxt } from "ajv"
import { parseJson } from "../src/lib/lossless-json"
import { I64_MIN, I64_MAX, U64_MAX } from "../src/domain/integer"

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
      const schema = integerSchema(parseJson(readFileSync(schemaPath, "utf8"))) as object
      const ajv = new Ajv2020({ allErrors: true, strict: true, validateFormats: false, code: { esm: true, source: true } })
      ajv.addKeyword({
        keyword: 'kanbanInteger',
        schemaType: 'object',
        code(context: KeywordCxt) {
          const { data, schema: bounds } = context
          const minimum = String(bounds.minimum)
          const maximum = String(bounds.maximum)
          context.fail(_`!((typeof ${data} === "bigint" || (typeof ${data} === "number" && Number.isSafeInteger(${data}))) && ${data} >= BigInt(${minimum}) && ${data} <= BigInt(${maximum}))`)
        },
      })
      const validator = ajv.compile(schema)
      return standaloneCode(ajv, validator)
    },
  }
}

/** 仅在构建时投影整数规则；canonical JSON Schema 保持原样。 */
function integerSchema(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(integerSchema)
  if (value === null || typeof value !== 'object') return value
  const schema = Object.fromEntries(Object.entries(value).map(([key, child]) => [key, integerSchema(child)]))
  if (schema.format !== undefined && !['int64', 'uint64', 'uint'].includes(String(schema.format))) return schema
  const types = Array.isArray(schema.type) ? schema.type : [schema.type]
  if (!types.includes('integer')) return schema
  const unsigned = schema.format === 'uint64' || schema.format === 'uint'
  const lower = unsigned ? 0n : I64_MIN
  const upper = unsigned ? U64_MAX : I64_MAX
  const minimum = schema.minimum === undefined ? lower : BigInt(String(schema.minimum))
  const maximum = schema.maximum === undefined ? upper : BigInt(String(schema.maximum))
  const integer = { kanbanInteger: { minimum: String(minimum > lower ? minimum : lower), maximum: String(maximum < upper ? maximum : upper) } }
  delete schema.type
  delete schema.format
  delete schema.minimum
  delete schema.maximum
  return { ...schema, anyOf: [integer, ...types.flatMap(type => type === 'integer' ? [] : [{ type }])] }
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
