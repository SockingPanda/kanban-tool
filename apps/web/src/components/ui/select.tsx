import React, { useCallback, useEffect, useId, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Icon, type IconName } from './icon';
import { cn } from './classes';
export type ChoiceOption = {
  value: string;
  label: string;
  disabled?: boolean;
  icon?: IconName;
  tone?: string;
  description?: string;
};
export interface SelectProps extends Omit<React.ButtonHTMLAttributes<HTMLButtonElement>, 'value' | 'onChange' | 'children' | 'defaultValue'> {
  value?: string;
  defaultValue?: string;
  onValueChange?: (value: string) => void;
  children?: React.ReactNode;
  options?: ChoiceOption[];
  searchable?: boolean;
  onSearchChange?: (value: string) => void;
  placeholder?: string;
  name?: string;
  required?: boolean;
}
const statusTones: Record<string, string> = { running: 'blue', in_progress: 'blue', active: 'blue', done: 'green', completed: 'green', present: 'green', blocked: 'red', cancelled: 'gray', todo: 'gray', review: 'amber', paused: 'amber', planned: 'violet', backlog: 'gray', pass: 'green', fail: 'red', urgent: 'red', high: 'amber', medium: 'blue', low: 'gray', emerald: 'green', violet: 'violet', slate: 'gray', amber: 'amber', blue: 'blue' };
function textOf(node: React.ReactNode): string {
  if (typeof node === 'string' || typeof node === 'number')
    return String(node); if (Array.isArray(node))
    return node.map(textOf).join(''); if (React.isValidElement(node))
    return textOf((node.props as {
      children?: React.ReactNode;
    }).children); return '';
}
function readOptions(children: React.ReactNode): ChoiceOption[] {
  const found: ChoiceOption[] = [];
  React.Children.forEach(children, child => {
    if (!React.isValidElement(child))
      return;
    const p = child.props as {
      value?: string;
      children?: React.ReactNode;
      disabled?: boolean;
      'data-icon'?: IconName;
      'data-tone'?: string;
    };
    if (child.type === 'option')
      found.push({ value: String(p.value ?? textOf(p.children)), label: textOf(p.children), disabled: p.disabled, icon: p['data-icon'], tone: p['data-tone'] });
    else
      found.push(...readOptions(p.children));
  });
  return found;
}
/** 可搜索的选择控件；弹出层优先使用 Popover，并保持对话框内的焦点归属。 */
function useSelect({ children, options: explicit, value, defaultValue = '', onValueChange, className, searchable, onSearchChange, placeholder = '请选择', name, required, disabled, id, ...buttonProps }: SelectProps) {
  const options = useMemo(() => explicit ?? readOptions(children), [explicit, children]);
  const [localValue, setLocalValue] = useState(defaultValue);
  const selected = value ?? localValue;
  const [open, setOpen] = useState(false), [query, setQuery] = useState(''), [active, setActive] = useState(0);
  const [host, setHost] = useState<HTMLElement | null>(null);
  const [pos, setPos] = useState({ left: 0, top: 0, width: 240, maxHeight: 320, above: false });
  const trigger = useRef<HTMLButtonElement>(null), popup = useRef<HTMLDivElement>(null), input = useRef<HTMLInputElement>(null), list = useRef<HTMLDivElement>(null);
  const listId = useId(), canSearch = searchable ?? options.length > 5;
  const label = options.find(o => o.value === selected), filtered = options.filter(o => o.label.toLowerCase().includes(query.toLowerCase().trim()));
  const close = useCallback((restore = true) => {
    setOpen(false); if (restore)
      requestAnimationFrame(() => trigger.current?.focus());
  }, []);
  const choose = (option: ChoiceOption) => {
    if (option.disabled)
      return; setLocalValue(option.value); onValueChange?.(option.value); close();
  };
  const updatePosition = useCallback(() => {
    const el = trigger.current;
    if (!el)
      return;
    const r = el.getBoundingClientRect(), width = Math.min(Math.max(r.width, 240), innerWidth - 24);
    const below = innerHeight - r.bottom - 14, above = below < 225 && r.top > below;
    const height = Math.max(110, Math.min(340, above ? r.top - 20 : below));
    setPos({ left: Math.max(12, Math.min(r.left, innerWidth - width - 12)), top: above ? r.top - 6 : r.bottom + 6, width, maxHeight: height, above });
  }, []);
  const show = (initial = '') => {
    if (disabled)
      return; setHost(trigger.current?.closest('dialog[open]') as HTMLElement ?? document.body); setQuery(initial); onSearchChange?.(initial); const index = options.findIndex(o => o.value === selected && !o.disabled); setActive(Math.max(0, index)); updatePosition(); setOpen(true);
  };
  useLayoutEffect(() => {
    if (!open || !host)
      return;
    const el = popup.current;
    if (!el)
      return;
    if ('showPopover' in el) {
      el.setAttribute('popover', 'manual');
      try {
        (el as HTMLDivElement & {
          showPopover: () => void;
        }).showPopover();
      }
      catch { /* 不支持 Popover 时保留定位弹层。 */ }
    }
    (canSearch ? input.current : list.current)?.focus({ preventScroll: true });
    return () => {
      if ('hidePopover' in el)
        try {
          (el as HTMLDivElement & {
            hidePopover: () => void;
          }).hidePopover();
        }
        catch { /* 弹层可能已被浏览器移除。 */ }
    };
  }, [open, host, canSearch]);
  useEffect(() => {
    if (!open)
      return;
    const outside = (e: PointerEvent) => {
      const target = e.target as Node; if (!popup.current?.contains(target) && !trigger.current?.contains(target))
        close(false);
    };
    const reposition = () => updatePosition();
    document.addEventListener('pointerdown', outside, true);
    window.addEventListener('resize', reposition);
    window.addEventListener('scroll', reposition, true);
    return () => { document.removeEventListener('pointerdown', outside, true); window.removeEventListener('resize', reposition); window.removeEventListener('scroll', reposition, true); };
  }, [open, close, updatePosition]);
  useEffect(() => {
    if (open)
      popup.current?.querySelector(`[data-option-index="${active}"]`)?.scrollIntoView({ block: 'nearest' });
  }, [active, open]);
  const handleKey = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      close();
    }
    else if (e.key === 'Tab') {
      e.preventDefault();
      const scope = trigger.current?.closest('dialog[open]') ?? document;
      const focusable = Array.from(scope.querySelectorAll<HTMLElement>('button:not([disabled]), input:not([disabled]):not([type=hidden]), textarea:not([disabled]), a[href], [tabindex="0"]')).filter(el => !el.closest('[data-choice-popup]') && el.getClientRects().length > 0);
      const i = focusable.indexOf(trigger.current!);
      const next = focusable[i + (e.shiftKey ? -1 : 1)];
      close(false);
      requestAnimationFrame(() => (next ?? trigger.current)?.focus());
    }
    else if (e.key === 'ArrowDown' || e.key === 'ArrowUp' || e.key === 'Home' || e.key === 'End') {
      e.preventDefault();
      const dir = e.key === 'ArrowUp' ? -1 : 1;
      let index = e.key === 'Home' ? 0 : e.key === 'End' ? filtered.length - 1 : (active + dir + filtered.length) % filtered.length;
      for (let attempts = 0;attempts < filtered.length && filtered[index]?.disabled;attempts++)
        index = (index + dir + filtered.length) % filtered.length;
      setActive(Number.isFinite(index) ? index : 0);
    }
    else if (e.key === 'Enter' || e.key === ' ' && !canSearch) {
      e.preventDefault();
      const option = filtered[active];
      if (option)
        choose(option);
    }
  };
  return {buttonProps,id,trigger,open,listId,required,disabled,className,close,show,label,placeholder,name,selected,host,popup,pos,handleKey,canSearch,input,query,filtered,active,setQuery,setActive,list,choose};
}
export function Select(props:SelectProps) {
  const state=useSelect(props);
  const {buttonProps,id,trigger,open,listId,required,disabled,className,close,show,label,placeholder,name,selected,host}=state;
  const icon = label?.icon;
  const tone = label?.tone ?? statusTones[selected];
  return <>
    <button {...buttonProps} id={id} ref={trigger} type="button" role="combobox" aria-haspopup="listbox" aria-expanded={open} aria-controls={open ? listId : undefined} aria-required={required || undefined} disabled={disabled} data-slot="select-trigger" className={cn('ui-select choice-trigger', open && 'choice-open', className)} onClick={() => open ? close() : show()} onKeyDown={e => {
      if (['ArrowDown', 'ArrowUp', 'Enter', ' '].includes(e.key)) {
        e.preventDefault();
        show();
      }
      else if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) {
        e.preventDefault();
        show(e.key);
      }
    }}>
      {icon ? <Icon name={icon} size={14} /> : tone ? <span className={'choice-dot tone-' + tone} /> : null}
      <span className="choice-value">
        {label?.label ?? placeholder}
      </span>
      <Icon name="down" size={13} className="choice-chevron" />
    </button>
    {name && <input type="hidden" name={name} value={selected} />}
    {open && host && createPortal(<ChoicePopup state={state} onSearchChange={props.onSearchChange} />, host)}
  </>;
}

