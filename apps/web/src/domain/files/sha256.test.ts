import { createHash, randomBytes } from 'node:crypto';
import { expect, test } from 'vitest';
import { hashBlob, Sha256 } from './sha256';

test.each([0, 1, 55, 56, 63, 64, 65, 65535, 65536, 1000000])('分块 SHA-256 与独立实现一致：%i 字节', size => {
  const bytes = randomBytes(size), hash = new Sha256();
  for (let offset = 0; offset < size; offset += 37) hash.update(bytes.subarray(offset, offset + 37));
  const expected = createHash('sha256').update(bytes).digest('hex');
  expect(hash.hex()).toBe(expected);
  expect(hash.hex()).toBe(expected);
});

test('Blob 跨块、空文件及提前取消', async () => {
  const bytes = Uint8Array.from(randomBytes(65536 * 3 + 17));
  expect(await hashBlob(new Blob([bytes]))).toBe(createHash('sha256').update(bytes).digest('hex'));
  expect(await hashBlob(new Blob([]))).toBe(createHash('sha256').digest('hex'));
  const controller = new AbortController(); controller.abort();
  await expect(hashBlob(new Blob(['x']), controller.signal)).rejects.toMatchObject({ name: 'AbortError' });
});
