// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiMaintenanceImportV30RequestSchema = {"$id":"urn:kanban-tool:schema:api:maintenance-import-v30-request:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"description":"Legacy SQLite v30 导入请求。此 host-admin 路径有意与 portable JSONL 导入分开，\n因为它读取旧的磁盘 schema，并可能需要显式的附件根目录映射。","properties":{"canonical_attachment_root":{"default":null,"type":["string","null"]},"path":{"type":"string"}},"required":["path"],"title":"Kanban legacy SQLite v30 import request v1","type":"object"} as const;
export type ApiMaintenanceImportV30RequestContract = FromSchema<typeof ApiMaintenanceImportV30RequestSchema>;

export const apiMaintenanceImportV30RequestValidator: ReturnType<typeof createContractValidator<ApiMaintenanceImportV30RequestContract>> = createContractValidator<ApiMaintenanceImportV30RequestContract>(
  "api.maintenance-import-v30.request",
  ApiMaintenanceImportV30RequestSchema,
);

export function parseApiMaintenanceImportV30Request(value: unknown): ApiMaintenanceImportV30RequestContract {
  if (!apiMaintenanceImportV30RequestValidator(value)) throw new ContractValidationError("api.maintenance-import-v30.request", apiMaintenanceImportV30RequestValidator.errors);
  return value;
}
