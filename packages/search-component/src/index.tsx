import { createRoot } from 'react-dom/client'

import { Player } from 'components/player'
import { Search } from 'components/search'

export function renderSearch(element: HTMLElement) {
    const root = createRoot(element)
    root.render(
        <Search />,
    )
}

export function renderPlayer(interviewNumber: number, element: HTMLElement) {
    const root = createRoot(element)
    root.render(
        <Player interviewNumber={ interviewNumber } />,
    )
}
