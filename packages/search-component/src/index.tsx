import { createRoot } from "react-dom/client";
import { StrictMode } from "react";
import { ApolloClient, HttpLink, InMemoryCache } from "@apollo/client";
import { ApolloProvider } from "@apollo/client/react";

const { VITE_AUOHP_API_URI: API_URI = "http://localhost:4000/graphql" } = import.meta.env;

const client = new ApolloClient({
    link: new HttpLink({ uri: API_URI }),
    cache: new InMemoryCache(),
});

// FIXME: Two roots (player, search), one tree (with portal)
import { Player } from "~/components/player";
import { Search } from "~/components/search";

export function renderSearch (element: HTMLElement) {
    const root = createRoot(element);
    root.render(
        <StrictMode>
            <ApolloProvider client={ client }>
                <Search />
            </ApolloProvider>
        </StrictMode>,
    );
}

export function renderPlayer (interviewNumber: number, element: HTMLElement) {
    const root = createRoot(element);
    root.render(
        <StrictMode>
            <ApolloProvider client={ client }>
                <Player interviewNumber={ interviewNumber } />
            </ApolloProvider>
        </StrictMode>,
    );
}
