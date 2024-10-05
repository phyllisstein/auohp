import {createRoot} from 'react-dom/client'
import {StrictMode} from 'react'

import {Player} from 'components/player'
import {Search} from 'components/search'

export function renderSearch(element: HTMLElement) {
  const root = createRoot(element)
  root.render(
    <StrictMode>
      <Search />
    </StrictMode>,
  )
}

export function renderPlayer(url: string, element: HTMLElement) {
  const root = createRoot(element)
  root.render(
    <StrictMode>
      <Player url={url} />
    </StrictMode>,
  )
}
