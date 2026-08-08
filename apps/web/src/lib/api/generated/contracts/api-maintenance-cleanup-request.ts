// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiMaintenanceCleanupRequestSchema = {"$id":"urn:kanban-tool:schema:api:maintenance-cleanup-request:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"action":{"type":["string","null"]},"owner":{"type":["string","null"]}},"title":"Kanban maintenance cleanup request v1","type":"object"} as const;
export type ApiMaintenanceCleanupRequestContract = FromSchema<typeof ApiMaintenanceCleanupRequestSchema>;

export const apiMaintenanceCleanupRequestValidator: ReturnType<typeof createContractValidator<ApiMaintenanceCleanupRequestContract>> = createContractValidator<ApiMaintenanceCleanupRequestContract>(
  "api.maintenance-cleanup.request",
  ApiMaintenanceCleanupRequestSchema,
);

export function parseApiMaintenanceCleanupRequest(value: unknown): ApiMaintenanceCleanupRequestContract {
  if (!apiMaintenanceCleanupRequestValidator(value)) throw new ContractValidationError("api.maintenance-cleanup.request", apiMaintenanceCleanupRequestValidator.errors);
  return value;
}
