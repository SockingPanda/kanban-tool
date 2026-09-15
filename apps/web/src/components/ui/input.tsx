import React from 'react';
import { cn } from "./classes";
export const Input = React.forwardRef<HTMLInputElement, React.InputHTMLAttributes<HTMLInputElement>>(({ className, ...props }, ref) => <input ref={ref} data-slot="input" data-dialog-autofocus={props.autoFocus || undefined} className={cn('ui-input', className)} {...props} />);
Input.displayName = 'Input';
export const Textarea = React.forwardRef<HTMLTextAreaElement, React.TextareaHTMLAttributes<HTMLTextAreaElement>>(({ className, ...props }, ref) => <textarea ref={ref} data-slot="textarea" data-dialog-autofocus={props.autoFocus || undefined} className={cn('ui-textarea', className)} {...props} />);
Textarea.displayName = 'Textarea';
export function Field({ label, children, hint, htmlFor }: {
  label: string;
  children: React.ReactNode;
  hint?: string;
  htmlFor?: string;
}) {
  return <div className="field">
    <label htmlFor={htmlFor} className="field-label">
      {label}
    </label>
    {children}
    {hint && <p className="field-hint">
      {hint}
    </p>}
  </div>;
}
export function Checkbox({ className, ...props }: React.InputHTMLAttributes<HTMLInputElement>) { return <input type="checkbox" className={cn('ui-checkbox', className)} {...props} />; }
