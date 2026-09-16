import assert from 'node:assert/strict'
import { Buffer } from 'node:buffer'
import { fileURLToPath, URL } from 'node:url'
import process from 'node:process'
import { fromBinary } from '@bufbuild/protobuf'
import { Code, ConnectError } from '@connectrpc/connect'
import { createServer } from 'vite'

const [baseUrl, board] = process.argv.slice(2)
assert(baseUrl && board, '需要当前候选 Host URL 和隔离看板 ID')
const server = await createServer({
  root: fileURLToPath(new URL('..', import.meta.url)),
  server: { middlewareMode: true },
  appType: 'custom',
  logLevel: 'error',
})
try {
  const [{ createRpcClients }, { ErrorDetailSchema }, { DtoApiErrorCode }, { QueryResultSchema }] = await Promise.all([
    server.ssrLoadModule('/src/lib/rpc/client.ts'),
    server.ssrLoadModule('/src/generated/rpc/kanban/v1/kanban_pb.ts'),
    server.ssrLoadModule('/src/generated/rpc/kanban/v1/dto_pb.ts'),
    server.ssrLoadModule('/src/generated/rpc/kanban/v1/query_pb.ts'),
  ])
  const requests = []
  const rpc = createRpcClients(baseUrl, (input, init) => {
    const headers = new globalThis.Headers(init.headers)
    headers.set('origin', baseUrl)
    requests.push({ url: String(input), contentType: headers.get('content-type') })
    return globalThis.fetch(input, { ...init, headers })
  })
  const created = await rpc.business.createTask({ board, title: '完整生成的浏览器客户端', description: '保留正文', actor: 'Fetch 作者' })
  assert(created.data?.id)
  const result = await rpc.business.getTaskDetails({ taskId: created.data.id })
  assert.equal(result.data.task.description, '保留正文')
  assert.equal(typeof result.data.task.lockVersion, 'bigint')
  await assert.rejects(rpc.business.getTask({ taskId: 't_missing' }), error => {
    const failure = ConnectError.from(error)
    assert.equal(failure.code, Code.NotFound)
    const details = failure.findDetails(ErrorDetailSchema)
    assert.equal(details.length, 1)
    assert.equal(details[0].code, DtoApiErrorCode.NOT_FOUND)
    assert.match(details[0].message, /t_missing/)
    return true
  })
  const abort = new globalThis.AbortController()
  let completed = false
  const chunks = []
  try {
    for await (const frame of rpc.query.watchQueries({ protocolVersion: 1, queries: [{
      clientQueryId: 'smoke', projectionVersion: 1, query: { case: 'getTaskDetails', value: { taskId: created.data.id } },
    }] }, { signal: abort.signal })) {
      const body = frame.body
      if (body.case === 'heartbeat') continue
      assert.equal(frame.clientQueryId, 'smoke')
      if (body.case === 'begin') assert(body.value.snapshot)
      if (body.case === 'chunk') chunks.push(body.value.data)
      if (body.case === 'end') {
        const snapshot = fromBinary(QueryResultSchema, Buffer.concat(chunks))
        assert.equal(snapshot.result.case, 'getTaskDetails')
        assert.equal(snapshot.result.value.data.task.description, '保留正文')
        completed = true
        abort.abort()
      }
    }
  } catch (error) {
    assert.equal(ConnectError.from(error).code, Code.Canceled)
  }
  assert(completed)
  assert(requests.every(request => request.contentType === 'application/grpc-web+proto'))
  assert(requests.every(request => new URL(request.url).pathname.startsWith('/kanban.v1.')))
  globalThis.console.log('正式生成客户端：binary Fetch、完整详情、bigint、标准错误详情、完整 QueryService 及取消通过')
} finally {
  await server.close()
}
