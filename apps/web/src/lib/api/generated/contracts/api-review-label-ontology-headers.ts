// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiReviewLabelOntologyHeadersSchema = {"$id":"urn:kanban-tool:schema:api:review-label-ontology-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]}},"title":"Kanban api.review-label-ontology request headers v1","type":"object"} as const;
export type ApiReviewLabelOntologyHeadersContract = FromSchema<typeof ApiReviewLabelOntologyHeadersSchema>;

export const apiReviewLabelOntologyHeadersValidator: ReturnType<typeof createContractValidator<ApiReviewLabelOntologyHeadersContract>> = createContractValidator<ApiReviewLabelOntologyHeadersContract>(
  "api.review-label-ontology.headers",
  ApiReviewLabelOntologyHeadersSchema,
);

export function parseApiReviewLabelOntologyHeaders(value: unknown): ApiReviewLabelOntologyHeadersContract {
  if (!apiReviewLabelOntologyHeadersValidator(value)) throw new ContractValidationError("api.review-label-ontology.headers", apiReviewLabelOntologyHeadersValidator.errors);
  return value;
}
