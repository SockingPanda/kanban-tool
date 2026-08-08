// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiReviewLabelOntologyPathSchema = {"$id":"urn:kanban-tool:schema:api:review-label-ontology-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"board":{"type":"string"}},"required":["board"],"title":"Review Label Ontology Path v1","type":"object"} as const;
export type ApiReviewLabelOntologyPathContract = FromSchema<typeof ApiReviewLabelOntologyPathSchema>;

export const apiReviewLabelOntologyPathValidator: ReturnType<typeof createContractValidator<ApiReviewLabelOntologyPathContract>> = createContractValidator<ApiReviewLabelOntologyPathContract>(
  "api.review-label-ontology.path",
  ApiReviewLabelOntologyPathSchema,
);

export function parseApiReviewLabelOntologyPath(value: unknown): ApiReviewLabelOntologyPathContract {
  if (!apiReviewLabelOntologyPathValidator(value)) throw new ContractValidationError("api.review-label-ontology.path", apiReviewLabelOntologyPathValidator.errors);
  return value;
}
