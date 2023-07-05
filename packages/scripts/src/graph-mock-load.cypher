CALL apoc.load.json('file:///graph-mock.json')
YIELD value
UNWIND value.interviews AS interview
MERGE (i:Interview {uuid: interview.uuid})
    ON CREATE SET i.date = datetime(interview.date)
WITH *
UNWIND value.subjects AS subject
MERGE (s:Subject {uuid: subject.uuid})
    ON CREATE SET s.name = subject.name
WITH *
UNWIND value.interviewers AS interviewer
MERGE (int:Interviewer {uuid: interviewer.uuid})
    ON CREATE SET int.name = interviewer.name
MERGE (s) -[:GAVE_INTERVIEW]-> (i) <-[:CONDUCTED_INTERVIEW]-(int)
RETURN s, i, int

///////////////////////////////////////////////

CALL apoc.load.json('file:///graph-mock.json')
YIELD value
UNWIND value.transcriptionJobs AS job
MERGE (j:TranscriptionJob {uuid: job.uuid})
    ON CREATE SET j += properties(job)
WITH *
MATCH (int:Interview)
MERGE (int)-[:FROM_JOB]-> (j)
RETURN int, j

///////////////////////////////////////////////

CALL apoc.load.json('file:///graph-mock.json')
YIELD value
UNWIND value.captionTracks as track
MERGE (trk:Track {uuid: track.uuid})
    ON CREATE SET trk += properties(track)
WITH *
MATCH (int:Interview)
MERGE (int) -[:HAS_CAPTIONS]-> (trk)
RETURN trk, int

///////////////////////////////////////////////

CALL apoc.load.json('file:///graph-mock.json')
YIELD value
UNWIND value.cues as cue
MERGE (c:Cue {uuid: cue.uuid})
    ON CREATE SET c += properties(cue)

///////////////////////////////////////////////

CALL apoc.load.json('file:///graph-mock.json')
YIELD value
UNWIND value.cues AS cue
MERGE (c:Cue {uuid: cue.uuid})
    ON CREATE SET c += properties(cue)
WITH *
MATCH (ct:Track)
MERGE (ct) -[:FROM_CONTENT]-> (c)
RETURN ct, c

///////////////////////////////////////////////

CALL apoc.load.json('file:///graph-mock.json')
YIELD value
UNWIND value.cueWords['cue-1'] AS v
MATCH (q:Cue {identifier: 'cue-1'})
MERGE (q) -[p:FROM_CONTENT {startTime: v.startTime, endTime: v.endTime}]-> (w:Word {content: v.content})
    ON CREATE SET w.content = v.content,
      w.type = v.type,
      p.startTime = v.startTime,
      p.endTime = v.endTime
RETURN q, p, w

///////////////////////////////////////////////

CALL apoc.load.json('file:///graph-mock.json')
YIELD value
UNWIND value.cueWords['cue-2'] AS v
MATCH (q:Cue {identifier: 'cue-2'})
MERGE (q) -[p:FROM_CONTENT {startTime: v.startTime, endTime: v.endTime}]-> (w:Word {content: v.content})
    ON CREATE SET w.content = v.content,
      w.type = v.type,
      p.startTime = v.startTime,
      p.endTime = v.endTime
RETURN q, p, w

///////////////////////////////////////////////

CREATE FULLTEXT INDEX transcriptSearch IF NOT EXISTS
FOR (n:Cue|Word) ON EACH [n.content]
OPTIONS {
    indexConfig: {
        `fulltext.analyzer`: 'english',
        `fulltext.eventually_consistent`: true
    }
}
