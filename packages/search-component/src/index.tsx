import { createRoot } from "react-dom/client";
import { StrictMode } from "react";
import { ApolloClient, HttpLink, InMemoryCache } from "@apollo/client";
import { ApolloProvider } from "@apollo/client/react";

const { VITE_GRAPHQL_URI: GRAPHQL_URI = "http://localhost:4000/graphql" } = import.meta.env;

const client = new ApolloClient({
    link: new HttpLink({ uri: GRAPHQL_URI }),
    cache: new InMemoryCache(),
});

import { Player } from "components/player";
import { Search } from "components/search";

export function renderSearch(element: HTMLElement) {
    const root = createRoot(element);
    root.render(
        <StrictMode>
            <ApolloProvider client={ client }>
                <Search />
            </ApolloProvider>
        </StrictMode>,
    );
}

export function renderPlayer(url: string, element: HTMLElement) {
    const root = createRoot(element);
    root.render(
        <StrictMode>
            <Player url={ url } />
        </StrictMode>,
    );
}
