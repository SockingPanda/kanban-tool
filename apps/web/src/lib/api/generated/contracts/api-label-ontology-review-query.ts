// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiLabelOntologyReviewQuerySchema = {"$defs":{"LabelOntologyReviewGroupByWire":{"enum":["label","candidate_atom","proposed_label","cluster"],"type":"string"}},"$id":"urn:kanban-tool:schema:api:label-ontology-review-query:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"group_by":{"$ref":"#/$defs/LabelOntologyReviewGroupByWire","default":"label"},"include_all":{"default":false,"type":"boolean"},"limit":{"default":100,"format":"uint","minimum":0,"type":"integer"}},"title":"Label ontology review query v1","type":"object"} as const;
export type ApiLabelOntologyReviewQueryContract = FromSchema<typeof ApiLabelOntologyReviewQuerySchema>;

export const apiLabelOntologyReviewQueryValidator: ReturnType<typeof createContractValidator<ApiLabelOntologyReviewQueryContract>> = createContractValidator<ApiLabelOntologyReviewQueryContract>(
  "api.label-ontology-review.query",
  ApiLabelOntologyReviewQuerySchema,
);

export function parseApiLabelOntologyReviewQuery(value: unknown): ApiLabelOntologyReviewQueryContract {
  if (!apiLabelOntologyReviewQueryValidator(value)) throw new ContractValidationError("api.label-ontology-review.query", apiLabelOntologyReviewQueryValidator.errors);
  return value;
}
