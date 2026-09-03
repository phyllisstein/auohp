import { createRoot } from "react-dom/client";
import { StrictMode } from "react";
import { ApolloClient, HttpLink, InMemoryCache } from "@apollo/client";
import { ApolloProvider } from "@apollo/client/react";
import { Search } from "~/components/search";

const GRAPHQL_URI = "https://api.auohp.example/graphql";

const client = new ApolloClient({
    link: new HttpLink({ uri: GRAPHQL_URI }),
    cache: new InMemoryCache(),
});

function mount(el: HTMLElement) {
    createRoot(el).render(
        <StrictMode>
            <ApolloProvider client={client}>
                <Search />
            </ApolloProvider>
        </StrictMode>,
    );
}

const el = document.getElementById("auohp-search");
if (el) mount(el);
