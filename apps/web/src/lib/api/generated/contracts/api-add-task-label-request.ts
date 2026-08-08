// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiAddTaskLabelRequestSchema = {"$id":"urn:kanban-tool:schema:api:add-task-label-request:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"actor":{"type":["string","null"]},"create_missing":{"default":false,"type":"boolean"},"name":{"type":["string","null"]},"names":{"items":{"type":"string"},"type":["array","null"]}},"title":"Kanban add task label request v1","type":"object"} as const;
export type ApiAddTaskLabelRequestContract = FromSchema<typeof ApiAddTaskLabelRequestSchema>;

export const apiAddTaskLabelRequestValidator: ReturnType<typeof createContractValidator<ApiAddTaskLabelRequestContract>> = createContractValidator<ApiAddTaskLabelRequestContract>(
  "api.add-task-label.request",
  ApiAddTaskLabelRequestSchema,
);

export function parseApiAddTaskLabelRequest(value: unknown): ApiAddTaskLabelRequestContract {
  if (!apiAddTaskLabelRequestValidator(value)) throw new ContractValidationError("api.add-task-label.request", apiAddTaskLabelRequestValidator.errors);
  return value;
}
