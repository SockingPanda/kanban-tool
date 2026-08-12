import { useState } from "react"
import { createRoot } from "react-dom/client"

import { MultiSelector } from "../../src/ui/astryx/selectors/MultiSelector"
import { Typeahead, type SearchSource } from "../../src/ui/astryx/selectors/Typeahead"

type FixtureItem = { readonly id: string; readonly label: string }

const oldItem: FixtureItem = { id: "old", label: "Old result" }
const newItem: FixtureItem = { id: "new", label: "New result" }
let resolveOld: ((items: FixtureItem[]) => void) | undefined

const sourceA: SearchSource<FixtureItem> = {
  search: () => new Promise((resolve) => {
    resolveOld = resolve
  }),
  bootstrap: () => [],
}

const sourceB: SearchSource<FixtureItem> = {
  search: () => [newItem],
  bootstrap: () => [newItem],
}

export function Fixture() {
  const [selected, setSelected] = useState<readonly string[]>([])
  const [multiDisabled, setMultiDisabled] = useState(false)
  const [typeaheadDisabled, setTypeaheadDisabled] = useState(false)
  const [source, setSource] = useState<SearchSource<FixtureItem>>(sourceA)
  const [item, setItem] = useState<FixtureItem | null>(null)
  const [openEvents, setOpenEvents] = useState(0)

  return (
    <main>
      <button type="button" data-testid="outside">Outside</button>
      <button type="button" data-testid="toggle-multi-disabled" onClick={() => setMultiDisabled((current) => !current)}>Toggle multi disabled</button>
      <button type="button" data-testid="toggle-typeahead-disabled" onClick={() => setTypeaheadDisabled((current) => !current)}>Toggle typeahead disabled</button>
      <button type="button" data-testid="swap-source" onClick={() => setSource(sourceB)}>Swap source</button>
      <button type="button" data-testid="resolve-old" onClick={() => resolveOld?.([oldItem])}>Resolve old</button>
      <output data-testid="open-events">{openEvents}</output>
      <MultiSelector
        label="Statuses"
        options={[{ value: "ready", label: "Ready" }, { type: "section", title: "Other statuses", options: [{ value: "review", label: "Review", disabled: true }, { value: "done", label: "Done" }] }]}
        value={selected}
        onChange={setSelected}
        htmlName="statuses"
        placeholder="Choose statuses"
        loadingText="Loading statuses"
        noOptionsText="No statuses"
        selectedText={(count) => `${count} statuses selected`}
        hasSearch
        searchLabel="Filter statuses"
        searchPlaceholder="Filter statuses"
        hasSelectAll
        selectAllLabel="Select all statuses"
        isDisabled={multiDisabled}
        data-testid="multi-selector"
      />
      <Typeahead
        label="Projects"
        searchSource={source}
        value={item}
        onChange={setItem}
        placeholder="Find projects"
        searchLabel="Find projects"
        listboxLabel="Project matches"
        loadingText="Loading projects"
        clearLabel="Clear project"
        emptySearchResultsText="No projects"
        errorText="Project search failed"
        debounceMs={0}
        isDisabled={typeaheadDisabled}
        onOpenChange={() => setOpenEvents((current) => current + 1)}
        data-testid="typeahead"
      />
    </main>
  )
}

const root = document.getElementById("root")
if (root === null) throw new Error("selector browser fixture root missing")
createRoot(root).render(<Fixture />)
