// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
export type ContractErrorObject = {
  readonly instancePath: string;
  readonly schemaPath: string;
  readonly keyword: string;
  readonly params: Record<string, unknown>;
  readonly message?: string;
  readonly propertyName?: string;
  readonly schema?: unknown;
  readonly data?: unknown;
};

export type ContractValidator<T> = ((value: unknown) => value is T) & {
  readonly errors: ContractErrorObject[] | null | undefined;
};

type StaticValidator = ((value: unknown) => boolean) & {
  errors?: ContractErrorObject[] | null | undefined;
};

export class ContractValidationError extends Error {
  readonly contractId: string;
  readonly errors: ContractErrorObject[] | null | undefined;

  constructor(contractId: string, errors: ContractErrorObject[] | null | undefined) {
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

// 数字策略 `reject_unsafe_json_numbers`：先拒绝非有限数和非安全整数，再调用构建期生成的静态 validator。
export function createContractValidator<T>(_id: string, staticValidator: StaticValidator): ContractValidator<T> {
  let errors: ContractErrorObject[] | null | undefined;
  const validate = Object.assign(
    (value: unknown): value is T => {
      const unsafePath = unsafeNumberPath(value);
      if (unsafePath !== null) {
        errors = [{ instancePath: unsafePath, schemaPath: "#/numericPolicy", keyword: "safeNumber", params: {}, message: "number must be finite and safe" }];
        validate.errors = errors;
        return false;
      }
      const valid = staticValidator(value);
      errors = staticValidator.errors;
      validate.errors = errors;
      return valid;
    },
    { errors },
  );
  return validate;
}
