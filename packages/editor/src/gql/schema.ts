export type Maybe<T> = T | null;
export type InputMaybe<T> = Maybe<T>;
/** All built-in and custom scalars, mapped to their actual values */
export type Scalars = {
  ID: { input: string; output: string; }
  String: { input: string; output: string; }
  Boolean: { input: boolean; output: boolean; }
  Int: { input: number; output: number; }
  Float: { input: number; output: number; }
  NaiveDate: { input: unknown; output: unknown; }
};

export type AddAssetInput = {
  kind: AssetKind;
  parentId: Scalars['String']['input'];
  uri: Scalars['String']['input'];
};

export type AddAssetPayload = {
  __typename?: 'AddAssetPayload';
  asset: Asset;
};

export type Asset = {
  __typename?: 'Asset';
  kind: AssetKind;
  uid: Scalars['String']['output'];
  uri: Scalars['String']['output'];
};

export enum AssetKind {
  Unknown = 'UNKNOWN',
  Video = 'VIDEO'
}

export type Captions = {
  __typename?: 'Captions';
  spanAtTime: Maybe<Statement>;
};


export type CaptionsSpanAtTimeArgs = {
  timestamp: Scalars['Float']['input'];
};

export type CreateStatementInput = {
  endTime: Scalars['Float']['input'];
  startTime: Scalars['Float']['input'];
  text: Scalars['String']['input'];
};

export type CreateStatementPayload = {
  __typename?: 'CreateStatementPayload';
  statement: Statement;
};

export type DestroyStatementPayload = {
  __typename?: 'DestroyStatementPayload';
  ok: Scalars['Boolean']['output'];
  statement: StatementNode;
};

export type EditStatementInput = {
  endTime: Scalars['Float']['input'];
  startTime: Scalars['Float']['input'];
  text: Scalars['String']['input'];
  uid: Scalars['String']['input'];
};

export type EditStatementPayload = {
  __typename?: 'EditStatementPayload';
  newHash: Scalars['String']['output'];
  oldHash: Scalars['String']['output'];
  statement: Statement;
  wroteEmbedding: Scalars['Boolean']['output'];
};

export type Interview = {
  __typename?: 'Interview';
  date: Scalars['NaiveDate']['output'];
  interviewee: Person;
  number: Scalars['Int']['output'];
  transcript: Transcript;
  uid: Scalars['String']['output'];
  videos: Array<Video>;
};

/** Optional media assets to attach to the interview. */
export type InterviewAssetsInput = {
  jsonCaptionText: InputMaybe<Scalars['String']['input']>;
  jsonCaptionUrl: InputMaybe<Scalars['String']['input']>;
  videoUrl: InputMaybe<Scalars['String']['input']>;
  vttText: InputMaybe<Scalars['String']['input']>;
  vttUrl: InputMaybe<Scalars['String']['input']>;
};

export type MutationRoot = {
  __typename?: 'MutationRoot';
  addAsset: AddAssetPayload;
  createStatement: CreateStatementPayload;
  destroyStatement: DestroyStatementPayload;
  editStatement: EditStatementPayload;
  /**
   * Seeds a complete interview: creates Interview, Person, Transcript, and
   * Statement nodes plus all relationships. Segments are grouped into
   * statements by consecutive same-speaker runs.
   */
  seedInterview: SeedInterviewPayload;
};


export type MutationRootAddAssetArgs = {
  input: AddAssetInput;
};


export type MutationRootCreateStatementArgs = {
  interviewUid: Scalars['String']['input'];
  statement: CreateStatementInput;
};


export type MutationRootDestroyStatementArgs = {
  uid: Scalars['String']['input'];
};


export type MutationRootEditStatementArgs = {
  input: EditStatementInput;
};


export type MutationRootSeedInterviewArgs = {
  input: SeedInterviewInput;
};

/**
 * Mirrors the (:Person) node.
 *
 * Derives `Deserialize` so neo4rs can deserialize a Bolt Node directly into
 * this struct via `node.to::<Person>()`. The field names (`uid`, `name`)
 * match the Neo4j property names exactly, so no `#[serde(rename)]` is needed.
 *
 * This is the idiomatic neo4rs pattern: return whole nodes from Cypher
 * (`RETURN p`) rather than destructuring properties into arbitrary column
 * aliases (`RETURN p.uid AS person_uid, p.name AS person_name`).
 */
export type Person = {
  __typename?: 'Person';
  name: Scalars['String']['output'];
  uid: Scalars['String']['output'];
};

export type Query = {
  __typename?: 'Query';
  captions: Captions;
  /** Returns "ok". Useful for readiness and liveness probes. */
  health: Scalars['String']['output'];
  interview: Interview;
  interviews: Array<Interview>;
  search: Search;
};


export type QueryCaptionsArgs = {
  interviewNumber: Scalars['Int']['input'];
};


export type QueryInterviewArgs = {
  number: InputMaybe<Scalars['Int']['input']>;
  uid: InputMaybe<Scalars['String']['input']>;
};

export type Search = {
  __typename?: 'Search';
  interviews: Array<SearchHit>;
  statementText: Array<SearchHit>;
};


export type SearchInterviewsArgs = {
  limit: InputMaybe<Scalars['Int']['input']>;
  query: Scalars['String']['input'];
};


export type SearchStatementTextArgs = {
  fragment: Scalars['String']['input'];
  interviewUid: InputMaybe<Scalars['String']['input']>;
};

