/** Internal type. DO NOT USE DIRECTLY. */
type Exact<T extends { [key: string]: unknown }> = { [K in keyof T]: T[K] };
/** Internal type. DO NOT USE DIRECTLY. */
export type Incremental<T> = T | { [P in keyof T]?: P extends " $fragmentName" | "__typename" ? T[P] : never };
import type * as Types from "./schema.gql";

export type CreateStatementInput = {
    endTime: number;
    startTime: number;
    text: string;
};

export type EditStatementMutation_editStatement_statement = { uid: string; text: string; startTime: number | null; endTime: number | null };

export type EditStatementMutation_editStatement = { oldHash: string; newHash: string; wroteEmbedding: boolean; statement: EditStatementMutation_editStatement_statement };

export type EditStatementMutation = { editStatement: EditStatementMutation_editStatement };


export type EditStatementMutationVariables = Exact<{
    uid: string;
    text: string;
    startTime: number;
    endTime: number;
}>;

export type CreateStatementMutation_createStatement_statement = { uid: string; text: string; startTime: number | null; endTime: number | null };

export type CreateStatementMutation_createStatement = { statement: CreateStatementMutation_createStatement_statement };

export type CreateStatementMutation = { createStatement: CreateStatementMutation_createStatement };


export type CreateStatementMutationVariables = Exact<{
    statement: Types.CreateStatementInput;
    interviewUid: string;
}>;

export type DestroyStatementMutation_destroyStatement_statement = { uid: string };

export type DestroyStatementMutation_destroyStatement = { ok: boolean; statement: DestroyStatementMutation_destroyStatement_statement };

export type DestroyStatementMutation = { destroyStatement: DestroyStatementMutation_destroyStatement };


export type DestroyStatementMutationVariables = Exact<{
    uid: string;
}>;

export type TranscriptQuery_interview_interviewee = { uid: string; name: string };

export type TranscriptQuery_interview_transcript_statements = { uid: string; startTime: number | null; endTime: number | null; text: string };

export type TranscriptQuery_interview_transcript = { uid: string; statements: Array<TranscriptQuery_interview_transcript_statements> };

export type TranscriptQuery_interview_videos = { uri: string };

export type TranscriptQuery_interview = { uid: string; number: number; interviewee: TranscriptQuery_interview_interviewee; transcript: TranscriptQuery_interview_transcript; videos: Array<TranscriptQuery_interview_videos> };

export type TranscriptQuery = { health: string; interview: TranscriptQuery_interview };


export type TranscriptQueryVariables = Exact<{
    interviewNumber: number;
}>;

export type SearchStatementsQuery_search_statementText_statement = { uid: string; text: string };

export type SearchStatementsQuery_search_statementText = { statement: SearchStatementsQuery_search_statementText_statement };

export type SearchStatementsQuery_search = { statementText: Array<SearchStatementsQuery_search_statementText> };

export type SearchStatementsQuery = { search: SearchStatementsQuery_search };


export type SearchStatementsQueryVariables = Exact<{
    fragment: string;
}>;
