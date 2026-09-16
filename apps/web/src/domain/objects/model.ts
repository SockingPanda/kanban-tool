import type { Integer } from '../integer';
export interface ObjectVersion { object: Integer; source: Integer | null }
export interface ObjectTarget { id: string; expected: ObjectVersion }
export interface ObjectValue { kind: 'text'|'number'|'boolean'|'date'|'object'|'select'; value: string|number|bigint|boolean }
export interface WorkspaceObject {
  id: string; board_id: string; type_key: string; title: string; body: string|null;
  version: ObjectVersion; created_at: Integer; updated_at: Integer; archived_at: Integer|null;
  properties: Record<string,ObjectValue[]>;
}
export interface ObjectPage { items: WorkspaceObject[]; next: {query_hash:string;after_id:string}|null }
export interface CatalogType { definition:{key:string;name:string;layout:unknown}; version:Integer; retired:boolean; capability:string; system:boolean }
export interface CatalogRelation { key:string;name:string;source_type:string;target_type:string|null;source_property:string;target_property:string|null;source_cardinality:'one'|'many';target_cardinality:'one'|'many';acyclic:boolean }
export interface ObjectCatalog { version:Integer;types:CatalogType[];relations:CatalogRelation[];properties:unknown[];bindings:unknown[];workflows:unknown[];rollups:unknown[] }
export interface ObjectReceipt { request_id:string;ids:string[];event_sequence:Integer;catalog_version:Integer;replayed:boolean;versions:Record<string,ObjectVersion> }
export interface ObjectCommand { board_id:string;actor:string;request_id:string;mutation:Record<string,unknown> }
export interface RelationLink { relation_key:string;source_id:string;target_id:string }
export interface ObjectQuery { board_id:string;type_key?:string;text?:string;include_archived?:boolean;limit?:number;filters?:unknown[];after?:ObjectPage['next'] }
export function target(object:WorkspaceObject):ObjectTarget {return{id:object.id,expected:object.version};}
