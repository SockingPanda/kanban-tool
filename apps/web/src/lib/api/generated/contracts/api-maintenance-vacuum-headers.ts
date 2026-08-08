// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiMaintenanceVacuumHeadersSchema = {"$id":"urn:kanban-tool:schema:api:maintenance-vacuum-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]}},"title":"Kanban api.maintenance-vacuum request headers v1","type":"object"} as const;
export type ApiMaintenanceVacuumHeadersContract = FromSchema<typeof ApiMaintenanceVacuumHeadersSchema>;

export const apiMaintenanceVacuumHeadersValidator: ReturnType<typeof createContractValidator<ApiMaintenanceVacuumHeadersContract>> = createContractValidator<ApiMaintenanceVacuumHeadersContract>(
  "api.maintenance-vacuum.headers",
  ApiMaintenanceVacuumHeadersSchema,
);

export function parseApiMaintenanceVacuumHeaders(value: unknown): ApiMaintenanceVacuumHeadersContract {
  if (!apiMaintenanceVacuumHeadersValidator(value)) throw new ContractValidationError("api.maintenance-vacuum.headers", apiMaintenanceVacuumHeadersValidator.errors);
  return value;
}
