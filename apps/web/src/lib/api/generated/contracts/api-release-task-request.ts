// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";
import staticValidator from "virtual:kanban-contract-validator/api-release-task-request";

export const ApiReleaseTaskRequestSchema = {"$id":"urn:kanban-tool:schema:api:release-task-request:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"actor":{"type":["string","null"]},"claim_token":{"type":"string"}},"required":["claim_token"],"title":"Kanban release task request v1","type":"object"} as const;
export type ApiReleaseTaskRequestContract = FromSchema<typeof ApiReleaseTaskRequestSchema>;

export const apiReleaseTaskRequestValidator: ReturnType<typeof createContractValidator<ApiReleaseTaskRequestContract>> = createContractValidator<ApiReleaseTaskRequestContract>(
  "api.release-task.request",
  staticValidator,
);

export function parseApiReleaseTaskRequest(value: unknown): ApiReleaseTaskRequestContract {
  if (!apiReleaseTaskRequestValidator(value)) throw new ContractValidationError("api.release-task.request", apiReleaseTaskRequestValidator.errors);
  return value;
}
