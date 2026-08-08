/// <reference types="vite/client" />

declare module "virtual:kanban-runtime-validator" {
  const validator: ((value: unknown) => value is import("./lib/api/generated/contracts/runtime-web-config-output").RuntimeWebConfigOutputContract) & {
    errors?: import("ajv").ErrorObject[] | null
  }

  export default validator
}
