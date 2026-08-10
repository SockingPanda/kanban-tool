// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";
import staticValidator from "virtual:kanban-contract-validator/api-create-label-ontology-action-path";

export const ApiCreateLabelOntologyActionPathSchema = {"$id":"urn:kanban-tool:schema:api:create-label-ontology-action-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"board":{"type":"string"}},"required":["board"],"title":"Create Label Ontology Action Path v1","type":"object"} as const;
export type ApiCreateLabelOntologyActionPathContract = FromSchema<typeof ApiCreateLabelOntologyActionPathSchema>;

export const apiCreateLabelOntologyActionPathValidator: ReturnType<typeof createContractValidator<ApiCreateLabelOntologyActionPathContract>> = createContractValidator<ApiCreateLabelOntologyActionPathContract>(
  "api.create-label-ontology-action.path",
  staticValidator,
);

export function parseApiCreateLabelOntologyActionPath(value: unknown): ApiCreateLabelOntologyActionPathContract {
  if (!apiCreateLabelOntologyActionPathValidator(value)) throw new ContractValidationError("api.create-label-ontology-action.path", apiCreateLabelOntologyActionPathValidator.errors);
  return value;
}
