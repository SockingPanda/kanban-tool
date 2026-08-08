// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiListStepsHeadersSchema = {"$id":"urn:kanban-tool:schema:api:list-steps-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]}},"title":"Kanban api.list-steps request headers v1","type":"object"} as const;
export type ApiListStepsHeadersContract = FromSchema<typeof ApiListStepsHeadersSchema>;

export const apiListStepsHeadersValidator: ReturnType<typeof createContractValidator<ApiListStepsHeadersContract>> = createContractValidator<ApiListStepsHeadersContract>(
  "api.list-steps.headers",
  ApiListStepsHeadersSchema,
);

export function parseApiListStepsHeaders(value: unknown): ApiListStepsHeadersContract {
  if (!apiListStepsHeadersValidator(value)) throw new ContractValidationError("api.list-steps.headers", apiListStepsHeadersValidator.errors);
  return value;
}
