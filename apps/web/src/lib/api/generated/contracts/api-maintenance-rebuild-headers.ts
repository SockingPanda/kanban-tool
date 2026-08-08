// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiMaintenanceRebuildHeadersSchema = {"$id":"urn:kanban-tool:schema:api:maintenance-rebuild-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]},"Content-Type":{"type":"string"}},"required":["Content-Type"],"title":"Kanban api.maintenance-rebuild request headers v1","type":"object"} as const;
export type ApiMaintenanceRebuildHeadersContract = FromSchema<typeof ApiMaintenanceRebuildHeadersSchema>;

export const apiMaintenanceRebuildHeadersValidator: ReturnType<typeof createContractValidator<ApiMaintenanceRebuildHeadersContract>> = createContractValidator<ApiMaintenanceRebuildHeadersContract>(
  "api.maintenance-rebuild.headers",
  ApiMaintenanceRebuildHeadersSchema,
);

export function parseApiMaintenanceRebuildHeaders(value: unknown): ApiMaintenanceRebuildHeadersContract {
  if (!apiMaintenanceRebuildHeadersValidator(value)) throw new ContractValidationError("api.maintenance-rebuild.headers", apiMaintenanceRebuildHeadersValidator.errors);
  return value;
}
