// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiListLabelOntologySignalsPathSchema = {"$id":"urn:kanban-tool:schema:api:list-label-ontology-signals-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"board":{"type":"string"}},"required":["board"],"title":"List Label Ontology Signals Path v1","type":"object"} as const;
export type ApiListLabelOntologySignalsPathContract = FromSchema<typeof ApiListLabelOntologySignalsPathSchema>;

export const apiListLabelOntologySignalsPathValidator: ReturnType<typeof createContractValidator<ApiListLabelOntologySignalsPathContract>> = createContractValidator<ApiListLabelOntologySignalsPathContract>(
  "api.list-label-ontology-signals.path",
  ApiListLabelOntologySignalsPathSchema,
);

export function parseApiListLabelOntologySignalsPath(value: unknown): ApiListLabelOntologySignalsPathContract {
  if (!apiListLabelOntologySignalsPathValidator(value)) throw new ContractValidationError("api.list-label-ontology-signals.path", apiListLabelOntologySignalsPathValidator.errors);
  return value;
}
