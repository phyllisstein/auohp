CALL db.index.fulltext.queryNodes($index, $query) YIELD node, score
MATCH (node)<-[speakerSays:SAYS]-(speaker:Speaker)<-[:INTERVIEWED_AS]-(person:Person)
MATCH (speaker)<-[:INTERVIEWED_WITH]-(interview)-[:HAS_VIDEO]->(video)
RETURN speakerSays.startTime AS startTime,
    speakerSays.endTime AS endTime,
    speakerSays.duration AS duration,
    person.name AS speakerName,
    node.text AS statement,
    node.uid AS statementUID,
    interview.number AS interviewNumber,
    video.url AS videoURL,
    score
ORDER BY score DESC
