// Adatped from https://neo4j.com/developer/genai-ecosystem/hybrid-search/


CYPHER 25
LET
    query = $query,
    queryVector = $queryVector,
    sourceK = 50,
    finalK = 15,
    rrfConstant = 60,
    sourceWeights = {
        fulltext: 1.0,
        vector: 1.0
    }

CALL (query, queryVector, sourceK, rrfConstant, sourceWeights) {
    CALL db.index.fulltext.queryNodes('statementText', query, {limit: sourceK})
    YIELD node AS statement, score
    ORDER BY score DESC, statement.uid ASC
    WITH collect(statement) AS statements, rrfConstant, sourceWeights
    LET weight = coalesce(sourceWeights.fulltext, 1.0)
    UNWIND CASE WHEN size(statements) = 0 THEN [] ELSE range(0, size(statements) - 1) END AS rankIndex
    RETURN
        statements[rankIndex] AS statement,
        weight / (rrfConstant + rankIndex + 1) AS contribution

    UNION ALL

    MATCH (statement:Statement)
        SEARCH statement IN (
            VECTOR INDEX statementEmbedding
            FOR queryVector
            LIMIT 50            // $sourceK
        ) SCORE AS score
    ORDER BY score DESC, statement.uid ASC
    WITH collect(statement) AS statements, rrfConstant, sourceWeights
    LET weight = coalesce(sourceWeights.vector, 1.0)
    UNWIND CASE WHEN size(statements) = 0 THEN [] ELSE range(0, size(statements) - 1) END AS rankIndex
    RETURN
        statements[rankIndex] AS statement,
        weight / (rrfConstant + rankIndex + 1) AS contribution
}
WITH statement, finalK, sum(contribution) AS wrrf
ORDER BY wrrf DESC, statement.uid ASC
WITH collect({statement: statement, wrrf: wrrf}) AS orderedRows, finalK
LET limitedRows = orderedRows[..finalK]
UNWIND limitedRows AS row
WITH row.statement AS statement, row.wrrf AS wrrf
MATCH  (interview:Interview)-[:HAS_TRANSCRIPT]->(transcript:Transcript)-[span:CONTAINS]->(statement)<-[:SAYS]-(person:Person)

return interview, transcript, person, statement, span, wrrf
