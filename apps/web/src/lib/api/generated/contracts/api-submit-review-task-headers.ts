// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiSubmitReviewTaskHeadersSchema = {"$id":"urn:kanban-tool:schema:api:submit-review-task-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]},"Content-Type":{"type":"string"},"X-KB-Actor":{"type":["string","null"]}},"required":["Content-Type"],"title":"Kanban api.submit-review-task request headers v1","type":"object"} as const;
export type ApiSubmitReviewTaskHeadersContract = FromSchema<typeof ApiSubmitReviewTaskHeadersSchema>;

export const apiSubmitReviewTaskHeadersValidator: ReturnType<typeof createContractValidator<ApiSubmitReviewTaskHeadersContract>> = createContractValidator<ApiSubmitReviewTaskHeadersContract>(
  "api.submit-review-task.headers",
  ApiSubmitReviewTaskHeadersSchema,
);

export function parseApiSubmitReviewTaskHeaders(value: unknown): ApiSubmitReviewTaskHeadersContract {
  if (!apiSubmitReviewTaskHeadersValidator(value)) throw new ContractValidationError("api.submit-review-task.headers", apiSubmitReviewTaskHeadersValidator.errors);
  return value;
}
