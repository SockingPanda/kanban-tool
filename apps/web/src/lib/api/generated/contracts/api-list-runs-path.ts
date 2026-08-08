// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiListRunsPathSchema = {"$id":"urn:kanban-tool:schema:api:list-runs-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"task_id":{"type":"string"}},"required":["task_id"],"title":"Kanban list runs path v1","type":"object"} as const;
export type ApiListRunsPathContract = FromSchema<typeof ApiListRunsPathSchema>;

export const apiListRunsPathValidator: ReturnType<typeof createContractValidator<ApiListRunsPathContract>> = createContractValidator<ApiListRunsPathContract>(
  "api.list-runs.path",
  ApiListRunsPathSchema,
);

export function parseApiListRunsPath(value: unknown): ApiListRunsPathContract {
  if (!apiListRunsPathValidator(value)) throw new ContractValidationError("api.list-runs.path", apiListRunsPathValidator.errors);
  return value;
}
