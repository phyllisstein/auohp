/** Internal type. DO NOT USE DIRECTLY. */
type Exact<T extends { [key: string]: unknown }> = { [K in keyof T]: T[K] };
/** Internal type. DO NOT USE DIRECTLY. */
export type Incremental<T> = T | { [P in keyof T]?: P extends " $fragmentName" | "__typename" ? T[P] : never };
import type * as Types from "../../__generated__/schema.gql";

export type ListInterviewsQuery_interviews_interviewee = { name: string };

export type ListInterviewsQuery_interviews = { number: number; interviewee: ListInterviewsQuery_interviews_interviewee };

export type ListInterviewsQuery = { interviews: Array<ListInterviewsQuery_interviews> };


export type ListInterviewsQueryVariables = Exact<{ [key: string]: never }>;
