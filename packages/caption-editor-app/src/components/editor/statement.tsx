import {Segment} from './segment'

export function Statement({attributes, element, children}) {
  return (
    <Segment {...attributes} data-testid='statement'>
      <div contentEditable={false} style={{gridRow: '1 / -1', gridColumn: '1'}}>
        <span style={{fontWeight: 'bold'}}>{element.speaker}</span>
      </div>
      <div contentEditable={false} style={{gridRow: '1', gridColumn: '2'}}>
        <span>{element.startTimestamp}</span>
                &nbsp;&ndash;&nbsp;
        <span>{element.endTimestamp}</span>
      </div>
      <div style={{gridRow: '2 / -1', gridColumn: '2 / -1'}}>
        {
          children
        }
      </div>
    </Segment>
  )
}
