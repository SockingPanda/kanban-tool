import React from 'react';
import { cn } from "./classes";
export function Badge({ children, label, variant, tone = 'neutral', className = '' }: {
  children?: React.ReactNode;
  label?: string;
  variant?: string;
  tone?: string;
  className?: string;
}) {
  return <span data-slot="badge" className={cn('ui-badge', `tone-${variant ?? tone}`, className)}>
    {children ?? label}
  </span>;
}
