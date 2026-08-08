// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiListStepsPathSchema = {"$id":"urn:kanban-tool:schema:api:list-steps-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"task_id":{"type":"string"}},"required":["task_id"],"title":"Kanban list steps path v1","type":"object"} as const;
export type ApiListStepsPathContract = FromSchema<typeof ApiListStepsPathSchema>;

export const apiListStepsPathValidator: ReturnType<typeof createContractValidator<ApiListStepsPathContract>> = createContractValidator<ApiListStepsPathContract>(
  "api.list-steps.path",
  ApiListStepsPathSchema,
);

export function parseApiListStepsPath(value: unknown): ApiListStepsPathContract {
  if (!apiListStepsPathValidator(value)) throw new ContractValidationError("api.list-steps.path", apiListStepsPathValidator.errors);
  return value;
}