function ChoicePopup({state,onSearchChange}:{state:ReturnType<typeof useSelect>;onSearchChange?:SelectProps['onSearchChange']}) {
  const {popup,pos,handleKey,canSearch,input,buttonProps,query,listId,filtered,active,setQuery,setActive,list,selected,choose}=state;
  return (<div ref={popup} data-choice-popup="true" className="choice-popup" style={{ left: pos.left, top: pos.top, width: pos.width, maxHeight: pos.maxHeight, transform: pos.above ? 'translateY(-100%)' : undefined }} onKeyDown={handleKey}>
      {canSearch && <div className="choice-search">
        <Icon name="search" size={15} />
        <input ref={input} aria-label={'搜索' + (buttonProps['aria-label'] ?? '选项')} placeholder="搜索选项…" value={query} role="combobox" aria-autocomplete="list" aria-expanded="true" aria-controls={listId} aria-activedescendant={filtered[active] ? `${listId}-${active}` : undefined} onChange={e => { setQuery(e.target.value); onSearchChange?.(e.target.value); setActive(0); }} />
      </div>}
      <div ref={list} className="choice-list" id={listId} role="listbox" aria-label={buttonProps['aria-label'] ?? '可选项目'} aria-activedescendant={!canSearch && filtered[active] ? `${listId}-${active}` : undefined} tabIndex={canSearch ? -1 : 0}>
        {filtered.map((option, i) => <div id={`${listId}-${i}`} key={option.value} role="option" aria-selected={selected === option.value} aria-disabled={option.disabled || undefined} data-option-index={i} className={cn('choice-option', active === i && 'choice-highlight', selected === option.value && 'choice-selected', option.disabled && 'choice-disabled')} onPointerMove={() => !option.disabled && setActive(i)} onMouseDown={e => e.preventDefault()} onClick={() => choose(option)}>
          {option.icon ? <Icon name={option.icon} size={15} /> : option.tone || statusTones[option.value] ? <span className={'choice-dot tone-' + (option.tone ?? statusTones[option.value])} /> : <span className="choice-option-mark" />}
          <span className="choice-option-text">
            {option.label}
            {option.description && <small>
              {option.description}
            </small>}
          </span>
          {selected === option.value && <Icon name="check" size={14} />}
        </div>)}
        {!filtered.length && <div className="choice-empty" role="status">没有匹配的选项</div>}
      </div>
      <div className="choice-footer">
        <span>{filtered.length} 个选项</span>
        <span>↑ ↓ 选择 <kbd>↵</kbd> 确认</span>
      </div>
    </div>);
}
export const Combobox = (props: SelectProps) => <Select {...props} searchable />;
