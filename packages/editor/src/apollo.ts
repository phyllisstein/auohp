import { ApolloClient, HttpLink, InMemoryCache } from "@apollo/client";


const client = new ApolloClient({
    link: new HttpLink({
        uri: import.meta.env.VITE_AUOHP_API_URI.replace(/\/+$/, "") + "/graphql",
    }),
    cache: new InMemoryCache(),
});


export default client;
