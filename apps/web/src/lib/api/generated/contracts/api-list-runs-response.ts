// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiListRunsResponseSchema = {"$defs":{"ApiRun":{"additionalProperties":false,"properties":{"claim_owner":{"type":"string"},"error":{"type":["string","null"]},"exit_code":{"format":"int64","type":["integer","null"]},"finished_at":{"format":"int64","type":["integer","null"]},"has_log":{"type":"boolean"},"id":{"type":"string"},"metadata":true,"started_at":{"format":"int64","type":"integer"},"status":{"$ref":"#/$defs/ApiRunStatus"},"summary":{"type":["string","null"]},"task_id":{"type":"string"},"worker_pid":{"format":"int64","type":["integer","null"]},"worker_profile":{"type":["string","null"]}},"required":["id","task_id","status","worker_profile","worker_pid","claim_owner","started_at","finished_at","exit_code","summary","error","has_log","metadata"],"type":"object"},"ApiRunStatus":{"enum":["running","succeeded","failed","canceled","expired"],"type":"string"}},"$id":"urn:kanban-tool:schema:api:list-runs-response:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"data":{"items":{"$ref":"#/$defs/ApiRun"},"type":"array"}},"required":["data"],"title":"Kanban list runs response v1","type":"object"} as const;
export type ApiListRunsResponseContract = FromSchema<typeof ApiListRunsResponseSchema>;

export const apiListRunsResponseValidator: ReturnType<typeof createContractValidator<ApiListRunsResponseContract>> = createContractValidator<ApiListRunsResponseContract>(
  "api.list-runs.response",
  ApiListRunsResponseSchema,
);

export function parseApiListRunsResponse(value: unknown): ApiListRunsResponseContract {
  if (!apiListRunsResponseValidator(value)) throw new ContractValidationError("api.list-runs.response", apiListRunsResponseValidator.errors);
  return value;
}
