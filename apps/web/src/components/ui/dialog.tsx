import React, { useRef, useEffect, useId } from 'react';
import { createPortal } from 'react-dom';
import { Button, IconButton } from "./button";
import { cn } from "./classes";
export function Dialog({ open, onClose, title, description, children, footer, side = false, className = '', role = 'dialog', testId, dismissDisabled = false }: {
  open: boolean;
  onClose: () => void;
  title: string;
  description?: string;
  children?: React.ReactNode;
  role?: "dialog" | "alertdialog";
  testId?: string;
  dismissDisabled?: boolean;
  footer?: React.ReactNode;
  side?: boolean;
  className?: string;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  const titleId = useId(), descId = useId();
  useEffect(() => {
    if (!open)
      return;
    const previous = document.activeElement as HTMLElement | null;
    const d = ref.current;
    if (d && !d.open)
      d.showModal();
    return () => {
      d?.close();
      if (previous?.isConnected)
        previous.focus();
    };
  }, [open]);
  if (!open)
    return null;
  return createPortal(<dialog ref={ref} role={role} data-testid={testId} className={cn('ui-dialog', side && 'dialog-side', className)} aria-labelledby={titleId} aria-describedby={description ? descId : undefined} onCancel={e => { e.preventDefault(); if (!dismissDisabled) onClose(); }} onClick={e => {
    if (!dismissDisabled && e.target === e.currentTarget) {
      const r = e.currentTarget.getBoundingClientRect();
      if (e.clientX < r.left || e.clientX > r.right || e.clientY < r.top || e.clientY > r.bottom)
        onClose();
    }
  }}>
    <div className="dialog-heading">
      <div>
        <h2 id={titleId}>
          {title}
        </h2>
        {description && <p id={descId}>
          {description}
        </p>}
      </div>
      <IconButton icon="close" label="关闭对话框" disabled={dismissDisabled} onClick={onClose} />
    </div>
    <div className="dialog-body">
      {children}
    </div>
    {footer && <div className="dialog-footer">
      {footer}
    </div>}
  </dialog>, document.body);
}
export function ConfirmDialog({ open, title, description, onClose, onConfirm, confirmLabel = '确认', danger = false }: {
  open: boolean;
  title: string;
  description: string;
  onClose: () => void;
  onConfirm: () => void;
  confirmLabel?: string;
  danger?: boolean;
}) {
  return <Dialog open={open} title={title} onClose={onClose} description={description} footer={<>
    <Button onClick={onClose}>取消</Button>
    <Button variant={danger ? 'danger' : 'default'} onClick={onConfirm}>
      {confirmLabel}
    </Button>
  </>}>
    <div className="confirm-note">请核对操作范围后确认。</div>
  </Dialog>;
}
