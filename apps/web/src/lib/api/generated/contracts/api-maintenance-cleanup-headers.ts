// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiMaintenanceCleanupHeadersSchema = {"$id":"urn:kanban-tool:schema:api:maintenance-cleanup-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]},"Content-Type":{"type":"string"}},"required":["Content-Type"],"title":"Kanban api.maintenance-cleanup request headers v1","type":"object"} as const;
export type ApiMaintenanceCleanupHeadersContract = FromSchema<typeof ApiMaintenanceCleanupHeadersSchema>;

export const apiMaintenanceCleanupHeadersValidator: ReturnType<typeof createContractValidator<ApiMaintenanceCleanupHeadersContract>> = createContractValidator<ApiMaintenanceCleanupHeadersContract>(
  "api.maintenance-cleanup.headers",
  ApiMaintenanceCleanupHeadersSchema,
);

export function parseApiMaintenanceCleanupHeaders(value: unknown): ApiMaintenanceCleanupHeadersContract {
  if (!apiMaintenanceCleanupHeadersValidator(value)) throw new ContractValidationError("api.maintenance-cleanup.headers", apiMaintenanceCleanupHeadersValidator.errors);
  return value;
}
