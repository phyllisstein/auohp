# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

The **ACT UP Oral History Project** (AUOHP) is a toolchain for transcribing, editing, and searching oral history interview videos. It processes interview footage through an AI transcription pipeline and stores the results in a Neo4j graph database, then exposes them via a caption editor and search interface.

## Monorepo Structure

This is a Yarn 4 workspace monorepo (`packages/*`). The package manager is **Yarn** — always use `yarn`, never `npm`.

| Package | Description |
|---|---|
| `packages/caption-editor-app` | Next.js 15 app (port 3030) — transcript review/edit UI backed directly by Neo4j |
| `packages/search-component` | Vite/React app (port 4040) — full-text and vector search over transcripts, connects to Neo4j from the browser |
| `packages/scripts` | Node.js seeding scripts — parses Whisper JSON output and seeds Neo4j |
| `packages/scripts/src/subwhisp` | Python CLI — runs WhisperX transcription + speaker diarization on audio/video files |
| `packages/caption-server-api` | NestJS API (port 5050, compiled only in `dist/`) — thin wrapper around Neo4j; largely superseded by Next.js API routes |

## Common Commands

```bash
# Install dependencies (run from repo root)
yarn install

# Start all packages in parallel (dev mode)
yarn packages:dev

# Start caption editor only (port 3030)
cd packages/caption-editor-app && yarn start:dev

# Start search component only (port 4040)
cd packages/search-component && yarn start:dev

# Seed the Neo4j database from Whisper JSON assets
cd packages/scripts && yarn seed

# Lint (from root)
yarn eslint .
yarn stylelint .
```

### subwhisp (Python transcription CLI)

```bash
# Set up conda environment
conda create -n auohp
conda activate auohp
conda env update -f environment.yml

# Install ML dependencies (choose one)
python install_dependencies.py --cuda   # Nvidia GPU
python install_dependencies.py --no-cuda

# Install the CLI via Poetry
poetry config virtualenvs.create false && poetry install

# Download models (large-v3 whisper + wav2vec2 + pyannote diarization)
subwhisp models

# Transcribe a video/audio file → produces .json, .speakers.json, .vtt, .captions.json
subwhisp transcribe <input_file>

# Re-generate captions from an existing .json transcript
subwhisp caption <input_file.json>
```

### Docker

```bash
# Start app + Neo4j (production-style, uses nginx-proxy network)
docker compose up

# Demo compose (standalone, no external network required)
docker compose -f demo.docker-compose.yml up
```

## Architecture

### Data Flow

```
Video files → subwhisp (WhisperX + pyannote) → JSON transcripts
→ packages/scripts yarn seed → Neo4j graph database
→ caption-editor-app (Next.js) ← edits persisted back to Neo4j
→ search-component (Vite) ← full-text / vector search
```

### Neo4j Graph Model

Nodes: `Interview`, `Transcript`, `Statement`, `Person`, `Speaker` (`:Interviewer`/`:Interviewee`), `Video`, `VTT`, `Action`, `Broadsheet`, `Documentary`, `Caption`

Key relationships:
- `(Interview)-[:HAS_TRANSCRIPT]->(Transcript)-[:TRANSCRIBES {startTime, endTime}]->(Statement)`
- `(Speaker)-[:SAYS]->(Statement)`
- `(Person)-[:INTERVIEWED_AS]->(Speaker)`
- `(Interview)-[:HAS_ASSET]->(Video)-[:HAS_CAPTIONS]->(VTT|JSON)`

Neo4j is configured with APOC and GenAI plugins. Default credentials: `neo4j`/`auohpauohp`. Connection defaults: `neo4j://neo4j:7687` (Docker) or `bolt+s://bolt.auohp.here:443` (search component).

Full-text indexes: `transcript_search` on `Statement.text`, `name_search` on `Person.name`. Vector index: `statement_embedding` (1536-dim cosine, OpenAI embeddings, seeded via `seedVectorIndex()` which is currently commented out).

### caption-editor-app (Next.js)

- `src/app/neo4j.ts` — server-side Neo4j driver, all Cypher queries, shared type definitions. All API routes import from here.
- `src/app/api/` — Next.js Route Handlers: `GET /api/interviews`, `GET /api/transcript/[interview]/json`, `GET /api/transcript/[interview]/vtt`, `PUT /api/transcript`
- `src/components/editor/` — Slate.js rich-text editor for transcript review. Statement nodes are editable; changes are saved via `PUT /api/transcript`.
- `src/styles/` — Styled-components theme (Spectrum CSS tokens, custom typefaces). Global styles registered via `registry.tsx` for SSR.
- State management uses Recoil.

### search-component (Vite)

- Connects directly to Neo4j from the browser via `bolt+s://` using the `neo4j-driver` package.
- `src/hooks/infrastructure/use-neo4j.ts` — React hook that manages the driver lifecycle; reads from `VITE_NEO4J_*` env vars.
- `src/hooks/interviews/` — hooks for querying transcripts and video metadata.
- Uses `fuzzbunny` for client-side fuzzy matching.

### Transcript JSON format

The Whisper pipeline and Slate editor both use a nested structure:
```
{ type: 'transcript', children: [
  { type: 'statement', uid, speaker, speakerName, startTime, endTime,
    startTimestamp, endTimestamp, transcription,
    children: [{ text: '...' }] }
]}
```

## Environment Variables

Required in `.env` (mounted as a Docker secret at `/run/secrets/environment`):
```
NEO4J_URI=
NEO4J_USERNAME=
NEO4J_PASSWORD=
OPENAI_API_KEY=   # needed for vector embedding seeding only
```

The search component uses `VITE_NEO4J_*` prefixed versions for Vite's env injection.

## Known Issues / FIXMEs

- The `:Speaker` indirection layer is acknowledged as clumsy; the target model is `(:Transcript)-[:TRANSCRIBES]->(:Statement)<-[:SAYS]-(:Person)`.
- Interviewer vs. Interviewee distinction is not fully implemented in the editor types.
- `caption-server-api` source files were removed; only the compiled `dist/` remains. The Next.js API routes now serve the same purpose.
- `seedVectorIndex()` is commented out in the seed script; re-enable and set `OPENAI_API_KEY` to populate embeddings.
