import { useId, type ReactNode } from 'react';
import './settings-section.css';

/** 设置分组承载标题和说明，数据与持久化由调用者负责。 */
export function SettingsSection({ title, description, children }: {
  title: string; description?: string; children: ReactNode;
}) {
  const id = useId();
  return <section className="settings-section" aria-labelledby={id}>
    <header><h2 id={id}>{title}</h2>{description && <p>{description}</p>}</header>
    <div className="settings-section-content">{children}</div>
  </section>;
}

/** 为按钮、输入框和选择控件建立显式标签关联。 */
export function SettingsRow({ label, description, controlId, children }: {
  label: string; description?: string; controlId?: string; children: ReactNode;
}) {
  return <div className="settings-control-row">
    <div className="settings-control-copy">
      {controlId ? <label htmlFor={controlId}>{label}</label> : <span>{label}</span>}
      {description && <p>{description}</p>}
    </div>
    <div className="settings-control-value">{children}</div>
  </div>;
}
