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


def match_interview(interview_fname: str, interview_data) -> Interview | None:
    interviews = interview_data.get("interviews", [])
    sm = SequenceMatcher()
    sm.set_seq2(interview_fname)
    ratios = [(interview, sm.set_seq1(interview.get("filename")) or sm.ratio()) for interview in interviews]
    ratios = sorted(ratios, key=lambda x: x[1], reverse=True)
    most_similar_interview = ratios[0]
    if most_similar_interview and most_similar_interview[1] >= 0.5:
        return Interview(
            **most_similar_interview[0],
        )
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
@click.argument("transcript_dir", type=Path)
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
def main(
    vimeo_manifest: Path,
    graphql_endpoint: str,
    interview_manifest: Path,
    transcript_dir: Path,
):
    with open(interview_manifest, "r") as f:
        interview_data = yaml.safe_load(f)
    if not interview_data:
        print(f"Failed to load interview data from: {interview_manifest}")
        return
    with open(vimeo_manifest, "r") as f:
        vimeo_data = yaml.safe_load(f)
    if not vimeo_data:
        print(f"Failed to load Vimeo data from: {vimeo_manifest}")
        return

    for transcript in transcript_dir.glob("*.json"):
        name = transcript.name
        print(f"Seeding interview from transcript: {name}")

        interview = match_interview(transcript.stem, interview_data)
        if not interview:
            print(
                f"Interview {name} not found in the interview manifest."
            )
            return

        vimeo_video = match_vimeo_video(interview, vimeo_data)
        if not vimeo_video:
            print(f"No matching Vimeo video found for interview {name}.")


        with open(transcript, "r") as f:
            transcript_data = json.load(f)
        transcription = transcript_data.get("transcription")

        seed_query = get_seed_query(interview, vimeo_video, transcription.get("segments", []))
        # print(f"Seed query: {json.dumps(seed_query)}")
        res = execute_seed_query(seed_query, graphql_endpoint)
        print(f"Seed query response: {json.dumps(res, indent=4)}")


if __name__ == "__main__":
    main()
