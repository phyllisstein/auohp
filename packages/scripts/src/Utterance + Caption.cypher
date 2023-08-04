MERGE (transcript:Transcript {id: 1})
MERGE (transcript)-[:RUNNING_JOB]-> (:TranscriptionJob:AWS {id: 1, status: "COMPLETED", transcript: "https://s3.amazonaws.com/aws-transcribe-output/1.json"})
MERGE (speaker1:Speaker {name: "Speaker 1", id: 1, providerLabel: 'spk_0'})
MERGE (speaker2:Speaker {name: "Speaker 2", id: 2, providerLabel: 'spk_1'})
MERGE (transcript)-[:HAS_SPEAKER]->(speaker1)
MERGE (transcript)-[:HAS_SPEAKER]->(speaker2)
MERGE (speaker1)-[:SAYS {startTime: 0, endTime: 12.12}]->(ut1:Utterance {text: "The alarm still oscillated, louder here, the rear wall dulling the roar of the Flatline as a construct.", id: 1}) -[:AS_CAPTION]-> (:Caption {text: "The alarm still oscillated, louder here", id: 1, startTime: 0, endTime: 6})
MERGE (ut1) -[:AS_CAPTION]-> (:Caption {text: "the rear wall dulling the roar of the Flatline as a construct", id: 2, startTime: 6, endTime: 12.12})
MERGE (speaker2)-[:SAYS {startTime: 12.4, endTime: 13}]->(ut2:Utterance {text: "A hardwired ROM cassette replicating a dead man’s skills, obsessions, kneejerk responses.", id: 2}) -[:AS_CAPTION {id: 4}]-> (:Caption {text: "A hardwired ROM cassette", id: 4, startTime: 12.4, endTime: 12.5})
MERGE (ut2) -[:AS_CAPTION]-> (:Caption {text: "replicating a dead man’s skills", id: 3})
MERGE (ut2) -[:AS_CAPTION]-> (:Caption {text: "obsessions", id: 5, startTIme: 12.5, endTime: 12.6})
MERGE (ut2) -[:AS_CAPTION]-> (:Caption {text: "kneejerk responses", id: 6, startTime: 12.6, endTime: 13})
MERGE (speaker1)-[:SAYS {startTime: 13.1, endTime: 14}]->(ut3:Utterance {text: "They were dropping, losing altitude in a canyon of rainbow foliage, a lurid communal mural that completely covered the hull of the blowers and the amplified breathing of the fighters. Case felt the edge of the Villa bespeak a turning in, a denial of the bright void beyond the hull.", id: 3}) -[:AS_CAPTION]-> (:Caption {text: "They were dropping", id: 7})
MERGE (ut3) -[:AS_CAPTION]-> (:Caption {text: "losing altitude in a canyon of rainbow foliage", id: 8, startTime: 13.1, endTime: 13.2})
MERGE (ut3) -[:AS_CAPTION]-> (:Caption {text: "a lurid communal mural that completely covered the hull of the blowers", id: 9, startTime: 13.2, endTime: 13.3})
MERGE (ut3) -[:AS_CAPTION]-> (:Caption {text: "and the amplified breathing of the fighters", id: 10, startTime: 13.3, endTime: 13.4})

MATCH p = ()--()
RETURN p

MATCH p = ()--()
DETACH DELETE p