/** A single hit from a vector similarity search over Statement nodes. */
export type SearchHit = {
  __typename?: 'SearchHit';
  /** The interview this statement belongs to. */
  interview: Interview;
  /** The matching statement, with speaker and timing. */
  statement: Statement;
};

/** Top-level input for the `seedInterview` mutation. */
export type SeedInterviewInput = {
  /** Optional asset URLs associated with the interview. */
  assets: InputMaybe<InterviewAssetsInput>;
  /** ISO 8601 date string, e.g. "2003-05-05". */
  date: Scalars['String']['input'];
  /** Display name for the interviewee (e.g. "Lei Chou"). */
  interviewee: Scalars['String']['input'];
  /** Interview number in the AUOHP archive (e.g. 25, 64, 82). */
  number: Scalars['Int']['input'];
  /**
   * Raw transcript segments from the transcription pipeline.
   * Either `segments_json` or `segments` must be specified.
   */
  segments: InputMaybe<Array<TranscriptSegmentInput>>;
  /**
   * JSON string of transcript segments.
   * Either `segments_json` or `segments` must be specified.
   */
  segmentsJson: InputMaybe<Scalars['String']['input']>;
  /**
   * Maps diarization labels to person names and roles.
   * Optional --- omit when speaker labels are not yet assigned.
   */
  speakers: InputMaybe<Array<SpeakerMappingInput>>;
};

/** Returned by `seedInterview` to confirm what was created. */
export type SeedInterviewPayload = {
  __typename?: 'SeedInterviewPayload';
  /**
   * Always `true`. Embeddings are written by a background task after the
   * mutation returns; vector search results for this interview will not be
   * available until that task completes.
   */
  embeddingsQueued: Scalars['Boolean']['output'];
  interview: Interview;
  speakerCount: Scalars['Int']['output'];
  statementCount: Scalars['Int']['output'];
  transcriptUid: Scalars['String']['output'];
};

/** Maps a diarization speaker label to a person and their role. */
export type SpeakerMappingInput = {
  /** Diarization label from the transcription pipeline (e.g. "SPEAKER_00"). */
  label: Scalars['String']['input'];
  /** The person's display name (e.g. "Jim Hubbard"). */
  name: Scalars['String']['input'];
  /** The role this person plays in this interview. */
  role: SpeakerRole;
};

/** Whether a speaker is an interviewer or the interviewee. */
export enum SpeakerRole {
  Interviewee = 'INTERVIEWEE',
  Interviewer = 'INTERVIEWER'
}

/**
 * Mirrors the (:Statement) node, with timing from the `:CONTAINS` edge and
 * speaker attribution from `:SAYS`.
 */
export type Statement = {
  __typename?: 'Statement';
  endTime: Maybe<Scalars['Float']['output']>;
  /** The person who said this (via `:SAYS`). */
  person: Maybe<Person>;
  /**
   * Seconds from start of recording. Null for non-media statements
   * (e.g. broadsheet text).
   */
  startTime: Maybe<Scalars['Float']['output']>;
  text: Scalars['String']['output'];
  uid: Scalars['String']['output'];
  /**
   * Per-word timing data as a JSON string, e.g.
   * `[{"word":"the","start":1.23,"end":1.45}, ...]`.
   * Null if the transcription pipeline did not produce word-level timing.
   */
  words: Maybe<Scalars['String']['output']>;
};

/**
 * The node-native properties of a (:Statement) node.
 *
 * Separated from `Statement` because `Statement` mixes node properties with
 * data from relationships (`:CONTAINS` timing, `:SAYS` speaker), making a
 * full `Deserialize` derive impossible. This struct covers only what lives on
 * the node itself. `#[serde(default)]` on `words` means serde substitutes
 * `None` for absent keys rather than erroring---older transcripts were seeded
 * without word-level timing, so the property may not exist on the node at all.
 */
export type StatementNode = {
  __typename?: 'StatementNode';
  text: Scalars['String']['output'];
  uid: Scalars['String']['output'];
  words: Maybe<Scalars['String']['output']>;
};

export type Transcript = {
  __typename?: 'Transcript';
  /** Statements in transcript order (via the `:NEXT` linked list). */
  statements: Array<Statement>;
  uid: Scalars['String']['output'];
};

/** A single transcript segment from the transcription pipeline. */
export type TranscriptSegmentInput = {
  /** End time in seconds. */
  endTime: Scalars['Float']['input'];
  /** Speaker label (e.g. "SPEAKER_00"). Optional --- omit when not yet assigned. */
  speaker: InputMaybe<Scalars['String']['input']>;
  /** Start time in seconds from the beginning of the recording. */
  startTime: Scalars['Float']['input'];
  /** Transcribed text for this segment. */
  text: Scalars['String']['input'];
  /** Per-word timing data. Optional---some segments may lack word alignment. */
  words: InputMaybe<Array<WordTimingInput>>;
};

export type Video = {
  __typename?: 'Video';
  uri: Scalars['String']['output'];
};

/** Word-level timing from the transcription pipeline. */
export type WordTimingInput = {
  end: Scalars['Float']['input'];
  /** Confidence score (0.0–1.0). Optional. */
  p: InputMaybe<Scalars['Float']['input']>;
  start: Scalars['Float']['input'];
  word: Scalars['String']['input'];
};
