// Leanest path: no graphql-request, no graphql parser. Just fetch.
const GRAPHQL_URI = "https://api.auohp.example/graphql";

const SEARCH_QUERY = `query SearchStatements($search: String!) {
  search { interviews(query: $search) {
    statement { uid startTime text person { name } }
    interview { number }
  } }
}`;

export async function runSearch(search) {
  const res = await fetch(GRAPHQL_URI, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ query: SEARCH_QUERY, variables: { search } }),
  });
  const json = await res.json();
  if (json.errors) throw new Error(json.errors[0]?.message ?? "GraphQL error");
  return json.data;
}

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
