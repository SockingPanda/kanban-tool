import { useRef, useState, type ComponentProps } from 'react';
import type { InspectorMutationOutcome } from '../../application/tasks/task-inspector-mutation-state';
import { Textarea } from '../../components/ui/input';
export function TaskTextField({ value, save, ...props }: Omit<ComponentProps<typeof Textarea>, 'value' | 'onChange' | 'onBlur'> & { value: string; save: (value: string) => Promise<InspectorMutationOutcome> }) {
  const [draft, setDraft] = useState<string | null>(null);
  const submitting = useRef(false);
  if (draft !== null && draft === value) setDraft(null);
  const submit = async () => {
    if (submitting.current || draft === null || draft === value) return;
    const submitted = draft;
    submitting.current = true;
    try {
      const result = await save(submitted);
      if (result.committed && result.reconciled) setDraft(current => current === submitted ? null : current);
    } finally { submitting.current = false; }
  };
  return <Textarea {...props} value={draft ?? value} onChange={event => setDraft(event.target.value)} onBlur={() => { void submit(); }} />;
}
