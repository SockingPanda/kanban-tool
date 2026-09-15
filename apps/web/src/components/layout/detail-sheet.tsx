import { useLayoutEffect } from "react";
import React, { useEffect, useId, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { IconButton } from "../ui/button";
import { cn } from "../ui/classes";
import { Dialog } from "../ui/dialog";
/** App-owned floating right panel. Map, task and source details share this shell.
 * Desktop remains non-modal so the canvas can still be operated.
 * Narrow screens use the existing modal dialog for focus containment.
 */
export function DetailSheet({ open, title, description, onClose, children, footer, className = '', actions, closeLabel = '关闭详情', dismissDisabled = false }: {
  closeLabel?: string;
  dismissDisabled?: boolean;
  open: boolean;
  title: string;
  description?: string;
  onClose: () => void;
  children: React.ReactNode;
  footer?: React.ReactNode;
  className?: string;
  actions?: React.ReactNode;
}) {
  const id = useId();
  const [wide, setWide] = useState(false);
  const [host, setHost] = useState<HTMLElement | null>(null);
  const [mobile, setMobile] = useState(() => typeof window !== 'undefined' && typeof window.matchMedia === 'function' && window.matchMedia('(max-width: 760px)').matches);
  const latestClose = useRef(onClose);
  useLayoutEffect(() => { latestClose.current = () => { if (!dismissDisabled) onClose(); }; });
  const panel = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    setHost(document.getElementById('global-detail-root'));
    const media = matchMedia('(max-width: 760px)');
    const update = () => setMobile(media.matches);
    media.addEventListener('change', update);
    return () => media.removeEventListener('change', update);
  }, []);
  useEffect(() => {
    if (!open || mobile)
      return;
    const previous = document.activeElement as HTMLElement | null;
    const escape = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && !e.defaultPrevented && !document.querySelector('dialog:modal, [data-choice-popup], [data-navigation-overlay]')) {
        e.preventDefault();
        latestClose.current();
      }
    };
    window.addEventListener('keydown', escape);
    // Do not steal focus from an opened dropdown or the map. F6 enters the dock.
    const focusDock = (e: KeyboardEvent) => {
      if (e.key === 'F6') {
        e.preventDefault();
        panel.current?.focus();
      }
    };
    window.addEventListener('keydown', focusDock);
    return () => {
      window.removeEventListener('keydown', escape);
      window.removeEventListener('keydown', focusDock);
      if (previous?.isConnected && document.activeElement === document.body)
        previous.focus();
    };
  }, [open, mobile]);
  if (!open)
    return null;
  if (mobile)
    return <Dialog open side dismissDisabled={dismissDisabled} closeLabel={closeLabel} title={title} description={description} onClose={onClose} footer={footer} className={cn('detail-mobile', className)}>
      {actions && <div className="mobile-detail-actions">
        {actions}
      </div>}
      {children}
    </Dialog>;
  if (!host)
    return null;
  return createPortal(<dialog open ref={panel} aria-modal="false" aria-labelledby={id} aria-describedby={description ? id + '-desc' : undefined} tabIndex={-1} className={cn('detail-sheet', wide && 'detail-wide', className)} data-testid="detail-sheet">
    <header className="detail-sheet-top">
      <div>
        <span id={id}>
          {title}
        </span>
        {description && <small id={id + '-desc'}>
          {description}
        </small>}
      </div>
      <div className="detail-sheet-actions">
        <span className="detail-overlay-label">浮层</span>
        {actions}
        <IconButton icon="panel" label={wide ? "收窄详情" : "展开详情宽度"} onClick={() => setWide(v => !v)} />
        <IconButton icon="close" label={closeLabel} disabled={dismissDisabled} onClick={onClose} />
      </div>
    </header>
    <div className="detail-sheet-body">
      {children}
    </div>
    {footer && <footer className="detail-sheet-footer">
      {footer}
    </footer>}
  </dialog>, host);
}
