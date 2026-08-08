// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiGetLabelOntologySignalHeadersSchema = {"$id":"urn:kanban-tool:schema:api:get-label-ontology-signal-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]}},"title":"Kanban api.get-label-ontology-signal request headers v1","type":"object"} as const;
export type ApiGetLabelOntologySignalHeadersContract = FromSchema<typeof ApiGetLabelOntologySignalHeadersSchema>;

export const apiGetLabelOntologySignalHeadersValidator: ReturnType<typeof createContractValidator<ApiGetLabelOntologySignalHeadersContract>> = createContractValidator<ApiGetLabelOntologySignalHeadersContract>(
  "api.get-label-ontology-signal.headers",
  ApiGetLabelOntologySignalHeadersSchema,
);

export function parseApiGetLabelOntologySignalHeaders(value: unknown): ApiGetLabelOntologySignalHeadersContract {
  if (!apiGetLabelOntologySignalHeadersValidator(value)) throw new ContractValidationError("api.get-label-ontology-signal.headers", apiGetLabelOntologySignalHeadersValidator.errors);
  return value;
}
