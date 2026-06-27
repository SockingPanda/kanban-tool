import { useCallback, useMemo, useState } from "react"

export function useTaskCreationDialogState() {
  const [open, setOpen] = useState(false)
  const [title, setTitle] = useState("")
  const [description, setDescription] = useState("")
  const [firstStepTitle, setFirstStepTitle] = useState("")

  const reset = useCallback(() => {
    setTitle("")
    setDescription("")
    setFirstStepTitle("")
  }, [])

  return useMemo(
    () => ({
      open,
      setOpen,
      title,
      setTitle,
      description,
      setDescription,
      firstStepTitle,
      setFirstStepTitle,
      reset,
    }),
    [description, firstStepTitle, open, reset, title],
  )
}
