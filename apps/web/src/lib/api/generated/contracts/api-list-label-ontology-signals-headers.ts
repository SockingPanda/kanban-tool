// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiListLabelOntologySignalsHeadersSchema = {"$id":"urn:kanban-tool:schema:api:list-label-ontology-signals-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]}},"title":"Kanban api.list-label-ontology-signals request headers v1","type":"object"} as const;
export type ApiListLabelOntologySignalsHeadersContract = FromSchema<typeof ApiListLabelOntologySignalsHeadersSchema>;

export const apiListLabelOntologySignalsHeadersValidator: ReturnType<typeof createContractValidator<ApiListLabelOntologySignalsHeadersContract>> = createContractValidator<ApiListLabelOntologySignalsHeadersContract>(
  "api.list-label-ontology-signals.headers",
  ApiListLabelOntologySignalsHeadersSchema,
);

export function parseApiListLabelOntologySignalsHeaders(value: unknown): ApiListLabelOntologySignalsHeadersContract {
  if (!apiListLabelOntologySignalsHeadersValidator(value)) throw new ContractValidationError("api.list-label-ontology-signals.headers", apiListLabelOntologySignalsHeadersValidator.errors);
  return value;
}
