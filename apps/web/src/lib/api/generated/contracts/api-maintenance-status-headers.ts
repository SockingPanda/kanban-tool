// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiMaintenanceStatusHeadersSchema = {"$id":"urn:kanban-tool:schema:api:maintenance-status-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]}},"title":"Kanban api.maintenance-status request headers v1","type":"object"} as const;
export type ApiMaintenanceStatusHeadersContract = FromSchema<typeof ApiMaintenanceStatusHeadersSchema>;

export const apiMaintenanceStatusHeadersValidator: ReturnType<typeof createContractValidator<ApiMaintenanceStatusHeadersContract>> = createContractValidator<ApiMaintenanceStatusHeadersContract>(
  "api.maintenance-status.headers",
  ApiMaintenanceStatusHeadersSchema,
);

export function parseApiMaintenanceStatusHeaders(value: unknown): ApiMaintenanceStatusHeadersContract {
  if (!apiMaintenanceStatusHeadersValidator(value)) throw new ContractValidationError("api.maintenance-status.headers", apiMaintenanceStatusHeadersValidator.errors);
  return value;
}
