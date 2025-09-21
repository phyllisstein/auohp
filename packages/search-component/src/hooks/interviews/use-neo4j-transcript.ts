import { useEffect, useState } from "react";
import { useNeo4j } from "hooks/infrastructure";
import type { Neo4jResult } from "./types";


const {
    VITE_OPENAI_API_KEY: OPENAI_API_KEY,
} = import.meta.env;


export function useNeo4jTranscript(query: string): Neo4jResult[] {
    const driver = useNeo4j();
    const [searchResults, setSearchResults] = useState<Neo4jResult[]>([]);

    useEffect(() => {
        if (!OPENAI_API_KEY) {
            throw new Error("Must provide an OpenAI API key");
        }

        if (!driver || !query) {
            setSearchResults([]);
            return;
        }

        async function search() {
            const result = await driver.executeQuery(
                // language=Cypher
                `
          CALL genai.vector.encodeBatch([$query], 'OpenAI', {token: $token}) YIELD vector
          CALL db.index.vector.queryNodes('statement_embedding', 15, vector) YIELD node AS statement
          MATCH (statement)<-[meta:TRANSCRIBES]-(transcript) <-[:HAS_TRANSCRIPT]- (artefact)
          OPTIONAL MATCH (person) -[:INTERVIEWED_AS]-> (speaker) -[:SAYS]-> (statement)
          WHERE speaker:Interviewee
          OPTIONAL MATCH (artefact) -[:HAS_ASSET]-> (asset)
          RETURN statement, meta, person, speaker, asset, artefact
        `, { query, token: OPENAI_API_KEY });

            const searchResults = result.records.map(record => {
                return {
                    artefact: record.get("artefact"),
                    asset: record.get("asset"),
                    meta: record.get("meta"),
                    person: record.get("person"),
                    statement: record.get("statement"),
                };
            }).filter(
                result =>
                    !result.artefact.labels.includes("Interview")
                    || result.person !== null,
            );

            setSearchResults(searchResults);
        }

        void search();
    }, [query, driver]);

    return searchResults;
}
