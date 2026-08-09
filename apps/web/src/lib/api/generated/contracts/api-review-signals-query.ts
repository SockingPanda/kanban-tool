// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";
import staticValidator from "virtual:kanban-contract-validator/api-review-signals-query";

export const ApiReviewSignalsQuerySchema = {"$id":"urn:kanban-tool:schema:api:review-signals-query:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"include_all":{"default":false,"type":"boolean"},"kind":{"default":[],"items":{"type":"string"},"type":"array"},"limit":{"default":100,"format":"uint","minimum":0,"type":"integer"},"status":{"default":[],"items":{"type":"string"},"type":"array"},"task_ref":{"type":["string","null"]}},"title":"Review Signals Query v1","type":"object"} as const;
export type ApiReviewSignalsQueryContract = FromSchema<typeof ApiReviewSignalsQuerySchema>;

export const apiReviewSignalsQueryValidator: ReturnType<typeof createContractValidator<ApiReviewSignalsQueryContract>> = createContractValidator<ApiReviewSignalsQueryContract>(
  "api.review-signals.query",
  staticValidator,
);

export function parseApiReviewSignalsQuery(value: unknown): ApiReviewSignalsQueryContract {
  if (!apiReviewSignalsQueryValidator(value)) throw new ContractValidationError("api.review-signals.query", apiReviewSignalsQueryValidator.errors);
  return value;
}
