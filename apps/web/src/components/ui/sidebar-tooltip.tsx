import React, { useId, useState } from 'react';
import { createPortal } from 'react-dom';
/** Portaled rail hint: remains visible outside a scrolling, clipped navigation column. */
export function SidebarTooltip({ label, enabled, children }: {
  label: string;
  enabled: boolean;
  children: React.ReactNode;
}) {
  const id = useId();
  const [position, setPosition] = useState<{
    top: number;
    left: number;
  } | null>(null);
  const show = (element: HTMLElement) => {
    if (enabled) {
      const r = element.getBoundingClientRect();
      setPosition({ top: Math.max(8, Math.min(innerHeight - 44, r.top + r.height / 2 - 17)), left: r.right + 12 });
    }
  };
  return <span className="sidebar-tooltip-target" onPointerEnter={e => {
    if (e.pointerType === 'mouse')
      show(e.currentTarget);
  }} onPointerLeave={() => setPosition(null)} onFocus={e => show(e.currentTarget)} onBlur={() => setPosition(null)} onKeyDown={e => {
    if (e.key === 'Escape')
      setPosition(null);
  }}>
    {children}
    {enabled && position && createPortal(<span id={id} className="sidebar-tooltip" role="tooltip" style={position}>
      {label}
    </span>, document.body)}
  </span>;
}
