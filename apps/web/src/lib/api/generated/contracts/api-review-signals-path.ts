// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiReviewSignalsPathSchema = {"$id":"urn:kanban-tool:schema:api:review-signals-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"board":{"type":"string"}},"required":["board"],"title":"Review Signals Path v1","type":"object"} as const;
export type ApiReviewSignalsPathContract = FromSchema<typeof ApiReviewSignalsPathSchema>;

export const apiReviewSignalsPathValidator: ReturnType<typeof createContractValidator<ApiReviewSignalsPathContract>> = createContractValidator<ApiReviewSignalsPathContract>(
  "api.review-signals.path",
  ApiReviewSignalsPathSchema,
);

export function parseApiReviewSignalsPath(value: unknown): ApiReviewSignalsPathContract {
  if (!apiReviewSignalsPathValidator(value)) throw new ContractValidationError("api.review-signals.path", apiReviewSignalsPathValidator.errors);
  return value;
}
