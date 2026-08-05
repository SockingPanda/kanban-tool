import { readFileSync } from "node:fs"
import { afterEach, describe, expect, it, vi } from "vitest"
import { KanbanApi } from "./api"
const config={apiBaseUrl:"http://127.0.0.1:8721",actor:"desktop-test",board:"default"}
const list=JSON.parse(readFileSync(new URL("../../../../schemas/fixtures/api/list-comments-response.v1.valid.json",import.meta.url),"utf8"))
const create=JSON.parse(readFileSync(new URL("../../../../schemas/fixtures/api/create-comment-response.v1.valid.json",import.meta.url),"utf8"))
function response(value:unknown,status=200){return new Response(JSON.stringify(value),{status,headers:{"Content-Type":"application/json"}})}
afterEach(()=>vi.unstubAllGlobals())
describe("comments exact contracts",()=>{
 it("consumes committed list and create fixtures without losing open natural metadata",async()=>{const fetch=vi.fn().mockResolvedValueOnce(response(list)).mockResolvedValueOnce(response(create,201));vi.stubGlobal("fetch",fetch);const api=new KanbanApi(config,{locale:"zh-CN"});const comments=await api.listComments("t_fixture");expect(comments).toHaveLength(2);expect(comments[1]?.metadata).toEqual(list.data[1].metadata);expect((await api.createComment("t_fixture","decision")).metadata).toEqual(create.data.metadata)})
 it("list uses the canonical task comments GET contract",async()=>{const fetch=vi.fn(async()=>response(list));vi.stubGlobal("fetch",fetch);const api=new KanbanApi(config,{locale:"zh-CN"});await expect(api.listComments("t_fixture")).resolves.toHaveLength(2);expect(fetch).toHaveBeenCalledTimes(1);expect(fetch).toHaveBeenCalledWith("http://127.0.0.1:8721/api/v1/tasks/t_fixture/comments",expect.objectContaining({method:"GET",headers:{"Accept-Language":"zh-CN"}}))})
 it("list preserves the host error envelope for unknown tasks",async()=>{vi.stubGlobal("fetch",vi.fn(async()=>response({error:{code:"not_found",message:"task not found"}},404)));await expect(new KanbanApi(config).listComments("t_missing")).rejects.toMatchObject({code:"not_found",message:"task not found"})})
 it("create sends exact transport with a unique retry key per call",async()=>{const fetch=vi.fn(async()=>response(create,201));vi.stubGlobal("fetch",fetch);const api=new KanbanApi(config,{locale:"zh-CN"});await api.createComment("t_fixture","decision");await api.createComment("t_fixture","decision");expect(fetch).toHaveBeenCalledTimes(2);const calls=fetch.mock.calls as unknown as [RequestInfo | URL,RequestInit][];const [url,init]=calls[0]!;expect(url).toBe("http://127.0.0.1:8721/api/v1/tasks/t_fixture/comments");expect(init).toMatchObject({method:"POST",headers:{"Accept-Language":"zh-CN","Content-Type":"application/json"}});expect((init.headers as Record<string,string>)["X-KB-Actor"]).toBeUndefined();const firstBody=JSON.parse(init.body as string);const secondBody=JSON.parse(calls[1]![1].body as string);expect(firstBody).toMatchObject({author:"desktop-test",body:"decision"});expect(secondBody).toMatchObject({author:"desktop-test",body:"decision"});expect(firstBody.idempotency_key).toMatch(/^comment\.create:c_[0-9a-f-]+$/);expect(secondBody.idempotency_key).toMatch(/^comment\.create:c_[0-9a-f-]+$/);expect(firstBody.idempotency_key).not.toBe(secondBody.idempotency_key)})
 for(const[name,mutate]of[
  ["extra envelope",(v:any)=>({...v,meta:{}})],
  ["extra comment",(v:any)=>({data:{...v.data,claim_token:"secret"}})],
  ["bad author",(v:any)=>({data:{...v.data,author_type:"robot"}})],
  ["bad kind",(v:any)=>({data:{...v.data,kind:"other"}})],
  ["legacy string metadata",(v:any)=>({data:{...v.data,metadata:JSON.stringify(v.data.metadata)}})],
  ["missing nullable",(v:any)=>{const c={...v.data};delete c.agent_type;return{data:c}}],
  ["unsafe time",(v:any)=>({data:{...v.data,created_at:Number.MAX_SAFE_INTEGER+1}})],
 ]as const)it("create rejects "+name,async()=>{vi.stubGlobal("fetch",vi.fn(async()=>response(mutate(structuredClone(create)),201)));await expect(new KanbanApi(config).createComment("t_fixture","x")).rejects.toMatchObject({code:"invalid_response"})})
 it("create rejects malformed JSON",async()=>{vi.stubGlobal("fetch",vi.fn(async()=>new Response("{bad",{status:201})));await expect(new KanbanApi(config).createComment("t_fixture","x")).rejects.toMatchObject({code:"invalid_response"})})
 it("create consumes exact non-2xx error envelope",async()=>{vi.stubGlobal("fetch",vi.fn(async()=>response({error:{code:"invalid_input",message:"bad comment",details:{field:"kind"}}},400)));await expect(new KanbanApi(config).createComment("t_fixture","x")).rejects.toMatchObject({code:"invalid_input",message:"bad comment",details:{field:"kind"}})})
})
