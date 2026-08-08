// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import Ajv2020, { type ErrorObject, type ValidateFunction } from "ajv/dist/2020";

// `strict` 保持开启；协议沿用的 int64/uint format 只在这里关闭格式校验，整数类型仍由 JSON Schema 校验。
const ajv = new Ajv2020({ allErrors: true, strict: true, validateFormats: false });
const validatorCache = new WeakMap<object, ValidateFunction>();

export type ContractValidator<T> = ((value: unknown) => value is T) & {
  readonly errors: ErrorObject[] | null | undefined;
};

export class ContractValidationError extends Error {
  readonly contractId: string;
  readonly errors: ErrorObject[] | null | undefined;

  constructor(contractId: string, errors: ErrorObject[] | null | undefined) {
    super(`Invalid ${contractId} payload`);
    this.name = "ContractValidationError";
    this.contractId = contractId;
    this.errors = errors;
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function unsafeNumberPath(value: unknown, path = ""): string | null {
  if (typeof value === "number") {
    return !Number.isFinite(value) || (Number.isInteger(value) && !Number.isSafeInteger(value)) ? path || "/" : null;
  }
  if (Array.isArray(value)) {
    for (let index = 0; index < value.length; index += 1) {
      const unsafe = unsafeNumberPath(value[index], `${path}/${index}`);
      if (unsafe !== null) return unsafe;
    }
    return null;
  }
  if (isRecord(value)) {
    for (const [key, child] of Object.entries(value)) {
      const escaped = key.replaceAll("~", "~0").replaceAll("/", "~1");
      const unsafe = unsafeNumberPath(child, `${path}/${escaped}`);
      if (unsafe !== null) return unsafe;
    }
  }
  return null;
}

function compileValidator(id: string, schema: object): ValidateFunction {
  const cached = validatorCache.get(schema);
  if (cached) return cached;
  const validator = ajv.compile(schema);
  validatorCache.set(schema, validator);
  return validator;
}

// 数字策略 `reject_unsafe_json_numbers`：先拒绝非有限数和非安全整数，再交给 AJV。
export function createContractValidator<T>(id: string, schema: object): ContractValidator<T> {
  let compiled: ValidateFunction | undefined;
  let errors: ErrorObject[] | null | undefined;
  const validate = Object.assign(
    (value: unknown): value is T => {
      const unsafePath = unsafeNumberPath(value);
      if (unsafePath !== null) {
        errors = [{ instancePath: unsafePath, schemaPath: "#/numericPolicy", keyword: "safeNumber", params: {}, message: "number must be finite and safe" }];
        validate.errors = errors;
        return false;
      }
      compiled ??= compileValidator(id, schema);
      const valid = compiled(value);
      errors = compiled.errors;
      validate.errors = errors;
      return valid;
    },
    { errors },
  );
  return validate;
}
