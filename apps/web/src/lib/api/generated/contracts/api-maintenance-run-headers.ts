// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiMaintenanceRunHeadersSchema = {"$id":"urn:kanban-tool:schema:api:maintenance-run-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]},"Content-Type":{"type":"string"}},"required":["Content-Type"],"title":"Kanban api.maintenance-run request headers v1","type":"object"} as const;
export type ApiMaintenanceRunHeadersContract = FromSchema<typeof ApiMaintenanceRunHeadersSchema>;

export const apiMaintenanceRunHeadersValidator: ReturnType<typeof createContractValidator<ApiMaintenanceRunHeadersContract>> = createContractValidator<ApiMaintenanceRunHeadersContract>(
  "api.maintenance-run.headers",
  ApiMaintenanceRunHeadersSchema,
);

export function parseApiMaintenanceRunHeaders(value: unknown): ApiMaintenanceRunHeadersContract {
  if (!apiMaintenanceRunHeadersValidator(value)) throw new ContractValidationError("api.maintenance-run.headers", apiMaintenanceRunHeadersValidator.errors);
  return value;
}
