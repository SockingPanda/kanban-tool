// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiGetRunLogPathSchema = {"$id":"urn:kanban-tool:schema:api:get-run-log-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"run_id":{"type":"string"}},"required":["run_id"],"title":"Kanban API get run log path v1","type":"object"} as const;
export type ApiGetRunLogPathContract = FromSchema<typeof ApiGetRunLogPathSchema>;

export const apiGetRunLogPathValidator: ReturnType<typeof createContractValidator<ApiGetRunLogPathContract>> = createContractValidator<ApiGetRunLogPathContract>(
  "api.get-run-log.path",
  ApiGetRunLogPathSchema,
);

export function parseApiGetRunLogPath(value: unknown): ApiGetRunLogPathContract {
  if (!apiGetRunLogPathValidator(value)) throw new ContractValidationError("api.get-run-log.path", apiGetRunLogPathValidator.errors);
  return value;
}
