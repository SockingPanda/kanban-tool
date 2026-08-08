// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiListBoardsResponseSchema = {"$defs":{"ApiBoard":{"additionalProperties":false,"properties":{"archived_at":{"format":"int64","type":["integer","null"]},"created_at":{"format":"int64","type":"integer"},"description":{"type":["string","null"]},"id":{"type":"string"},"name":{"type":"string"},"slug":{"type":"string"},"updated_at":{"format":"int64","type":"integer"}},"required":["id","slug","name","description","created_at","updated_at","archived_at"],"type":"object"}},"$id":"urn:kanban-tool:schema:api:list-boards-response:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"data":{"items":{"$ref":"#/$defs/ApiBoard"},"type":"array"}},"required":["data"],"title":"Kanban list boards response v1","type":"object"} as const;
export type ApiListBoardsResponseContract = FromSchema<typeof ApiListBoardsResponseSchema>;

export const apiListBoardsResponseValidator: ReturnType<typeof createContractValidator<ApiListBoardsResponseContract>> = createContractValidator<ApiListBoardsResponseContract>(
  "api.list-boards.response",
  ApiListBoardsResponseSchema,
);

export function parseApiListBoardsResponse(value: unknown): ApiListBoardsResponseContract {
  if (!apiListBoardsResponseValidator(value)) throw new ContractValidationError("api.list-boards.response", apiListBoardsResponseValidator.errors);
  return value;
}
