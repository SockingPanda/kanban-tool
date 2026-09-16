import { expect, test } from 'vitest';
import { AttachmentQueue, attachmentFileError, MAX_FILE_BYTES, type QueueTransport, type UploadReceipt } from './attachment-queue';

function fixture(reconcile = async () => {}) {
  const calls: { id: string; file: File; signal: AbortSignal; progress: (sent: number, total: number) => void; resolve: (receipt?: UploadReceipt) => void; reject: (error: Error) => void }[] = [];
  const transport: QueueTransport = { upload(owner, file, id, options) {
    return new Promise((resolve, reject) => calls.push({ id, file, signal: options.signal, progress: options.onProgress, resolve: receipt => resolve(receipt ?? { id, task_id: owner, size_bytes: BigInt(file.size) }), reject }));
  } };
  let sequence = 0;
  const queue = new AttachmentQueue({ taskId: 'obj_module', transport, reconcile, id: () => `a_${++sequence}` });
  return { queue, calls };
}
const tick = () => new Promise(resolve => setTimeout(resolve, 0));
const file = (name = '附件.txt') => new File(['abc'], name);

test('上传发送完成后仍等提交；两个槽位必须等原 Promise 结束才释放', async () => {
  const { queue, calls } = fixture();
  queue.enqueue([file('one'), file('two'), file('three')]);
  expect(calls).toHaveLength(2);
  calls[0].progress(3, 3);
  expect(queue.getSnapshot()[0]).toMatchObject({ status: 'committing', committed: false });
  queue.cancel(calls[0].id);
  expect(calls[0].signal.aborted).toBe(true);
  expect(calls[1].signal.aborted).toBe(false);
  await tick(); expect(calls).toHaveLength(2);
  calls[0].resolve(); await tick();
  expect(calls).toHaveLength(3);
  expect(queue.getSnapshot()[0]).toMatchObject({ status: 'succeeded', committed: true, reconciled: true });
  queue.dispose(); calls[1].resolve(); calls[2].resolve(); await tick();
});

test('不确定失败重试保留原 ID 和 File；提交后刷新失败只重读', async () => {
  let fail = true;
  const { queue, calls } = fixture(async () => { if (fail) throw new Error('offline'); });
  const selected = file(); queue.enqueue([selected]);
  calls[0].reject(new Error('acknowledgement lost')); await tick();
  queue.retry(calls[0].id);
  expect(calls[1].id).toBe(calls[0].id); expect(calls[1].file).toBe(selected);
  calls[1].resolve(); await tick();
  expect(queue.getSnapshot()[0]).toMatchObject({ committed: true, reconciled: false });
  queue.retry(calls[0].id); queue.clearSettled(); expect(calls).toHaveLength(2);
  expect(queue.getSnapshot()).toHaveLength(1);
  fail = false; await queue.reconcile(); queue.clearSettled(); expect(queue.getSnapshot()).toHaveLength(0);
  queue.dispose();
});

test('错误 receipt 不被承认为提交；关闭队列停止后续上传', async () => {
  const { queue, calls } = fixture(); queue.enqueue([file()]);
  calls[0].resolve({ id: 'a_other', task_id: 'obj_module', size_bytes: 3 }); await tick();
  expect(queue.getSnapshot()[0]).toMatchObject({ status: 'error', committed: false });
  queue.dispose(); expect(queue.enqueue([file()])).toEqual([{ code: 'file.queue_closed' }]);
});

test('文件名按 UTF-8 字节校验，拒绝路径和控制字符；空文件与大小边界', () => {
  for (const name of ['../secret', 'a\\b', 'a\n.txt', 'a\u0085.txt', '猫'.repeat(86)]) expect(attachmentFileError(file(name))).not.toBeNull();
  expect(attachmentFileError(new File([], 'empty'))).toBeNull();
  const boundary = new File([], 'boundary');
  Object.defineProperty(boundary, 'size', { value: MAX_FILE_BYTES, configurable: true });
  expect(attachmentFileError(boundary)).toBeNull();
  Object.defineProperty(boundary, 'size', { value: MAX_FILE_BYTES + 1 });
  expect(attachmentFileError(boundary)).toBe('file.too_large');
});
