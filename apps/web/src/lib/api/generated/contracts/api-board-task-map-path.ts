// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiBoardTaskMapPathSchema = {"$id":"urn:kanban-tool:schema:api:board-task-map-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"board":{"type":"string"}},"required":["board"],"title":"Kanban board task map path v1","type":"object"} as const;
export type ApiBoardTaskMapPathContract = FromSchema<typeof ApiBoardTaskMapPathSchema>;

export const apiBoardTaskMapPathValidator: ReturnType<typeof createContractValidator<ApiBoardTaskMapPathContract>> = createContractValidator<ApiBoardTaskMapPathContract>(
  "api.board-task-map.path",
  ApiBoardTaskMapPathSchema,
);

export function parseApiBoardTaskMapPath(value: unknown): ApiBoardTaskMapPathContract {
  if (!apiBoardTaskMapPathValidator(value)) throw new ContractValidationError("api.board-task-map.path", apiBoardTaskMapPathValidator.errors);
  return value;
}
