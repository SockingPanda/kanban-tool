// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiDoctorHeadersSchema = {"$id":"urn:kanban-tool:schema:api:doctor-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]}},"title":"Kanban api.doctor request headers v1","type":"object"} as const;
export type ApiDoctorHeadersContract = FromSchema<typeof ApiDoctorHeadersSchema>;

export const apiDoctorHeadersValidator: ReturnType<typeof createContractValidator<ApiDoctorHeadersContract>> = createContractValidator<ApiDoctorHeadersContract>(
  "api.doctor.headers",
  ApiDoctorHeadersSchema,
);

export function parseApiDoctorHeaders(value: unknown): ApiDoctorHeadersContract {
  if (!apiDoctorHeadersValidator(value)) throw new ContractValidationError("api.doctor.headers", apiDoctorHeadersValidator.errors);
  return value;
}
