// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiCreateLabelOntologyActionHeadersSchema = {"$id":"urn:kanban-tool:schema:api:create-label-ontology-action-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]},"Content-Type":{"type":"string"}},"required":["Content-Type"],"title":"Kanban api.create-label-ontology-action request headers v1","type":"object"} as const;
export type ApiCreateLabelOntologyActionHeadersContract = FromSchema<typeof ApiCreateLabelOntologyActionHeadersSchema>;

export const apiCreateLabelOntologyActionHeadersValidator: ReturnType<typeof createContractValidator<ApiCreateLabelOntologyActionHeadersContract>> = createContractValidator<ApiCreateLabelOntologyActionHeadersContract>(
  "api.create-label-ontology-action.headers",
  ApiCreateLabelOntologyActionHeadersSchema,
);

export function parseApiCreateLabelOntologyActionHeaders(value: unknown): ApiCreateLabelOntologyActionHeadersContract {
  if (!apiCreateLabelOntologyActionHeadersValidator(value)) throw new ContractValidationError("api.create-label-ontology-action.headers", apiCreateLabelOntologyActionHeadersValidator.errors);
  return value;
}
