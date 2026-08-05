import { readFileSync } from "node:fs"
import { afterEach, describe, expect, it, vi } from "vitest"
import { KanbanApi } from "./api"

const config={apiBaseUrl:"http://127.0.0.1:8721",actor:"desktop-test",board:"project"}
const fixture=(name:string)=>JSON.parse(readFileSync(new URL(`../../../../schemas/fixtures/api/${name}-step-response.v1.valid.json`,import.meta.url),"utf8"))
const fixtures={list:JSON.parse(readFileSync(new URL("../../../../schemas/fixtures/api/list-steps-response.v1.valid.json",import.meta.url),"utf8")),create:fixture("create"),update:fixture("update"),remove:fixture("remove"),complete:fixture("complete"),skip:fixture("skip"),reopen:fixture("reopen")}
const response=(value:unknown,status=200)=>new Response(JSON.stringify(value),{status,headers:{"Content-Type":"application/json"}})
afterEach(()=>vi.unstubAllGlobals())

describe("steps exact contracts",()=>{
  it("consumes every endpoint-specific committed success fixture",async()=>{
    const fetch=vi.fn()
    for(const value of Object.values(fixtures)) fetch.mockResolvedValueOnce(response(value))
    vi.stubGlobal("fetch",fetch);const api=new KanbanApi(config)
    expect((await api.listSteps("t_project_parent")).task_id).toBe("t_project_parent")
    expect((await api.createStep("t_project_parent",{title:"Draft checks"})).steps[0]?.title).toBe("Draft checks")
    expect((await api.updateStep("t_project_parent","st_fixture",{title:"Verify checks"})).steps[0]?.title).toBe("Verify checks")
    expect((await api.removeStep("t_project_parent","st_fixture")).steps).toEqual([])
    expect((await api.completeStep("t_project_parent","st_fixture","verified")).steps[0]?.status).toBe("done")
    expect((await api.skipStep("t_project_parent","st_fixture","not needed")).steps[0]?.status).toBe("skipped")
    expect((await api.reopenStep("t_project_parent","st_fixture","redo")).steps[0]?.status).toBe("todo")
  })

  for(const [name,mutate] of [
    ["extra envelope",(v:any)=>({...v,meta:{}})],
    ["missing nullable",(v:any)=>{const s={...v.data.steps[0]};delete s.resolution_note;return{data:{...v.data,steps:[s]}}}],
    ["unknown status",(v:any)=>({data:{...v.data,steps:[{...v.data.steps[0],status:"blocked"}]}})],
    ["unsafe integer",(v:any)=>({data:{...v.data,steps:[{...v.data.steps[0],position:Number.MAX_SAFE_INTEGER+1}]}})],
    ["extra plan field",(v:any)=>({data:{...v.data,execution_plan:{...v.data.execution_plan,guard:"bypass"}}})],
  ] as const) it(`rejects ${name}`,async()=>{vi.stubGlobal("fetch",vi.fn(async()=>response(mutate(structuredClone(fixtures.list)))));await expect(new KanbanApi(config).listSteps("t_project_parent")).rejects.toMatchObject({code:"invalid_response"})})

  it("preserves exact production transports and actors",async()=>{
    const fetch=vi.fn()
    fetch.mockResolvedValueOnce(response(fixtures.create,201)).mockResolvedValueOnce(response(fixtures.update,200))
    vi.stubGlobal("fetch",fetch)
    const api=new KanbanApi(config,{locale:"zh-CN"})
    await api.createStep("t_project_parent",{title:"Draft checks",required:true})
    await api.updateStep("t_project_parent","step_fixture",{title:"Verify checks",body:null,required:false})
    const [url,init]=(fetch.mock.calls as unknown as [RequestInfo|URL,RequestInit][])[0]!
    expect(url).toBe("http://127.0.0.1:8721/api/v1/tasks/t_project_parent/steps")
    expect(init).toMatchObject({method:"POST",headers:{"Accept-Language":"zh-CN","Content-Type":"application/json"}})
    const body=JSON.parse(init.body as string)
    expect(body).toMatchObject({title:"Draft checks",required:true,actor:"desktop-test"})
    expect(body.idempotency_key).toMatch(/^step\.create:step_[0-9a-f-]+$/)
    const [updateUrl,updateInit]=(fetch.mock.calls as unknown as [RequestInfo|URL,RequestInit][])[1]!
    expect(updateUrl).toBe("http://127.0.0.1:8721/api/v1/tasks/t_project_parent/steps/step_fixture")
    expect(updateInit).toMatchObject({method:"PATCH",headers:{"Accept-Language":"zh-CN","Content-Type":"application/json"}})
    expect(JSON.parse(updateInit.body as string)).toMatchObject({title:"Verify checks",body:null,required:false,actor:"desktop-test"})
  })
})
