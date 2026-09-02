import { createFileRoute } from "@tanstack/react-router";
import { style } from "@react-spectrum/s2/style" with { type: "macro" };
import { searchQuery } from "../-search-signal";

export const Route = createFileRoute("/search/results")({
    component: ResultsPage,
});

function ResultsPage () {
    // Bare `.value` reads --- the signals-react-transform Babel pass wraps this
    // component so each read subscribes it and a write anywhere re-renders it.
    const { query, results, loading, error } = searchQuery;

    if (loading.value) {
        return <p>Searching for &ldquo;{ query.value }&rdquo;&hellip;</p>;
    }

    if (error.value) {
        return <p role="alert">Search failed: { error.value.message }</p>;
    }

    // `null` means no search has run (e.g. a direct visit to this route via an
    // unmasked link); an empty array means a search ran and matched nothing.
    if (results.value == null) {
        return <p>Enter a query to search the transcripts.</p>;
    }

    const hits = results.value.search.statementText;

    if (hits.length === 0) {
        return <p>No results found for &ldquo;{ query.value }&rdquo;.</p>;
    }

    return (
        <div>
            { hits.map(hit => (
                <div
                    key={ hit.statement.uid }
                    className={ style({ marginBottom: "text-to-control", backgroundColor: "layer-1" }) }>
                    <p>
                        <strong>
                            Interview #{ hit.interview.number } &mdash; { hit.interview.interviewee.name }
                        </strong>
                    </p>
                    <p>{ hit.statement.text }</p>
                    <p>
                        <em>
                            Start time: { hit.statement.startTime } | End time: { hit.statement.endTime }
                        </em>
                    </p>
                </div>
            )) }
        </div>
    );
}
