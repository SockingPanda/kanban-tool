// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiMaintenanceRebuildRequestSchema = {"$id":"urn:kanban-tool:schema:api:maintenance-rebuild-request:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"action":{"type":["string","null"]},"owner":{"type":["string","null"]}},"title":"Kanban maintenance rebuild request v1","type":"object"} as const;
export type ApiMaintenanceRebuildRequestContract = FromSchema<typeof ApiMaintenanceRebuildRequestSchema>;

export const apiMaintenanceRebuildRequestValidator: ReturnType<typeof createContractValidator<ApiMaintenanceRebuildRequestContract>> = createContractValidator<ApiMaintenanceRebuildRequestContract>(
  "api.maintenance-rebuild.request",
  ApiMaintenanceRebuildRequestSchema,
);

export function parseApiMaintenanceRebuildRequest(value: unknown): ApiMaintenanceRebuildRequestContract {
  if (!apiMaintenanceRebuildRequestValidator(value)) throw new ContractValidationError("api.maintenance-rebuild.request", apiMaintenanceRebuildRequestValidator.errors);
  return value;
}
