// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import { ContractValidationError, createContractValidator, type ContractValue } from "../runtime";
import staticValidator from "virtual:kanban-contract-validator/api-list-boards-query";

export const ApiListBoardsQuerySchema = {"$id":"urn:kanban-tool:schema:api:list-boards-query:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"include_archived":{"default":false,"type":"boolean"}},"title":"Kanban list boards query v1","type":"object"} as const;
export type ApiListBoardsQueryContract = ContractValue<typeof ApiListBoardsQuerySchema>;

export const apiListBoardsQueryValidator: ReturnType<typeof createContractValidator<ApiListBoardsQueryContract>> = createContractValidator<ApiListBoardsQueryContract>(
  "api.list-boards.query",
  staticValidator,
);

export function parseApiListBoardsQuery(value: unknown): ApiListBoardsQueryContract {
  if (!apiListBoardsQueryValidator(value)) throw new ContractValidationError("api.list-boards.query", apiListBoardsQueryValidator.errors);
  return value;
}
