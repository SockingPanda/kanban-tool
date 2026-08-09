// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";
import staticValidator from "virtual:kanban-contract-validator/api-label-ontology-signal-query";

export const ApiLabelOntologySignalQuerySchema = {"$id":"urn:kanban-tool:schema:api:label-ontology-signal-query:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"include_all":{"default":false,"type":"boolean"},"kind":{"default":[],"items":{"type":"string"},"type":"array"},"limit":{"default":100,"format":"uint","maximum":100,"minimum":1,"type":"integer"},"proposed_label_name":{"type":["string","null"]},"status":{"default":[],"items":{"type":"string"},"type":"array"},"target_label_ref":{"type":["string","null"]},"task_ref":{"type":["string","null"]}},"title":"Label ontology signal query v1","type":"object"} as const;
export type ApiLabelOntologySignalQueryContract = FromSchema<typeof ApiLabelOntologySignalQuerySchema>;

export const apiLabelOntologySignalQueryValidator: ReturnType<typeof createContractValidator<ApiLabelOntologySignalQueryContract>> = createContractValidator<ApiLabelOntologySignalQueryContract>(
  "api.label-ontology-signal.query",
  staticValidator,
);

export function parseApiLabelOntologySignalQuery(value: unknown): ApiLabelOntologySignalQueryContract {
  if (!apiLabelOntologySignalQueryValidator(value)) throw new ContractValidationError("api.label-ontology-signal.query", apiLabelOntologySignalQueryValidator.errors);
  return value;
}
