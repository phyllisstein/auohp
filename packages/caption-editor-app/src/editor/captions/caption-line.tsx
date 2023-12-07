import styled from 'styled-components'

const CaptionWrapper = styled.div`
    padding: 12px;

    background-color: var(--spectrum-background-base-color);

    min-inline-size: min-content;
`

export function CaptionLine ({ attributes, children, element }) {
    return (
        <div { ...attributes }>
            <CaptionWrapper className='spectrum-InLineAlert'>
                <div className='spectrum-InLineAlert-content' style={{ margin: 0 }}>
                    { children }
                </div>
            </CaptionWrapper>
        </div>
    )
}
