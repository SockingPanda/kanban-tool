// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiSubmitReviewTaskPathSchema = {"$id":"urn:kanban-tool:schema:api:submit-review-task-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"task_id":{"type":"string"}},"required":["task_id"],"title":"Kanban submit review task path v1","type":"object"} as const;
export type ApiSubmitReviewTaskPathContract = FromSchema<typeof ApiSubmitReviewTaskPathSchema>;

export const apiSubmitReviewTaskPathValidator: ReturnType<typeof createContractValidator<ApiSubmitReviewTaskPathContract>> = createContractValidator<ApiSubmitReviewTaskPathContract>(
  "api.submit-review-task.path",
  ApiSubmitReviewTaskPathSchema,
);

export function parseApiSubmitReviewTaskPath(value: unknown): ApiSubmitReviewTaskPathContract {
  if (!apiSubmitReviewTaskPathValidator(value)) throw new ContractValidationError("api.submit-review-task.path", apiSubmitReviewTaskPathValidator.errors);
  return value;
}
