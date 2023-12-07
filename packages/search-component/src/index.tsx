import { Search } from './Search'
import { createRoot } from 'react-dom/client'

export function renderSearch(element: HTMLElement) {
  const root = createRoot(element)
  root.render(<Search />)
}
