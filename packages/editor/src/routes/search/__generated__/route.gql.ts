/** Internal type. DO NOT USE DIRECTLY. */
type Exact<T extends { [key: string]: unknown }> = { [K in keyof T]: T[K] };
/** Internal type. DO NOT USE DIRECTLY. */
export type Incremental<T> = T | { [P in keyof T]?: P extends " $fragmentName" | "__typename" ? T[P] : never };
import type * as Types from "../../../__generated__/schema.gql";

export type SearchAllStatementsQuery_search_statementText_statement = { uid: string; text: string; startTime: number | null; endTime: number | null };

export type SearchAllStatementsQuery_search_statementText_interview_interviewee = { uid: string; name: string };

export type SearchAllStatementsQuery_search_statementText_interview = { uid: string; number: number; interviewee: SearchAllStatementsQuery_search_statementText_interview_interviewee };

export type SearchAllStatementsQuery_search_statementText = { statement: SearchAllStatementsQuery_search_statementText_statement; interview: SearchAllStatementsQuery_search_statementText_interview };

export type SearchAllStatementsQuery_search = { statementText: Array<SearchAllStatementsQuery_search_statementText> };

export type SearchAllStatementsQuery = { search: SearchAllStatementsQuery_search };


export type SearchAllStatementsQueryVariables = Exact<{
    fragment: string;
}>;
