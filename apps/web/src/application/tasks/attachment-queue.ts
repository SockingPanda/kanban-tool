import type { Integer } from '../../domain/integer';
/** 纯队列状态机。上传失败或取消后仍保留原 ID，避免响应丢失后的重复附件。 */
export const MAX_FILE_BYTES = 256 * 1024 * 1024;
export const MAX_QUEUE_ITEMS = 64;
export type UploadStatus = 'queued' | 'uploading' | 'committing' | 'succeeded' | 'error' | 'canceled';
export interface AttachmentRejection { readonly code: string; readonly filename?: string }
export interface UploadReceipt { readonly id: string; readonly task_id: string; readonly size_bytes: Integer }
export interface UploadEntry {
  readonly id: string;
  readonly file: File;
  readonly status: UploadStatus;
  readonly sent: number;
  readonly message: string | null;
  readonly committed: boolean;
  readonly reconciled: boolean;
}
export interface QueueTransport {
  upload(taskId: string, file: File, id: string, options: { signal: AbortSignal; onProgress: (sent: number, total: number) => void }): Promise<UploadReceipt>;
}
export interface AttachmentQueueOptions {
  readonly taskId: string;
  readonly transport: QueueTransport;
  readonly reconcile: () => Promise<void>;
  readonly id?: () => string;
  readonly concurrency?: number;
}
function invalidPath(value: string, extended = false): boolean {
  return Array.from(value).some(char => { const code = char.charCodeAt(0); return char === '/' || char === '\\' || code < 32 || (extended && code >= 127 && code <= 159); });
}
export function attachmentFileError(file: File): string | null {
  if (file.size > MAX_FILE_BYTES) return 'file.too_large';
  if (!file.name.trim() || file.name === '.' || file.name === '..' || invalidPath(file.name, true)) return 'file.name_invalid';
  if (new TextEncoder().encode(file.name).length > 255) return 'file.name_long';
  return null;
}
function messageOf(error: unknown): string { return error instanceof Error ? error.message : 'file.unknown'; }
function makeId(): string { return `a_${crypto.randomUUID().replaceAll('-', '')}`; }

export class AttachmentQueue {
  #entries: readonly UploadEntry[] = [];
  #listeners = new Set<() => void>();
  #active = new Map<string, AbortController>();
  #disposed = false;
  readonly #options: AttachmentQueueOptions;
  readonly #concurrency: number;
  constructor(options: AttachmentQueueOptions) {
    if (!options.taskId || options.taskId.length > 128 || invalidPath(options.taskId)) throw new Error('file.owner_invalid');
    this.#options = options;
    const limit = options.concurrency ?? 2;
    this.#concurrency = Number.isFinite(limit) ? Math.max(1, Math.min(2, Math.floor(limit))) : 2;
  }
  getSnapshot = (): readonly UploadEntry[] => this.#entries;
  subscribe = (listener: () => void): (() => void) => { this.#listeners.add(listener); return () => { this.#listeners.delete(listener); }; };
  enqueue(files: Iterable<File>): readonly AttachmentRejection[] {
    if (this.#disposed) return [{ code: 'file.queue_closed' }];
    const rejected: AttachmentRejection[] = [];
    for (const file of files) {
      const error = attachmentFileError(file);
      if (error) { rejected.push({ filename: file.name, code: error }); continue; }
      if (this.#entries.length >= MAX_QUEUE_ITEMS) { rejected.push({ code: 'file.queue_full' }); break; }
      const id = (this.#options.id ?? makeId)();
      if (!/^a_[A-Za-z0-9_-]+$/.test(id) || this.#entries.some(entry => entry.id === id)) throw new Error('file.id_invalid');
      this.#entries = [...this.#entries, { id, file, status: 'queued', sent: 0, message: null, committed: false, reconciled: false }];
    }
    this.#emit(); this.#pump(); return rejected;
  }
  retry(id: string): void {
    const entry = this.#entries.find(item => item.id === id);
    if (!entry || this.#disposed || this.#active.has(id) || !['error', 'canceled'].includes(entry.status)) return;
    this.#update(id, { status: 'queued', message: null, sent: 0 }); this.#pump();
  }
  cancel(id: string): void {
    const entry = this.#entries.find(item => item.id === id);
    if (!entry || !['queued', 'uploading', 'committing'].includes(entry.status)) return;
    const active = this.#active.get(id);
    this.#update(id, { status: 'canceled', message: active ? 'file.cancel_unknown' : 'file.cancel_unsent' });
    active?.abort();
    // active 槽位必须等原 Promise settled 才释放，不能同时对同一 ID 开始第二次传输。
  }
  clearSettled(): void {
    this.#entries = this.#entries.filter(entry => this.#active.has(entry.id) || entry.status !== 'succeeded' || !entry.reconciled);
    this.#emit();
  }
  async reconcile(): Promise<void> {
    await this.#options.reconcile();
    if (this.#disposed) return;
    this.#entries = this.#entries.map(entry => entry.committed ? { ...entry, reconciled: true, message: null } : entry);
    this.#emit();
  }
  dispose(): void {
    this.#disposed = true;
    for (const controller of this.#active.values()) controller.abort();
    this.#listeners.clear();
  }
  #update(id: string, patch: Partial<Omit<UploadEntry, 'id' | 'file'>>): void {
    if (this.#disposed) return;
    this.#entries = this.#entries.map(entry => entry.id === id ? { ...entry, ...patch } : entry);
    this.#emit();
  }
  #emit(): void { if (!this.#disposed) for (const listener of this.#listeners) listener(); }
  #pump(): void {
    if (this.#disposed) return;
    while (this.#active.size < this.#concurrency) {
      const entry = this.#entries.find(item => item.status === 'queued');
      if (!entry) break;
      const controller = new AbortController(); this.#active.set(entry.id, controller);
      this.#update(entry.id, { status: 'uploading' });
      void this.#run(entry, controller);
    }
  }
  async #run(entry: UploadEntry, controller: AbortController): Promise<void> {
    try {
      const receipt = await this.#options.transport.upload(this.#options.taskId, entry.file, entry.id, {
        signal: controller.signal,
        onProgress: (sent) => {
          if (controller.signal.aborted) return;
          const bytes = Math.max(0, Math.min(entry.file.size, Number.isFinite(sent) ? sent : 0));
          this.#update(entry.id, { sent: bytes, status: bytes >= entry.file.size ? 'committing' : 'uploading' });
        },
      });
      if (receipt.id !== entry.id || receipt.task_id !== this.#options.taskId || BigInt(receipt.size_bytes) !== BigInt(entry.file.size)) throw new Error('file.invalid_receipt');
      // 即使 abort 与成功响应交错，明确的服务端 receipt 仍是已提交的证据。
      if (this.#disposed) return;
      this.#update(entry.id, { status: 'succeeded', committed: true, sent: entry.file.size, message: null });
      try {
        await this.#options.reconcile();
        this.#update(entry.id, { reconciled: true });
      } catch {
        this.#update(entry.id, { message: 'file.refresh_committed' });
      }
    } catch (error) {
      if (!this.#disposed) this.#update(entry.id, {
        status: controller.signal.aborted ? 'canceled' : 'error',
        message: controller.signal.aborted ? 'file.cancel_unknown' : messageOf(error),
      });
    } finally {
      this.#active.delete(entry.id); this.#pump();
    }
  }
}
