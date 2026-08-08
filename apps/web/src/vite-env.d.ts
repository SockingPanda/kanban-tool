/// <reference types="vite/client" />

declare module "virtual:kanban-contract-validator/*" {
  const validator: ((value: unknown) => boolean) & {
    errors?: import("./lib/api/generated/runtime").ContractErrorObject[] | null
  }

  export default validator
}
