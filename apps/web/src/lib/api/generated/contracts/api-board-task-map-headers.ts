// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiBoardTaskMapHeadersSchema = {"$id":"urn:kanban-tool:schema:api:board-task-map-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]}},"title":"Kanban api.board-task-map request headers v1","type":"object"} as const;
export type ApiBoardTaskMapHeadersContract = FromSchema<typeof ApiBoardTaskMapHeadersSchema>;

export const apiBoardTaskMapHeadersValidator: ReturnType<typeof createContractValidator<ApiBoardTaskMapHeadersContract>> = createContractValidator<ApiBoardTaskMapHeadersContract>(
  "api.board-task-map.headers",
  ApiBoardTaskMapHeadersSchema,
);

export function parseApiBoardTaskMapHeaders(value: unknown): ApiBoardTaskMapHeadersContract {
  if (!apiBoardTaskMapHeadersValidator(value)) throw new ContractValidationError("api.board-task-map.headers", apiBoardTaskMapHeadersValidator.errors);
  return value;
}
