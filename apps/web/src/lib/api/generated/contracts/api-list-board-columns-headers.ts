// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiListBoardColumnsHeadersSchema = {"$id":"urn:kanban-tool:schema:api:list-board-columns-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]}},"title":"Kanban api.list-board-columns request headers v1","type":"object"} as const;
export type ApiListBoardColumnsHeadersContract = FromSchema<typeof ApiListBoardColumnsHeadersSchema>;

export const apiListBoardColumnsHeadersValidator: ReturnType<typeof createContractValidator<ApiListBoardColumnsHeadersContract>> = createContractValidator<ApiListBoardColumnsHeadersContract>(
  "api.list-board-columns.headers",
  ApiListBoardColumnsHeadersSchema,
);

export function parseApiListBoardColumnsHeaders(value: unknown): ApiListBoardColumnsHeadersContract {
  if (!apiListBoardColumnsHeadersValidator(value)) throw new ContractValidationError("api.list-board-columns.headers", apiListBoardColumnsHeadersValidator.errors);
  return value;
}
