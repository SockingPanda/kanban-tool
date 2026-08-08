// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiListBoardColumnsPathSchema = {"$id":"urn:kanban-tool:schema:api:list-board-columns-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"board":{"type":"string"}},"required":["board"],"title":"Kanban API list board columns path v1","type":"object"} as const;
export type ApiListBoardColumnsPathContract = FromSchema<typeof ApiListBoardColumnsPathSchema>;

export const apiListBoardColumnsPathValidator: ReturnType<typeof createContractValidator<ApiListBoardColumnsPathContract>> = createContractValidator<ApiListBoardColumnsPathContract>(
  "api.list-board-columns.path",
  ApiListBoardColumnsPathSchema,
);

export function parseApiListBoardColumnsPath(value: unknown): ApiListBoardColumnsPathContract {
  if (!apiListBoardColumnsPathValidator(value)) throw new ContractValidationError("api.list-board-columns.path", apiListBoardColumnsPathValidator.errors);
  return value;
}
