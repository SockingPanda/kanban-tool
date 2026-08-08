// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiListEventsHeadersSchema = {"$id":"urn:kanban-tool:schema:api:list-events-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]}},"title":"Kanban api.list-events request headers v1","type":"object"} as const;
export type ApiListEventsHeadersContract = FromSchema<typeof ApiListEventsHeadersSchema>;

export const apiListEventsHeadersValidator: ReturnType<typeof createContractValidator<ApiListEventsHeadersContract>> = createContractValidator<ApiListEventsHeadersContract>(
  "api.list-events.headers",
  ApiListEventsHeadersSchema,
);

export function parseApiListEventsHeaders(value: unknown): ApiListEventsHeadersContract {
  if (!apiListEventsHeadersValidator(value)) throw new ContractValidationError("api.list-events.headers", apiListEventsHeadersValidator.errors);
  return value;
}
