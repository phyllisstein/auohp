import { Segment } from "./segment";

export function Statement({ attributes, children, element }) {
    return (
        <Segment { ...attributes } data-testid="statement">
            <div contentEditable={ false } style={{ gridColumn: "1", gridRow: "1 / -1" }}>
                <span style={{ fontWeight: "bold" }}>{ element.speaker }</span>
            </div>
            <div contentEditable={ false } style={{ gridColumn: "2", gridRow: "1" }}>
                <span>{ element.startTimestamp }</span>
                &nbsp;&ndash;&nbsp;
                <span>{ element.endTimestamp }</span>
            </div>
            <div style={{ gridColumn: "2 / -1", gridRow: "2 / -1" }}>
                {
                    children
                }
            </div>
        </Segment>
    );
}
