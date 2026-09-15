import React from 'react';
import { Button } from "../ui/button";
export function PageHeader({ eyebrow, title, description, actions, back }: {
  eyebrow?: string;
  title: string;
  description?: string;
  actions?: React.ReactNode;
  back?: () => void;
}) {
  return <header className="page-heading">
    <div>
      {back && <Button variant="ghost" size="sm" icon="left" onClick={back} className="back-button">返回</Button>}
      {eyebrow && <div className="eyebrow">
        {eyebrow}
      </div>}
      <h1>
        {title}
      </h1>
      {description && <p>
        {description}
      </p>}
    </div>
    {actions && <div className="heading-actions">
      {actions}
    </div>}
  </header>;
}
