/** Internal type. DO NOT USE DIRECTLY. */
type Exact<T extends { [key: string]: unknown }> = { [K in keyof T]: T[K] };
/** Internal type. DO NOT USE DIRECTLY. */
export type Incremental<T> = T | { [P in keyof T]?: P extends " $fragmentName" | "__typename" ? T[P] : never };
import type * as Types from "../../../../__generated__/schema.gql";

export type SearchStatementsQuery_search_statementText_statement = { uid: string; text: string };

export type SearchStatementsQuery_search_statementText = { statement: SearchStatementsQuery_search_statementText_statement };

export type SearchStatementsQuery_search = { statementText: Array<SearchStatementsQuery_search_statementText> };

export type SearchStatementsQuery = { search: SearchStatementsQuery_search };


export type SearchStatementsQueryVariables = Exact<{
    fragment: string;
    interviewUid: string | null | undefined;
}>;
