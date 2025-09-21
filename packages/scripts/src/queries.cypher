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


// Fetch word-by-word timings for a single statement.
MATCH (interview:Interview)
WITH interview LIMIT 1
MATCH (interview) -[:INTERVIEWED_WITH]-> (speaker:Interviewee) -[:SAYS]-> (statement:Statement)
WITH statement LIMIT 1
MATCH (statement) -[:FROM_WORDS]-> (words:WordTimings)
RETURN statement.text as text,
      statement.startTime as startTime,
      statement.endTime as endTime,
      statement.duration as duration,
      words.text as words


// Another full-text search example, this time with a phrase.
CALL db.index.fulltext.queryNodes('transcript_search', '"ashes action"') YIELD node AS statement, score
MATCH (statement)<-[speakerSays:SAYS]-(transcript:Transcript)<-[:HAS_TRANSCRIPT]-(artefact)-[:HAS_VIDEO]->(video)
MATCH (speaker) <-[:INTERVIEWED_AS]- (person)
RETURN statement, person, speakerSays, artefact, video, transcript, speaker
ORDER BY score DESC

// Reconstruct a transcript
MATCH (interview:Interview {uid: "QIzCef06-xiHL5YPZxu6U"}) -[:HAS_TRANSCRIPT]-> () -[saying:SAYS]-> (statement)
MATCH (statement) <-[:SAYS]- (speaker:Speaker) <-[:INTERVIEWED_AS]- (person)
RETURN statement, saying, speaker, person
ORDER BY saying.startTime ASC
