// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiSuggestTaskLabelsResponseSchema = {"$defs":{"LabelSuggestionCandidateWire":{"additionalProperties":false,"properties":{"already_applied":{"type":"boolean"},"evidence_atoms":{"items":{"$ref":"#/$defs/LabelSuggestionEvidenceAtomWire"},"type":"array"},"label_id":{"type":"string"},"label_name":{"type":"string"},"negative_evidence_atoms":{"items":{"$ref":"#/$defs/LabelSuggestionEvidenceAtomWire"},"type":"array"},"score":{"format":"float","type":"number"},"weight":{"format":"float","type":"number"}},"required":["label_id","label_name","score","weight","already_applied","evidence_atoms","negative_evidence_atoms"],"type":"object"},"LabelSuggestionEvidenceAtomWire":{"additionalProperties":false,"properties":{"atom_id":{"type":"string"},"kind":{"type":"string"},"label_id":{"type":"string"},"label_name":{"type":"string"},"polarity":{"type":"string"},"score":{"format":"float","type":"number"},"text":{"type":"string"}},"required":["atom_id","label_id","label_name","polarity","kind","text","score"],"type":"object"},"LabelSuggestionResultWire":{"additionalProperties":false,"properties":{"board_id":{"type":"string"},"candidates":{"items":{"$ref":"#/$defs/LabelSuggestionCandidateWire"},"type":"array"},"coverage":{"format":"float","type":"number"},"coverage_cosine":{"format":"float","type":"number"},"degraded":{"type":"boolean"},"diagnostics":{"items":{"type":"string"},"type":"array"},"needs_new_label":{"type":"boolean"},"reason_codes":{"items":{"type":"string"},"type":"array"},"residual_norm":{"format":"float","type":"number"},"selected_labels":{"items":{"$ref":"#/$defs/LabelSuggestionCandidateWire"},"type":"array"},"task_id":{"type":"string"}},"required":["task_id","board_id","selected_labels","candidates","coverage","coverage_cosine","residual_norm","needs_new_label","reason_codes","degraded","diagnostics"],"type":"object"}},"$id":"urn:kanban-tool:schema:api:suggest-task-labels-response:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"data":{"$ref":"#/$defs/LabelSuggestionResultWire"}},"required":["data"],"title":"Suggest task labels response v1","type":"object"} as const;
export type ApiSuggestTaskLabelsResponseContract = FromSchema<typeof ApiSuggestTaskLabelsResponseSchema>;

export const apiSuggestTaskLabelsResponseValidator: ReturnType<typeof createContractValidator<ApiSuggestTaskLabelsResponseContract>> = createContractValidator<ApiSuggestTaskLabelsResponseContract>(
  "api.suggest-task-labels.response",
  ApiSuggestTaskLabelsResponseSchema,
);

export function parseApiSuggestTaskLabelsResponse(value: unknown): ApiSuggestTaskLabelsResponseContract {
  if (!apiSuggestTaskLabelsResponseValidator(value)) throw new ContractValidationError("api.suggest-task-labels.response", apiSuggestTaskLabelsResponseValidator.errors);
  return value;
}
