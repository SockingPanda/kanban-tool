// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiClaimTaskRequestSchema = {"$id":"urn:kanban-tool:schema:api:claim-task-request:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"actor":{"type":["string","null"]},"metadata":true,"ttl_ms":{"default":300000,"format":"int64","type":"integer"},"worker_profile":{"type":["string","null"]}},"title":"Kanban claim task request v1","type":"object"} as const;
export type ApiClaimTaskRequestContract = FromSchema<typeof ApiClaimTaskRequestSchema>;

export const apiClaimTaskRequestValidator: ReturnType<typeof createContractValidator<ApiClaimTaskRequestContract>> = createContractValidator<ApiClaimTaskRequestContract>(
  "api.claim-task.request",
  ApiClaimTaskRequestSchema,
);

export function parseApiClaimTaskRequest(value: unknown): ApiClaimTaskRequestContract {
  if (!apiClaimTaskRequestValidator(value)) throw new ContractValidationError("api.claim-task.request", apiClaimTaskRequestValidator.errors);
  return value;
}
