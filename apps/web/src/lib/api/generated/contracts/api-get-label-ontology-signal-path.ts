// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiGetLabelOntologySignalPathSchema = {"$id":"urn:kanban-tool:schema:api:get-label-ontology-signal-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"signal_id":{"type":"string"}},"required":["signal_id"],"title":"Get Label Ontology Signal Path v1","type":"object"} as const;
export type ApiGetLabelOntologySignalPathContract = FromSchema<typeof ApiGetLabelOntologySignalPathSchema>;

export const apiGetLabelOntologySignalPathValidator: ReturnType<typeof createContractValidator<ApiGetLabelOntologySignalPathContract>> = createContractValidator<ApiGetLabelOntologySignalPathContract>(
  "api.get-label-ontology-signal.path",
  ApiGetLabelOntologySignalPathSchema,
);

export function parseApiGetLabelOntologySignalPath(value: unknown): ApiGetLabelOntologySignalPathContract {
  if (!apiGetLabelOntologySignalPathValidator(value)) throw new ContractValidationError("api.get-label-ontology-signal.path", apiGetLabelOntologySignalPathValidator.errors);
  return value;
}
