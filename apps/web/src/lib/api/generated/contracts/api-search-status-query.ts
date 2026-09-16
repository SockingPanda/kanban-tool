// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import { ContractValidationError, createContractValidator, type ContractValue } from "../runtime";
import staticValidator from "virtual:kanban-contract-validator/api-search-status-query";

export const ApiSearchStatusQuerySchema = {"$id":"urn:kanban-tool:schema:api:search-status-query:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"board":{"default":"default","type":"string"}},"title":"Kanban search status query v1","type":"object"} as const;
export type ApiSearchStatusQueryContract = ContractValue<typeof ApiSearchStatusQuerySchema>;

export const apiSearchStatusQueryValidator: ReturnType<typeof createContractValidator<ApiSearchStatusQueryContract>> = createContractValidator<ApiSearchStatusQueryContract>(
  "api.search-status.query",
  staticValidator,
);

export function parseApiSearchStatusQuery(value: unknown): ApiSearchStatusQueryContract {
  if (!apiSearchStatusQueryValidator(value)) throw new ContractValidationError("api.search-status.query", apiSearchStatusQueryValidator.errors);
  return value;
}
