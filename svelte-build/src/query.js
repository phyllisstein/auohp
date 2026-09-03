import { GraphQLClient, gql } from "graphql-request";

const GRAPHQL_URI = "https://api.auohp.example/graphql";
const client = new GraphQLClient(GRAPHQL_URI);

export const SEARCH_QUERY = gql`
  query SearchStatements($search: String!) {
    search {
      interviews(query: $search) {
        statement { uid startTime text person { name } }
        interview { number }
      }
    }
  }
`;

export function runSearch(search) {
  return client.request(SEARCH_QUERY, { search });
}

// Parity with the React widget's queryString.stringifyUrl for the result click.
export function playerUrl(startTime, interviewNumber) {
  const q = new URLSearchParams({ timestamp: String(startTime), interview: String(interviewNumber) });
  return `/player?${q}`;
}

export function formatTimestamp(timestamp) {
  const total = Math.round(timestamp);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const pad = (n) => String(n).padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${m}:${pad(s)}`;
}
