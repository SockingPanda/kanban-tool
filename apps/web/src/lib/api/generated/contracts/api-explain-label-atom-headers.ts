// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiExplainLabelAtomHeadersSchema = {"$id":"urn:kanban-tool:schema:api:explain-label-atom-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]}},"title":"Kanban api.explain-label-atom request headers v1","type":"object"} as const;
export type ApiExplainLabelAtomHeadersContract = FromSchema<typeof ApiExplainLabelAtomHeadersSchema>;

export const apiExplainLabelAtomHeadersValidator: ReturnType<typeof createContractValidator<ApiExplainLabelAtomHeadersContract>> = createContractValidator<ApiExplainLabelAtomHeadersContract>(
  "api.explain-label-atom.headers",
  ApiExplainLabelAtomHeadersSchema,
);

export function parseApiExplainLabelAtomHeaders(value: unknown): ApiExplainLabelAtomHeadersContract {
  if (!apiExplainLabelAtomHeadersValidator(value)) throw new ContractValidationError("api.explain-label-atom.headers", apiExplainLabelAtomHeadersValidator.errors);
  return value;
}
