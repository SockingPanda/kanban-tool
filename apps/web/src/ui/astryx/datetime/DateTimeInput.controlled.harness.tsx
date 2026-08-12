/* eslint-disable react-refresh/only-export-components */

import { useState } from "react"
import { createRoot } from "react-dom/client"

import { DateTimeInput, type ISODateTimeString } from "./DateTimeInput"

function ControlledHarness() {
  const [value, setValue] = useState<ISODateTimeString | undefined>("2026-08-10T08:00" as ISODateTimeString)
  const [lastChange, setLastChange] = useState("none")

  const handleChange = (nextValue: ISODateTimeString | undefined) => {
    setLastChange(nextValue ?? "undefined")
    setValue(nextValue)
  }

  return (
    <main>
      <DateTimeInput
        clearLabel="清除截止时间"
        dateLabel="截止日期"
        data-testid="datetime-field"
        label="截止时间"
        min="2026-08-09T09:00"
        onChange={handleChange}
        timeLabel="截止时间"
        value={value}
      />
      <output data-testid="last-change">{lastChange}</output>
    </main>
  )
}

const root = document.createElement("main")
document.body.replaceChildren(root)
createRoot(root).render(<ControlledHarness />)
