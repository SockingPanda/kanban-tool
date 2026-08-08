// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiListBoardColumnsResponseSchema = {"$defs":{"ApiBoardColumn":{"additionalProperties":false,"properties":{"board_id":{"type":"string"},"created_at":{"format":"int64","type":"integer"},"hidden":{"type":"boolean"},"id":{"type":"string"},"position":{"format":"int64","type":"integer"},"status":{"$ref":"#/$defs/ApiTaskStatus"},"title":{"type":"string"},"updated_at":{"format":"int64","type":"integer"},"wip_limit":{"format":"int64","type":["integer","null"]}},"required":["id","board_id","status","title","position","hidden","wip_limit","created_at","updated_at"],"type":"object"},"ApiTaskStatus":{"enum":["triage","todo","scheduled","ready","running","blocked","review","done","archived"],"type":"string"}},"$id":"urn:kanban-tool:schema:api:list-board-columns-response:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"data":{"items":{"$ref":"#/$defs/ApiBoardColumn"},"type":"array"}},"required":["data"],"title":"Kanban API list board columns response v1","type":"object"} as const;
export type ApiListBoardColumnsResponseContract = FromSchema<typeof ApiListBoardColumnsResponseSchema>;

export const apiListBoardColumnsResponseValidator: ReturnType<typeof createContractValidator<ApiListBoardColumnsResponseContract>> = createContractValidator<ApiListBoardColumnsResponseContract>(
  "api.list-board-columns.response",
  ApiListBoardColumnsResponseSchema,
);

export function parseApiListBoardColumnsResponse(value: unknown): ApiListBoardColumnsResponseContract {
  if (!apiListBoardColumnsResponseValidator(value)) throw new ContractValidationError("api.list-board-columns.response", apiListBoardColumnsResponseValidator.errors);
  return value;
}
