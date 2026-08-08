// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiHealthResponseSchema = {"$defs":{"HealthReport":{"additionalProperties":false,"properties":{"db":{"type":"string"},"db_fingerprint":{"type":"string"},"db_path":{"type":"string"},"ok":{"type":"boolean"},"version":{"type":"string"}},"required":["ok","db","version","db_path","db_fingerprint"],"type":"object"}},"$id":"urn:kanban-tool:schema:api:health-response:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"data":{"$ref":"#/$defs/HealthReport"}},"required":["data"],"title":"Kanban API health response v1","type":"object"} as const;
export type ApiHealthResponseContract = FromSchema<typeof ApiHealthResponseSchema>;

export const apiHealthResponseValidator: ReturnType<typeof createContractValidator<ApiHealthResponseContract>> = createContractValidator<ApiHealthResponseContract>(
  "api.health.response",
  ApiHealthResponseSchema,
);

export function parseApiHealthResponse(value: unknown): ApiHealthResponseContract {
  if (!apiHealthResponseValidator(value)) throw new ContractValidationError("api.health.response", apiHealthResponseValidator.errors);
  return value;
}
