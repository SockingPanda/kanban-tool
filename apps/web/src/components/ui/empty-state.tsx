import React from 'react';
import { Icon, IconName } from "./icon";
export function EmptyState({ icon = 'box', title, description, action }: {
  icon?: IconName;
  title: string;
  description: string;
  action?: React.ReactNode;
}) {
  return <div className="empty-state">
    <span className="empty-icon">
      <Icon name={icon} size={27} />
    </span>
    <h3>
      {title}
    </h3>
    <p>
      {description}
    </p>
    {action}
  </div>;
}
