import { createRoot } from 'react-dom/client'

import { Search } from './components/search'

export function renderSearch (element: HTMLElement) {
    const root = createRoot(element)
    root.render(<Search />)
}
