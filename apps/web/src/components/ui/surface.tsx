import { createElement, forwardRef, type HTMLAttributes, type ReactNode } from 'react';
import { cn } from "./classes";

export const Card = forwardRef<HTMLDivElement, HTMLAttributes<HTMLDivElement>>(
  ({ children, className, ...props }, ref) => <div ref={ref} className={cn('paper-card', className)} {...props}>{children}</div>,
);
Card.displayName = 'Card';

export function Heading({ level = 2, children, ...props }: HTMLAttributes<HTMLHeadingElement> & { level?: 1 | 2 | 3 | 4 | 5 | 6 }) {
  return createElement(`h${level}`, props, children);
}

export function Text({ as = 'span', children, ...props }: HTMLAttributes<HTMLElement> & { as?: 'span' | 'p' }) {
  return createElement(as, props, children);
}

export function Banner({ status = 'warning', title, description, endContent, className, ...props }: Omit<HTMLAttributes<HTMLDivElement>, 'title'> & {
  status?: 'error' | 'warning' | 'info' | 'success'; title: ReactNode; description?: ReactNode; endContent?: ReactNode;
}) {
  return <div className={cn('paper-banner', `banner-${status}`, className)} role={status === 'error' ? 'alert' : 'status'} {...props}>
    <div><strong>{title}</strong>{description && <div>{description}</div>}</div>{endContent}
  </div>;
}
