import React from 'react';
import { Icon, type IconName } from "./icon";
import { cn } from './classes';
export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'default' | 'primary' | 'secondary' | 'ghost' | 'outline' | 'danger' | 'destructive';
  size?: 'sm' | 'default' | 'icon';
  icon?: IconName;
  label?: string;
  isDisabled?: boolean;
  isLoading?: boolean;
}
/** 提交状态由调用者的异步操作提供。 */
export const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(({ variant = 'outline', size = 'default', icon, className, children, label, isDisabled, isLoading, disabled, ...props }, ref) => <button ref={ref} type="button" data-slot="button" className={cn('ui-button', `button-${variant === 'primary' ? 'default' : variant === 'destructive' ? 'danger' : variant}`, size !== 'default' && `button-${size}`, className)} disabled={disabled || isDisabled || isLoading} aria-busy={isLoading || undefined} {...props}>
  {icon && <Icon name={icon} size={size === 'sm' ? 14 : 16} />}
  {children ?? label}
</button>);
Button.displayName = 'Button';
export function IconButton({ icon, label, ...props }: {
  icon: IconName;
  label: string;
} & ButtonProps) { return <Button size="icon" variant="ghost" icon={icon} aria-label={label} title={label} {...props} />; }
