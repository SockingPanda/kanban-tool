// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiSubmitReviewTaskRequestSchema = {"$id":"urn:kanban-tool:schema:api:submit-review-task-request:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"actor":{"type":["string","null"]},"claim_token":{"type":["string","null"]},"force":{"default":false,"type":"boolean"},"summary":{"type":["string","null"]}},"title":"Kanban submit review task request v1","type":"object"} as const;
export type ApiSubmitReviewTaskRequestContract = FromSchema<typeof ApiSubmitReviewTaskRequestSchema>;

export const apiSubmitReviewTaskRequestValidator: ReturnType<typeof createContractValidator<ApiSubmitReviewTaskRequestContract>> = createContractValidator<ApiSubmitReviewTaskRequestContract>(
  "api.submit-review-task.request",
  ApiSubmitReviewTaskRequestSchema,
);

export function parseApiSubmitReviewTaskRequest(value: unknown): ApiSubmitReviewTaskRequestContract {
  if (!apiSubmitReviewTaskRequestValidator(value)) throw new ContractValidationError("api.submit-review-task.request", apiSubmitReviewTaskRequestValidator.errors);
  return value;
}
