// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiCheckpointHeadersSchema = {"$id":"urn:kanban-tool:schema:api:checkpoint-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]}},"title":"Kanban api.checkpoint request headers v1","type":"object"} as const;
export type ApiCheckpointHeadersContract = FromSchema<typeof ApiCheckpointHeadersSchema>;

export const apiCheckpointHeadersValidator: ReturnType<typeof createContractValidator<ApiCheckpointHeadersContract>> = createContractValidator<ApiCheckpointHeadersContract>(
  "api.checkpoint.headers",
  ApiCheckpointHeadersSchema,
);

export function parseApiCheckpointHeaders(value: unknown): ApiCheckpointHeadersContract {
  if (!apiCheckpointHeadersValidator(value)) throw new ContractValidationError("api.checkpoint.headers", apiCheckpointHeadersValidator.errors);
  return value;
}
