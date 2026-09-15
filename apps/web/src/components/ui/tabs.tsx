import React from 'react';
import { cn } from "./classes";
import { Icon, IconName } from "./icon";
export function Tabs({ value, onChange, items, variant = 'line', label = '视图选项' }: {
  value: string;
  onChange: (value: string) => void;
  items: {
    value: string;
    label: string;
    count?: number;
    icon?: IconName;
    disabled?: boolean;
  }[];
  variant?: 'line' | 'segment';
  label?: string;
}) {
  return <div className={cn('ui-tabs', 'tabs-' + variant)} role="group" aria-label={label}>
    {items.map(item => <button key={item.value} type="button" disabled={item.disabled} title={item.disabled ? "尚未接入" : undefined} aria-pressed={value === item.value} className={value === item.value ? 'active' : ''} onClick={() => onChange(item.value)}>
      {item.icon && <Icon name={item.icon} size={15} />}
      <span>
        {item.label}
      </span>
      {item.count !== undefined && <span className="tab-count">
        {item.count}
      </span>}
    </button>)}
  </div>;
}
