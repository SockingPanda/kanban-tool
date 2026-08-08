// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiGetSignalHeadersSchema = {"$id":"urn:kanban-tool:schema:api:get-signal-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]}},"title":"Kanban api.get-signal request headers v1","type":"object"} as const;
export type ApiGetSignalHeadersContract = FromSchema<typeof ApiGetSignalHeadersSchema>;

export const apiGetSignalHeadersValidator: ReturnType<typeof createContractValidator<ApiGetSignalHeadersContract>> = createContractValidator<ApiGetSignalHeadersContract>(
  "api.get-signal.headers",
  ApiGetSignalHeadersSchema,
);

export function parseApiGetSignalHeaders(value: unknown): ApiGetSignalHeadersContract {
  if (!apiGetSignalHeadersValidator(value)) throw new ContractValidationError("api.get-signal.headers", apiGetSignalHeadersValidator.errors);
  return value;
}
