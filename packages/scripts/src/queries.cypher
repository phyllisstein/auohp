// Fetch all of an interview subject's statements made during a single
// interview.
MATCH (interview:Interview {number: $interviewNumber}) -[:INTERVIEWED_WITH]-> (speaker:Interviewee)
MATCH (subject) -[:INTERVIEWED_AS]-> (speaker)
MATCH (speaker) -[saying:SAYS]-> (statement)
RETURN saying.startTime as startTime,
       saying.endTime as endTime,
       saying.duration as duration,
       statement.text as text

// Search full-text Lucene index of `(:Statement {text})`. In the `MATCH`
// clause, attaching the `:Interviewee` label neatly filters out questions and
// discussion by the interviewers.
CALL db.index.fulltext.queryNodes('transcript_search', 'action') YIELD node AS statement, score
MATCH (statement)<-[said:SAYS]-(speaker:Interviewee)<-[:INTERVIEWED_AS]-(person)
MATCH (speaker)<-[:INTERVIEWED_WITH]-(interview)-[:HAS_VIDEO]->(video)
RETURN said.startTime AS timestamp,
    person.name AS speaker,
    statement.text AS statement,
    video.url AS videoURL,
    interview.number AS interviewNumber,
    score
ORDER BY score DESC
