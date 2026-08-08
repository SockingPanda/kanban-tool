// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiMaintenanceVacuumResponseSchema = {"$defs":{"VacuumReport":{"additionalProperties":false,"properties":{"after_bytes":{"format":"uint64","minimum":0,"type":"integer"},"before_bytes":{"format":"uint64","minimum":0,"type":"integer"},"ok":{"type":"boolean"},"source_fingerprint":{"type":"string"}},"required":["ok","before_bytes","after_bytes","source_fingerprint"],"type":"object"}},"$id":"urn:kanban-tool:schema:api:maintenance-vacuum-response:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"data":{"$ref":"#/$defs/VacuumReport"}},"required":["data"],"title":"Kanban maintenance vacuum response v1","type":"object"} as const;
export type ApiMaintenanceVacuumResponseContract = FromSchema<typeof ApiMaintenanceVacuumResponseSchema>;

export const apiMaintenanceVacuumResponseValidator: ReturnType<typeof createContractValidator<ApiMaintenanceVacuumResponseContract>> = createContractValidator<ApiMaintenanceVacuumResponseContract>(
  "api.maintenance-vacuum.response",
  ApiMaintenanceVacuumResponseSchema,
);

export function parseApiMaintenanceVacuumResponse(value: unknown): ApiMaintenanceVacuumResponseContract {
  if (!apiMaintenanceVacuumResponseValidator(value)) throw new ContractValidationError("api.maintenance-vacuum.response", apiMaintenanceVacuumResponseValidator.errors);
  return value;
}
