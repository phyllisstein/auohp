#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12,<3.14"
# dependencies = [
#   "click",
#   "pyyaml",
# ]
# ///

"""Seed the GraphQL API with interview data."""

import click
import os
from pathlib import Path
import yaml
from difflib import SequenceMatcher
from dataclasses import dataclass
import datetime as dt
import json
import urllib.request


SEED_QUERY = """
mutation SeedInterview($input: SeedInterviewInput!) {
    seedInterview(input: $input) {
        statementCount
        transcriptUid
        embeddingsQueued
        interview {
            date
            interviewee {
                name
                uid
            }
            number
            uid
        }
    }
}
"""


@dataclass
class Interview:
    interview_number: int
    interviewee: str
    interviewer: str
    interview_date: dt.date
    copyright_year: str
    filename: str


@dataclass
class VimeoVideo:
    id: str
    name: str
    stem: str
    ext: str


def similar(a: str, b: str) -> float:
    return SequenceMatcher(None, a, b).ratio()


def most_similar(target: str, options: list[str]) -> str:
    return max(options, key=lambda option: similar(target, option))


def find_interview(interview_number: int, interview_data) -> Interview | None:
    for interview in interview_data.get("interviews", []):
        if interview.get("interview_number") == interview_number:
            return Interview(**interview)
    return None


def match_vimeo_video(interview: Interview, vimeo_data) -> VimeoVideo | None:
    videos = vimeo_data.get("videos", [])

    for video in videos:
        if video.get("name") == most_similar(
            f"{interview.interview_number} {interview.interviewee}",
            [v.get("name") for v in videos],
        ):
            return VimeoVideo(
                id=video.get("id"),
                name=video.get("name"),
                stem=video.get("stem"),
                ext=video.get("ext"),
            )
    return None


def get_seed_query(interview: Interview, vimeo_video: VimeoVideo | None, transcription_segments):
    input_data = {
        "input": {
            "number": interview.interview_number,
            "interviewee": interview.interviewee,
            "date": interview.interview_date.isoformat(),
            "segments": json.dumps(transcription_segments),
        },
    }

    if vimeo_video:
        input_data["input"]["assets"] = {"vimeoId": vimeo_video.id}

    request_data = {
        "query": SEED_QUERY,
        "variables": input_data,
    }
    return request_data


def execute_seed_query(request_data, graphql_endpoint: str):
    req = urllib.request.Request(
        graphql_endpoint,
        data=json.dumps(request_data).encode("utf-8"),
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req) as response:
        return json.load(response)


@click.command()
@click.argument("interview_number", type=int)
@click.option(
    "--vimeo-manifest",
    type=Path,
    help="Path to the Vimeo manifest file",
    default="docs/vimeo-manifest.yaml",
)
@click.option(
    "--graphql-endpoint",
    type=str,
    help="GraphQL endpoint URL",
    default=lambda: os.environ.get("GRAPHQL_ENDPOINT", "http://localhost:6060/graphql"),
)
@click.option(
    "--interview-manifest",
    type=Path,
    help="Path to the interview manifest file",
    default="docs/interview-manifest.yaml",
)
@click.option(
    "--transcript", type=Path, help="Path to the JSON transcript", required=True
)
def main(
    interview_number: int,
    vimeo_manifest: Path,
    graphql_endpoint: str,
    interview_manifest: Path,
    transcript: Path,
):
    print(f"Seeding interview number: {interview_number}")

    if not interview_manifest.exists():
        print(f"Interview manifest file not found: {interview_manifest}")
        return

    with open(interview_manifest, "r") as f:
        interview_data = yaml.safe_load(f)
    interview = find_interview(interview_number, interview_data)
    if not interview:
        print(
            f"Interview number {interview_number} not found in the interview manifest."
        )
        return


    if not vimeo_manifest.exists():
        print(f"Vimeo manifest file not found: {vimeo_manifest}")
        return

    with open(vimeo_manifest, "r") as f:
        vimeo_data = yaml.safe_load(f)
    vimeo_video = match_vimeo_video(interview, vimeo_data)
    if not vimeo_video:
        print(f"No matching Vimeo video found for interview number {interview_number}.")

    if not transcript.exists():
        print(f"Transcript file not found: {transcript}")
        return

    with open(transcript, "r") as f:
        transcript_data = json.load(f)
    transcription = transcript_data.get("transcription")

    seed_query = get_seed_query(interview, vimeo_video, transcription.get("segments", []))
    # print(f"Seed query: {json.dumps(seed_query)}")
    res = execute_seed_query(seed_query, graphql_endpoint)
    print(f"Seed query response: {json.dumps(res, indent=4)}")


if __name__ == "__main__":
    main()
