/** Internal type. DO NOT USE DIRECTLY. */
type Exact<T extends { [key: string]: unknown }> = { [K in keyof T]: T[K] };
/** Internal type. DO NOT USE DIRECTLY. */
export type Incremental<T> = T | { [P in keyof T]?: P extends " $fragmentName" | "__typename" ? T[P] : never };
import type * as Types from "../../../__generated__/schema.gql";

export type HeaderQuery_interview_interviewee = { name: string };

export type HeaderQuery_interview = { interviewee: HeaderQuery_interview_interviewee };

export type HeaderQuery = { interview: HeaderQuery_interview };


export type HeaderQueryVariables = Exact<{
    interviewNumber: number;
}>;
