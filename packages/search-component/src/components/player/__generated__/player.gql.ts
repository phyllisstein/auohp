/** Internal type. DO NOT USE DIRECTLY. */
type Exact<T extends { [key: string]: unknown }> = { [K in keyof T]: T[K] };
/** Internal type. DO NOT USE DIRECTLY. */
export type Incremental<T> = T | { [P in keyof T]?: P extends " $fragmentName" | "__typename" ? T[P] : never };
import type * as Types from "../../../__generated__/schema.gql";

export type PlayerInterviewQuery_interview_videos = { uri: string };

export type PlayerInterviewQuery_interview = { videos: Array<PlayerInterviewQuery_interview_videos> };

export type PlayerInterviewQuery = { interview: PlayerInterviewQuery_interview };


export type PlayerInterviewQueryVariables = Exact<{
    interviewNumber: number;
}>;
