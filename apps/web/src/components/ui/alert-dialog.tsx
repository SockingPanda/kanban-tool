import { Dialog } from "./dialog";
import { Button } from "./button";

export function AlertDialog({ isOpen, onOpenChange, title, description, cancelLabel, actionLabel, actionVariant, isActionLoading, onAction, 'data-testid': testId }: {
  isOpen: boolean; onOpenChange: (open: boolean) => void; title: string; description: string;
  cancelLabel: string; actionLabel: string; actionVariant: 'destructive' | 'primary';
  isActionLoading: boolean; onAction: () => void | Promise<void>; 'data-testid'?: string;
}) {
  return <Dialog open={isOpen} title={title} description={description} onClose={() => { if (!isActionLoading) onOpenChange(false); }} role="alertdialog" testId={testId} dismissDisabled={isActionLoading} footer={<>
    <Button variant="secondary" disabled={isActionLoading} onClick={() => onOpenChange(false)}>{cancelLabel}</Button>
    <Button variant={actionVariant} isLoading={isActionLoading} onClick={() => void onAction()}>{actionLabel}</Button>
  </>} />;
}
