// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiGetSignalPathSchema = {"$id":"urn:kanban-tool:schema:api:get-signal-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"signal_id":{"type":"string"}},"required":["signal_id"],"title":"Get Signal Path v1","type":"object"} as const;
export type ApiGetSignalPathContract = FromSchema<typeof ApiGetSignalPathSchema>;

export const apiGetSignalPathValidator: ReturnType<typeof createContractValidator<ApiGetSignalPathContract>> = createContractValidator<ApiGetSignalPathContract>(
  "api.get-signal.path",
  ApiGetSignalPathSchema,
);

export function parseApiGetSignalPath(value: unknown): ApiGetSignalPathContract {
  if (!apiGetSignalPathValidator(value)) throw new ContractValidationError("api.get-signal.path", apiGetSignalPathValidator.errors);
  return value;
}
