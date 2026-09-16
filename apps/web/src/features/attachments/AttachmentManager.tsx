import { useEffect, useId, useRef, useState, type DragEvent } from 'react';
import type { Locale } from '../../platform/preferences/preferences';
import type { AttachmentTransferClient, TransferredAttachment } from '../../application/data/attachment-transfer';
import { useAttachmentQueue } from '../../application/tasks/use-attachment-queue';
import { Button } from '../../components/ui/button';
import { attachmentCopy, attachmentProblem } from './copy';
import type { AttachmentRejection } from '../../application/tasks/attachment-queue';
import styles from './attachments.module.css';

interface Props {
  readonly taskId: string;
  readonly attachments: readonly TransferredAttachment[];
  readonly client: AttachmentTransferClient | null;
  readonly reconcile: () => Promise<void>;
  readonly remove: (id: string) => Promise<{ committed: boolean; reconciled: boolean }>;
  readonly loading?: boolean;
  readonly error?: string | null;
  readonly disabled?: boolean;
  readonly locale?: Locale;
  readonly onBusyChange?: (busy: boolean) => void;
}
function sizeText(value: number | bigint): string { const size = Number(value); return size >= 1048576 ? `${(size / 1048576).toFixed(1)} MiB` : size >= 1024 ? `${(size / 1024).toFixed(1)} KiB` : `${size} B`; }
function errorText(error: unknown): string { return error instanceof Error ? error.message : String(error); }
function fileDrag(event:DragEvent) {return Array.from(event.dataTransfer.types).includes('Files');}
export function AttachmentManager(props: Props) {
  const { taskId, client, attachments, reconcile, remove, onBusyChange } = props;
  const locale = props.locale ?? 'zh';
  const copy = attachmentCopy(locale);
  const id = useId();
  const inputRef = useRef<HTMLInputElement>(null);
  const queue = useAttachmentQueue(taskId, client, reconcile, onBusyChange);
  const [messages, setMessages] = useState<readonly AttachmentRejection[]>([]);
  const [dragging, setDragging] = useState(false);
  const [confirmation, setConfirmation] = useState<string | null>(null);
  const [deleting, setDeleting] = useState<string | null>(null);
  const [downloading, setDownloading] = useState<string | null>(null);
  const downloadAbort = useRef<AbortController | null>(null);
  useEffect(() => () => { downloadAbort.current?.abort(); }, []);
  const download = async (attachmentId:string) => {
    if (!client || downloading) return;
    const controller = new AbortController(); downloadAbort.current = controller; setDownloading(attachmentId);
    try {
      const result = await client.download(taskId, attachmentId, controller.signal);
      const url = URL.createObjectURL(result.blob); const anchor = document.createElement('a');
      anchor.href = url; anchor.download = result.filename; document.body.append(anchor); anchor.click(); anchor.remove();
      setTimeout(() => URL.revokeObjectURL(url), 60000);
    } catch(error) { if (!controller.signal.aborted) setMessages([{ code:errorText(error) }]); }
    finally { if (!controller.signal.aborted) setDownloading(null); if (downloadAbort.current === controller) downloadAbort.current = null; }
  };
  const blocked = props.disabled || !client;
  const add = (files: Iterable<File>) => { if (!blocked) setMessages(queue.enqueue(files)); };
  const refresh = async () => { try { await queue.reconcile(); setMessages([]); } catch (error) { setMessages([{ code: errorText(error) }]); } };
  const deleteConfirmed = async (attachmentId: string) => {
    if (deleting) return;
    setDeleting(attachmentId);
    try {
      const result = await remove(attachmentId);
      if (!result.committed) throw new Error('file.unlink_failed');
      setConfirmation(null);
      if (!result.reconciled) setMessages([{ code: 'file.refresh_committed' }]);
    } catch (error) { setMessages([{ code: errorText(error) }]); }
    finally { setDeleting(null); }
  };
  return <section className={styles.panel} aria-labelledby={`${id}-heading`} data-testid="attachment-manager">
    <header className={styles.heading}><h3 id={`${id}-heading`}>{copy.title} <span>{attachments.length}</span></h3><Button size="sm" onClick={() => { void refresh(); }}>{copy.refresh}</Button></header>
    <div className={`${styles.dropzone} ${dragging ? styles.dragging : ''}`} tabIndex={blocked ? -1 : 0} role="group" aria-label={copy.drop}
      onDragOver={event => { if (fileDrag(event)) { event.preventDefault(); event.dataTransfer.dropEffect = blocked ? 'none' : 'copy'; if (!blocked) setDragging(true); } }}
      onDragLeave={event => { const next = event.relatedTarget; if (!(next instanceof Node) || !event.currentTarget.contains(next)) setDragging(false); }}
      onDrop={event => { if (fileDrag(event)) { event.preventDefault(); setDragging(false); add(Array.from(event.dataTransfer.files)); } }}
      onPaste={event => { if (!blocked && event.clipboardData.files.length) { event.preventDefault(); add(Array.from(event.clipboardData.files)); } }}>
      <p>{copy.drop}</p><p className={styles.hint}>{copy.limit}</p>
      <input data-testid="attachment-file" ref={inputRef} className={styles.fileInput} type="file" multiple aria-label={copy.choose} disabled={blocked}
        onChange={event => { add(Array.from(event.currentTarget.files ?? [])); event.currentTarget.value = ''; }} />
      <Button variant="secondary" disabled={blocked} onClick={() => inputRef.current?.click()}>{copy.choose}</Button>
    </div>
    {!client && <p role="status">{copy.unavailable}</p>}
    {props.error && <p role="alert" className={styles.error}>{props.error}</p>}
    {messages.length > 0 && <div role="alert" className={styles.error}>{[...new Map(messages.map(message=>[JSON.stringify(message),message])).entries()].map(([key,message]) => <p key={key}>{message.filename && <span translate="no">{message.filename}: </span>}{attachmentProblem(locale,message.code)}</p>)}</div>}
    {queue.entries.length > 0 && <div className={styles.queue}>
      {queue.entries.map(entry => <div className={styles.row} key={entry.id} data-upload-status={entry.status}>
        <div className={styles.main}><strong title={entry.file.name}>{entry.file.name}</strong><span>{copy.status[entry.status]} · {sizeText(entry.file.size)}</span>
          <progress aria-label={`${entry.file.name} ${copy.status[entry.status]}`} value={entry.sent} max={Math.max(1, entry.file.size)} />
          {entry.message && <p className={styles.hint} role={entry.status === 'error' ? 'alert' : 'status'}>{attachmentProblem(locale,entry.message)}</p>}
        </div>
        {['queued', 'uploading', 'committing'].includes(entry.status) && <Button size="sm" onClick={() => queue.cancel(entry.id)}>{copy.cancel}</Button>}
        {['error', 'canceled'].includes(entry.status) && <Button size="sm" disabled={blocked} onClick={() => queue.retry(entry.id)}>{copy.retry}</Button>}
      </div>)}
      <Button size="sm" onClick={queue.clear}>{copy.clear}</Button>
    </div>}
    {props.loading && <p role="status">{copy.loading}</p>}
    {!props.loading && attachments.length === 0 && <p className={styles.hint}>{copy.empty}</p>}
    <ul className={styles.list}>{attachments.map(attachment => <li data-testid="attachment-row" key={attachment.id} className={styles.row}>
      <div className={styles.main}><strong title={attachment.filename}>{attachment.filename}</strong><span>{sizeText(attachment.size_bytes)} · {attachment.content_type ?? 'application/octet-stream'}</span>
        <details><summary>{copy.identity}</summary><code>{attachment.id}<br />{attachment.sha256 ?? copy.hashMissing}</code></details>
      </div>
      <div className={styles.actions}>
        {client && <Button data-testid="attachment-download" size="sm" disabled={Boolean(downloading)} onClick={() => { void download(attachment.id); }}>{downloading === attachment.id ? copy.downloading : copy.download}</Button>}
        {confirmation === attachment.id ? <><Button size="sm" disabled={Boolean(deleting)} onClick={() => { void deleteConfirmed(attachment.id); }}>{copy.confirm}</Button><Button size="sm" disabled={Boolean(deleting)} onClick={() => setConfirmation(null)}>{copy.keep}</Button></>
          : <Button data-testid="attachment-delete" size="sm" disabled={props.disabled || Boolean(deleting)} onClick={() => setConfirmation(attachment.id)}>{copy.remove}</Button>}
      </div>
    </li>)}</ul>
  </section>;
}
