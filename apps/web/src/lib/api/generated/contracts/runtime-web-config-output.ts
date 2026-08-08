// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const RuntimeWebConfigOutputSchema = {"$id":"urn:kanban-tool:schema:runtime:web-config:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"description":"浏览器与 Tauri 加载 `/app/runtime.json` 时消费的 host metadata。","properties":{"actor":{"type":"string"},"apiBaseUrl":{"type":"string"},"defaultBoard":{"type":"string"},"protocolVersion":{"type":"string"},"serverVersion":{"type":"string"},"webBasePath":{"type":"string"},"webBuildId":{"type":"string"}},"required":["apiBaseUrl","webBasePath","actor","defaultBoard","serverVersion","protocolVersion","webBuildId"],"title":"Kanban Web runtime config v1","type":"object"} as const;
export type RuntimeWebConfigOutputContract = FromSchema<typeof RuntimeWebConfigOutputSchema>;

export const runtimeWebConfigOutputValidator: ReturnType<typeof createContractValidator<RuntimeWebConfigOutputContract>> = createContractValidator<RuntimeWebConfigOutputContract>(
  "runtime.web-config.output",
  RuntimeWebConfigOutputSchema,
);

export function parseRuntimeWebConfigOutput(value: unknown): RuntimeWebConfigOutputContract {
  if (!runtimeWebConfigOutputValidator(value)) throw new ContractValidationError("runtime.web-config.output", runtimeWebConfigOutputValidator.errors);
  return value;
}
