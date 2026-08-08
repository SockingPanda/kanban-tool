// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiCreateLabelOntologyActionRequestSchema = {"$defs":{"JsonBodyField":true,"LabelOntologyActionTypeWire":{"enum":["confirm","reject","supersede","resolve_no_change","add_positive_atom","add_negative_atom","adopt_existing_atom","update_semantics","create_label_proposal","bootstrap_label","rename_label","split_label","merge_labels","revert_ontology_mutation","validate"],"type":"string"},"LabelOntologyActorWire":{"additionalProperties":false,"properties":{"agent_type":{"type":["string","null"]},"name":{"type":"string"},"type":{"type":"string"}},"required":["name","type"],"type":"object"},"LabelOntologyValidationStatusWire":{"enum":["not_required","pending","passed","failed","partial"],"type":"string"}},"$id":"urn:kanban-tool:schema:api:create-label-ontology-action-request:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"action_type":{"$ref":"#/$defs/LabelOntologyActionTypeWire"},"actor":{"$ref":"#/$defs/LabelOntologyActorWire"},"canonical_after_hash":{"type":["string","null"]},"canonical_before_hash":{"type":["string","null"]},"change":{"$ref":"#/$defs/JsonBodyField"},"parent_action_id":{"type":["string","null"]},"reason":{"type":"string"},"result_atom_content_hash":{"type":["string","null"]},"result_atom_id":{"type":["string","null"]},"result_label_ref":{"type":["string","null"]},"result_proposal_id":{"type":["string","null"]},"signal_ids":{"items":{"type":"string"},"type":"array"},"superseded_by_signal_id":{"type":["string","null"]},"target_label_ref":{"type":["string","null"]},"validation":{"$ref":"#/$defs/JsonBodyField"},"validation_status":{"anyOf":[{"$ref":"#/$defs/LabelOntologyValidationStatusWire"},{"type":"null"}]}},"required":["actor","action_type","signal_ids","reason"],"title":"Create label ontology action request v1","type":"object"} as const;
export type ApiCreateLabelOntologyActionRequestContract = FromSchema<typeof ApiCreateLabelOntologyActionRequestSchema>;

export const apiCreateLabelOntologyActionRequestValidator: ReturnType<typeof createContractValidator<ApiCreateLabelOntologyActionRequestContract>> = createContractValidator<ApiCreateLabelOntologyActionRequestContract>(
  "api.create-label-ontology-action.request",
  ApiCreateLabelOntologyActionRequestSchema,
);

export function parseApiCreateLabelOntologyActionRequest(value: unknown): ApiCreateLabelOntologyActionRequestContract {
  if (!apiCreateLabelOntologyActionRequestValidator(value)) throw new ContractValidationError("api.create-label-ontology-action.request", apiCreateLabelOntologyActionRequestValidator.errors);
  return value;
}
