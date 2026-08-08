// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiLabelAtomPathSchema = {"$id":"urn:kanban-tool:schema:api:label-atom-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"atom_ref":{"type":"string"},"board":{"type":"string"}},"required":["board","atom_ref"],"title":"Label atom path v1","type":"object"} as const;
export type ApiLabelAtomPathContract = FromSchema<typeof ApiLabelAtomPathSchema>;

export const apiLabelAtomPathValidator: ReturnType<typeof createContractValidator<ApiLabelAtomPathContract>> = createContractValidator<ApiLabelAtomPathContract>(
  "api.label-atom.path",
  ApiLabelAtomPathSchema,
);

export function parseApiLabelAtomPath(value: unknown): ApiLabelAtomPathContract {
  if (!apiLabelAtomPathValidator(value)) throw new ContractValidationError("api.label-atom.path", apiLabelAtomPathValidator.errors);
  return value;
}
