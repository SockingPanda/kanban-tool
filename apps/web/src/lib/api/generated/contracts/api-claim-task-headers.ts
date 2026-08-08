// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiClaimTaskHeadersSchema = {"$id":"urn:kanban-tool:schema:api:claim-task-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]},"Content-Type":{"type":"string"},"X-KB-Actor":{"type":["string","null"]}},"required":["Content-Type"],"title":"Kanban api.claim-task request headers v1","type":"object"} as const;
export type ApiClaimTaskHeadersContract = FromSchema<typeof ApiClaimTaskHeadersSchema>;

export const apiClaimTaskHeadersValidator: ReturnType<typeof createContractValidator<ApiClaimTaskHeadersContract>> = createContractValidator<ApiClaimTaskHeadersContract>(
  "api.claim-task.headers",
  ApiClaimTaskHeadersSchema,
);

export function parseApiClaimTaskHeaders(value: unknown): ApiClaimTaskHeadersContract {
  if (!apiClaimTaskHeadersValidator(value)) throw new ContractValidationError("api.claim-task.headers", apiClaimTaskHeadersValidator.errors);
  return value;
}
