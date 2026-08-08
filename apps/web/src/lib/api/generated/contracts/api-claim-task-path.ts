// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiClaimTaskPathSchema = {"$id":"urn:kanban-tool:schema:api:claim-task-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"task_id":{"type":"string"}},"required":["task_id"],"title":"Kanban claim task path v1","type":"object"} as const;
export type ApiClaimTaskPathContract = FromSchema<typeof ApiClaimTaskPathSchema>;

export const apiClaimTaskPathValidator: ReturnType<typeof createContractValidator<ApiClaimTaskPathContract>> = createContractValidator<ApiClaimTaskPathContract>(
  "api.claim-task.path",
  ApiClaimTaskPathSchema,
);

export function parseApiClaimTaskPath(value: unknown): ApiClaimTaskPathContract {
  if (!apiClaimTaskPathValidator(value)) throw new ContractValidationError("api.claim-task.path", apiClaimTaskPathValidator.errors);
  return value;
}
