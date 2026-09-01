/** Internal type. DO NOT USE DIRECTLY. */
type Exact<T extends { [key: string]: unknown }> = { [K in keyof T]: T[K] };
/** Internal type. DO NOT USE DIRECTLY. */
export type Incremental<T> = T | { [P in keyof T]?: P extends " $fragmentName" | "__typename" ? T[P] : never };
import type * as Types from "../../../__generated__/schema.gql";

export type SearchStatementsQuery_search_interviews_statement_person = { name: string };

export type SearchStatementsQuery_search_interviews_statement = { uid: string; startTime: number | null; text: string; person: SearchStatementsQuery_search_interviews_statement_person | null };

export type SearchStatementsQuery_search_interviews_interview = { number: number };

export type SearchStatementsQuery_search_interviews = { statement: SearchStatementsQuery_search_interviews_statement; interview: SearchStatementsQuery_search_interviews_interview };

export type SearchStatementsQuery_search = { interviews: Array<SearchStatementsQuery_search_interviews> };

export type SearchStatementsQuery = { search: SearchStatementsQuery_search };


export type SearchStatementsQueryVariables = Exact<{
    search: string;
}>;
