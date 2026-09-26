#!/usr/bin/env python3
"""Mirror a Vimeo folder into the transcription inbox, one video at a time.

    VIMEO_TOKEN=... scripts/vimeo-sync.py list          # write the canonical manifest, print what's missing
    VIMEO_TOKEN=... scripts/vimeo-sync.py next DIR      # download the first missing video into DIR, print its path

The folder listing comes from the Vimeo API rather than yt-dlp, which is unreliable
on semi-visible assets in a shared team library. The token needs the `private` and
`video_files` scopes and must belong to an account with access to the folder.

A video counts as present when a file with the same normalized stem is already in
the inbox, so re-running after a partial batch picks up where it stopped.
"""

import json
import os
import re
import sys
import urllib.parse
import urllib.request
from pathlib import Path

API = "https://api.vimeo.com"
OWNER = os.environ.get("VIMEO_OWNER", "9495833")
FOLDER = os.environ.get("VIMEO_FOLDER", "3570988")
INBOX = Path(os.environ.get("INBOX", "/mnt/s3/fs1/in"))
MANIFEST = Path(os.environ.get("MANIFEST", "/mnt/s3/fs1/vimeo-manifest.json"))
FIELDS = "uri,name,duration,type,download.quality,download.type,download.width,download.size,download.link"
EXT = {"video/mp4": ".mp4", "video/quicktime": ".mov", "video/x-m4v": ".m4v"}


def get(url):
    if not url.startswith("http"):
        url = API + url
    req = urllib.request.Request(url, headers={
        "Authorization": f"bearer {os.environ['VIMEO_TOKEN']}",
        "Accept": "application/vnd.vimeo.*+json;version=3.4",
    })
    with urllib.request.urlopen(req, timeout=60) as r:
        return json.load(r)


def paged(path):
    while path:
        page = get(path)
        yield from page.get("data", [])
        path = (page.get("paging") or {}).get("next")


def walk(folder_id):
    """Videos in a folder, recursing into subfolders."""
    for item in paged(f"/users/{OWNER}/projects/{folder_id}/items?per_page=100&fields=type,video.{FIELDS.replace(',', ',video.')},folder.uri,folder.name"):
        if item.get("type") == "video":
            yield item["video"]
        elif item.get("type") == "folder":
            yield from walk(item["folder"]["uri"].rstrip("/").rsplit("/", 1)[-1])


def stem(name):
    s = re.sub(r"\.(mp4|mov|m4v|mxf)$", "", name.strip(), flags=re.I)
    return re.sub(r"[^a-z0-9]+", "_", s.lower()).strip("_")


def best_download(video):
    files = video.get("download") or []
    source = [f for f in files if f.get("quality") == "source"]
    pool = source or files
    return max(pool, key=lambda f: (f.get("width") or 0, f.get("size") or 0), default=None)


def manifest():
    rows = []
    for v in walk(FOLDER):
        dl = best_download(v)
        rows.append({
            "id": v["uri"].rsplit("/", 1)[-1],
            "name": v["name"],
            "stem": stem(v["name"]),
            "duration": v.get("duration"),
            "quality": dl and dl.get("quality"),
            "size": dl and dl.get("size"),
            "ext": EXT.get(dl and dl.get("type"), ".mp4"),
            "downloadable": bool(dl and dl.get("link")),
        })
    rows.sort(key=lambda r: r["stem"])
    MANIFEST.write_text(json.dumps(rows, indent=2) + "\n")
    return rows


def present():
    return {stem(p.name) for p in INBOX.iterdir() if p.is_file()}


def missing(rows):
    have = present()
    return [r for r in rows if r["stem"] not in have]


def download(row, dest_dir):
    video = get(f"/videos/{row['id']}?fields={FIELDS}")
    dl = best_download(video)
    if not dl or not dl.get("link"):
        raise SystemExit(f"{row['name']}: no download link (token lacks video_files scope, or downloads disabled)")
    dest_dir.mkdir(parents=True, exist_ok=True)
    final = dest_dir / f"{row['stem']}{row['ext']}"
    part = final.with_suffix(final.suffix + ".part")
    have = part.stat().st_size if part.exists() else 0
    req = urllib.request.Request(dl["link"], headers={"Range": f"bytes={have}-"} if have else {})
    with urllib.request.urlopen(req, timeout=120) as r, open(part, "ab" if have and r.status == 206 else "wb") as out:
        while chunk := r.read(8 << 20):
            out.write(chunk)
    if dl.get("size") and part.stat().st_size != dl["size"]:
        raise SystemExit(f"{row['name']}: size mismatch ({part.stat().st_size} != {dl['size']})")
    part.rename(final)
    return final


def main():
    cmd = sys.argv[1] if len(sys.argv) > 1 else "list"
    rows = manifest()
    todo = missing(rows)
    if cmd == "list":
        print(f"{len(rows)} in folder, {len(rows) - len(todo)} present, {len(todo)} missing", file=sys.stderr)
        for r in todo:
            flag = "" if r["downloadable"] else "  [no download link]"
            print(f"{r['id']}\t{r['stem']}{r['ext']}\t{r['quality']}\t{r['size']}{flag}")
    elif cmd == "next":
        todo = [r for r in todo if r["downloadable"]]
        if not todo:
            return
        print(download(todo[0], Path(sys.argv[2])))
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main()
