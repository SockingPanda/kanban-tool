import React from 'react';
export function Progress({ value, label, className = '' }: {
  value: number;
  label?: string;
  className?: string;
}) {
  return <div data-slot="progress" role="progressbar" aria-label={label ?? '完成进度'} aria-valuenow={value} aria-valuemin={0} aria-valuemax={100} className={'progress-track ' + className}>
    <span style={{ width: `${Math.min(100, Math.max(0, value))}%` }} />
  </div>;
}
export function SegmentedProgress({ done, active, total }: {
  done: number;
  active: number;
  total: number;
}) {
  return <div className="segmented-progress" aria-label={`${done} 已完成，${active} 进行中，共 ${total}`}>
    <span className="segment-done" style={{ width: `${total ? done / total * 100 : 0}%` }} />
    <span className="segment-active" style={{ width: `${total ? active / total * 100 : 0}%` }} />
  </div>;
}
export function Ring({ value, size = 64 }: {
  value: number;
  size?: number;
}) {
  const r = 26, c = 2 * Math.PI * r; return <svg className="progress-ring" width={size} height={size} viewBox="0 0 64 64" role="img" aria-label={`已完成 ${value}%`}>
    <circle cx="32" cy="32" r={r} fill="none" stroke="var(--border)" strokeWidth="4" />
    <circle cx="32" cy="32" r={r} fill="none" stroke="var(--primary)" strokeWidth="4" strokeDasharray={`${value / 100 * c} ${c}`} transform="rotate(-90 32 32)" strokeLinecap="round" />
    <text x="32" y="36" textAnchor="middle" fill="currentColor" fontSize="13" fontWeight="600">{value}%</text>
  </svg>;
}
