// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiListBoardsHeadersSchema = {"$id":"urn:kanban-tool:schema:api:list-boards-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]}},"title":"Kanban api.list-boards request headers v1","type":"object"} as const;
export type ApiListBoardsHeadersContract = FromSchema<typeof ApiListBoardsHeadersSchema>;

export const apiListBoardsHeadersValidator: ReturnType<typeof createContractValidator<ApiListBoardsHeadersContract>> = createContractValidator<ApiListBoardsHeadersContract>(
  "api.list-boards.headers",
  ApiListBoardsHeadersSchema,
);

export function parseApiListBoardsHeaders(value: unknown): ApiListBoardsHeadersContract {
  if (!apiListBoardsHeadersValidator(value)) throw new ContractValidationError("api.list-boards.headers", apiListBoardsHeadersValidator.errors);
  return value;
}
