// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";
import staticValidator from "virtual:kanban-contract-validator/api-release-task-path";

export const ApiReleaseTaskPathSchema = {"$id":"urn:kanban-tool:schema:api:release-task-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"task_id":{"type":"string"}},"required":["task_id"],"title":"Kanban release task path v1","type":"object"} as const;
export type ApiReleaseTaskPathContract = FromSchema<typeof ApiReleaseTaskPathSchema>;

export const apiReleaseTaskPathValidator: ReturnType<typeof createContractValidator<ApiReleaseTaskPathContract>> = createContractValidator<ApiReleaseTaskPathContract>(
  "api.release-task.path",
  staticValidator,
);

export function parseApiReleaseTaskPath(value: unknown): ApiReleaseTaskPathContract {
  if (!apiReleaseTaskPathValidator(value)) throw new ContractValidationError("api.release-task.path", apiReleaseTaskPathValidator.errors);
  return value;
}
