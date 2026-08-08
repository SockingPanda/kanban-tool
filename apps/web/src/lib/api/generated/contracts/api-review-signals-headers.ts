// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiReviewSignalsHeadersSchema = {"$id":"urn:kanban-tool:schema:api:review-signals-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]}},"title":"Kanban api.review-signals request headers v1","type":"object"} as const;
export type ApiReviewSignalsHeadersContract = FromSchema<typeof ApiReviewSignalsHeadersSchema>;

export const apiReviewSignalsHeadersValidator: ReturnType<typeof createContractValidator<ApiReviewSignalsHeadersContract>> = createContractValidator<ApiReviewSignalsHeadersContract>(
  "api.review-signals.headers",
  ApiReviewSignalsHeadersSchema,
);

export function parseApiReviewSignalsHeaders(value: unknown): ApiReviewSignalsHeadersContract {
  if (!apiReviewSignalsHeadersValidator(value)) throw new ContractValidationError("api.review-signals.headers", apiReviewSignalsHeadersValidator.errors);
  return value;
}
