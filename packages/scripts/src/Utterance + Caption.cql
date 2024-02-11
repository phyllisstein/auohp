//============================================================================//
//============================= Transcript Graph =============================//
//============================================================================//
// Example graph model for interview transcripts. Must facilitate:
//  - Keyword search localized to a specific timestamp
//  - Name search for a specific interviewee
//  - Additional metadata in the future: events, locations, tags...


//------------------------------ Mock Interview ------------------------------//
MERGE (interview:Interview {label: "35 - Larry Kramer"})
MERGE (interview)-[:HAS_TRANSCRIPTION_JOB]-> (:TranscriptionJob:AWS {id: 1, status: "COMPLETED", transcript: "https://s3.amazonaws.com/aws-transcribe-output/1.json", vtt: "https://s3.amazonaws.com/aws-transcribe-output/1.vtt"})
MERGE (larry:Person {name: "Larry Kramer"})
MERGE (jim:Person {name: "Jim Hubbard"})
MERGE (sarah:Person {name: "Sarah Schulman"})
MERGE (jimSpeaker:Speaker {id: 1, remoteID: "spk_0"})
MERGE (larrySpeaker:Speaker {id: 2, remoteID: "spk_2"})
MERGE (sarahSpeaker:Speaker {id: 3, remoteID: "spk_1"})
MERGE (larry) -[:INTERVIEWED_AS]-> (larrySpeaker)
MERGE (jim) -[:INTERVIEWED_AS]-> (jimSpeaker)
MERGE (sarah) -[:INTERVIEWED_AS]-> (sarahSpeaker)
MERGE (interview) -[:INTERVIEW_WITH]-> (jimSpeaker)
MERGE (interview) -[:INTERVIEW_WITH]-> (larrySpeaker)
MERGE (interview) -[:INTERVIEW_WITH]-> (sarahSpeaker)
MERGE (jimSpeaker)-[:SAYS {startTime: 438.51, endTime: 458.66}]->(st1:Statement {text: "you've been interviewed many times and you have a lot to say and we really want to do or ask you the questions that you probably have not repeated ad nauseam before and that maybe are more of an internal conversation from people who were inside act up together Um So that's so that we're not going to be asking you these generic"})
MERGE (jimSpeaker)-[:SAYS {startTime: 459.25, endTime: 468.77}]->(st2:Statement {text: "questions that things that anyone who is interested can find other places I just want to ask you a few background questions When did your family come to this country"})
MERGE (larrySpeaker)-[:SAYS {startTime: 471.16, endTime: 476.33}]->(st3:Statement {text: "Well my father was born in this country and his"})
MERGE (larrySpeaker)-[:SAYS {startTime: 477.33, endTime: 479.5}]->(st4:Statement {text: "mother was from Russia"})
MERGE (larrySpeaker)-[:SAYS {startTime: 481.08, endTime: 488.83}]->(st5:Statement {text: "and no one knows where his father was from And my mother came when she was four also from Russia"})


//------------------------------ Statement Index -----------------------------//
CREATE FULLTEXT INDEX transcript_search IF NOT EXISTS
FOR (n:Statement) ON EACH [n.text]
OPTIONS {
    indexConfig: {
        `fulltext.analyzer`: 'english',
        `fulltext.eventually_consistent`: true
    }
}


//----------------------------- Search Statements ----------------------------//
CALL db.index.fulltext.queryNodes('transcript_search', 'russia') YIELD node, score
MATCH (node)<-[sez:SAYS]-()<-[:INTERVIEWED_AS]-(whom)
RETURN sez.startTime AS startTime, sez.endTime AS endTime, whom.name AS speaker, node.text AS statement

// ╒═════════╤═══════╤══════════════╤════════════════════════╕
// │startTime│endTime│speaker       │statement               │
// ╞═════════╪═══════╪══════════════╪════════════════════════╡
// │477.33   │479.5  │"Larry Kramer"│"mother was from Russia"│
// ├─────────┼───────┼──────────────┼────────────────────────┤
// │481.08   │488.83 │"Larry Kramer"│"and no one knows where │
// │         │       │              │his father was from And │
// │         │       │              │my mother came when she │
// │         │       │              │was four also from Russi│
// │         │       │              │a"                      │
// └─────────┴───────┴──────────────┴────────────────────────┘


//-------------------------------- Name Index --------------------------------//
CREATE FULLTEXT INDEX nameSearch IF NOT EXISTS
FOR (n:Person) ON EACH [n.name]
OPTIONS {
    indexConfig: {
        `fulltext.eventually_consistent`: true
    }
}


//------------------------------- Search Names -------------------------------//

// The `otherSpeaker` bit doesn't wind up being all that useful, since each
// interview is one person speaking with Jim and Sarah.
CALL db.index.fulltext.queryNodes('nameSearch', 'Larry') YIELD node, score
MATCH (node) --{2} (interview:Interview)
WITH node, interview
MATCH (interview) -[:INTERVIEW_WITH]-> () <-[:INTERVIEWED_AS]- (speaker) WHERE speaker.name <> node.name
RETURN DISTINCT node.name AS name, interview.label AS interview, speaker.name AS otherSpeaker
