// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiGetRunLogResponseSchema = {"$defs":{"ApiRunLog":{"additionalProperties":false,"properties":{"content":{"type":"string"},"run_id":{"type":"string"},"truncated":{"type":"boolean"}},"required":["run_id","content","truncated"],"type":"object"}},"$id":"urn:kanban-tool:schema:api:get-run-log-response:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"data":{"$ref":"#/$defs/ApiRunLog"}},"required":["data"],"title":"Kanban API get run log response v1","type":"object"} as const;
export type ApiGetRunLogResponseContract = FromSchema<typeof ApiGetRunLogResponseSchema>;

export const apiGetRunLogResponseValidator: ReturnType<typeof createContractValidator<ApiGetRunLogResponseContract>> = createContractValidator<ApiGetRunLogResponseContract>(
  "api.get-run-log.response",
  ApiGetRunLogResponseSchema,
);

export function parseApiGetRunLogResponse(value: unknown): ApiGetRunLogResponseContract {
  if (!apiGetRunLogResponseValidator(value)) throw new ContractValidationError("api.get-run-log.response", apiGetRunLogResponseValidator.errors);
  return value;
}
