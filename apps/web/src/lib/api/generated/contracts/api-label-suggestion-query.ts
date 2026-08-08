// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiLabelSuggestionQuerySchema = {"$id":"urn:kanban-tool:schema:api:label-suggestion-query:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"atom_limit":{"default":80,"format":"uint","minimum":0,"type":"integer"},"candidate_limit":{"default":32,"format":"uint","minimum":0,"type":"integer"},"limit":{"default":5,"format":"uint","minimum":0,"type":"integer"},"max_selected_labels":{"default":4,"format":"uint","minimum":0,"type":"integer"},"min_score":{"default":0.15000000596046448,"format":"float","type":"number"}},"title":"Label suggestion query v1","type":"object"} as const;
export type ApiLabelSuggestionQueryContract = FromSchema<typeof ApiLabelSuggestionQuerySchema>;

export const apiLabelSuggestionQueryValidator: ReturnType<typeof createContractValidator<ApiLabelSuggestionQueryContract>> = createContractValidator<ApiLabelSuggestionQueryContract>(
  "api.label-suggestion.query",
  ApiLabelSuggestionQuerySchema,
);

export function parseApiLabelSuggestionQuery(value: unknown): ApiLabelSuggestionQueryContract {
  if (!apiLabelSuggestionQueryValidator(value)) throw new ContractValidationError("api.label-suggestion.query", apiLabelSuggestionQueryValidator.errors);
  return value;
}
