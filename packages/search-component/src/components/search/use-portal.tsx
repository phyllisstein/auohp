import {useRef, useEffect, useId} from 'react'

export function usePortal() {
  const portal = useRef<HTMLDivElement>(null)
  const portalID = useId()

  useEffect(() => {
    const currentPortal = portal.current

    if (typeof window === 'undefined' || currentPortal) {
      return
    }

    const portalRoot = document.createElement('div')
    portalRoot.id = portalID
    document.body.appendChild(portalRoot)

    portal.current = portalRoot
  }, [])

  return portal.current
}
