#!/usr/bin/env python3
"""OpenAI Images API adapter for endpoints that return `data[].url`.

The bundled Codex imagegen CLI expects `data[].b64_json`. This small adapter
keeps the same API semantics, immediately downloads signed URL responses, and
validates image files before atomically publishing them.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import sys
import tempfile
from datetime import datetime, timezone
from pathlib import Path
from urllib.parse import urlparse
from urllib.request import Request, urlopen

from openai import OpenAI
from PIL import Image

DEFAULT_ENV = Path.home() / ".config/codex/imagegen.env"
MAX_DOWNLOAD_BYTES = 50 * 1024 * 1024
CHUNK_SIZE = 64 * 1024


def die(message: str) -> None:
    raise SystemExit(message)


def load_env(path: Path) -> None:
    if not path.is_file():
        die(f"imagegen config not found: {path}")
    for raw_line in path.read_text().splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        if "=" not in line:
            die(f"invalid config line in {path}: {raw_line!r}")
        key, value = line.split("=", 1)
        value = value.strip()
        if len(value) >= 2 and value[0] == value[-1] and value[0] in "\"'":
            value = value[1:-1]
        os.environ.setdefault(key.strip(), value)


def ensure_config() -> None:
    if not os.getenv("OPENAI_API_KEY"):
        die("OPENAI_API_KEY is missing")
    if not os.getenv("OPENAI_BASE_URL"):
        die("OPENAI_BASE_URL is missing")


def download_image(url: str, destination: Path, force: bool) -> None:
    parsed = urlparse(url)
    if parsed.scheme != "https" or not parsed.netloc:
        die("image response URL must be HTTPS")
    if destination.exists() and not force:
        die(f"output already exists: {destination} (use --force to overwrite)")

    destination.parent.mkdir(parents=True, exist_ok=True)
    request = Request(url, headers={"User-Agent": "comet-imagegen/1"})
    with tempfile.NamedTemporaryFile(
        prefix=f".{destination.name}.", suffix=".download", dir=destination.parent, delete=False
    ) as temp_file:
        temp_path = Path(temp_file.name)
        try:
            with urlopen(request, timeout=180) as response:
                content_type = response.headers.get_content_type()
                if not content_type.startswith("image/"):
                    die(f"unexpected download content type: {content_type}")
                total = 0
                while chunk := response.read(CHUNK_SIZE):
                    total += len(chunk)
                    if total > MAX_DOWNLOAD_BYTES:
                        die("download exceeds 50 MiB limit")
                    temp_file.write(chunk)

            with Image.open(temp_path) as image:
                image.verify()
            shutil.move(temp_path, destination)
        finally:
            temp_path.unlink(missing_ok=True)


def write_metadata(args: argparse.Namespace, destination: Path) -> None:
    metadata = {
        "created_at": datetime.now(timezone.utc).isoformat(),
        "operation": args.command,
        "model": args.model,
        "quality": args.quality,
        "size": args.size,
        "prompt": args.prompt,
        "input_images": [str(path) for path in getattr(args, "image", [])],
        "output": str(destination),
    }
    destination.with_suffix(destination.suffix + ".json").write_text(
        json.dumps(metadata, ensure_ascii=False, indent=2) + "\n"
    )


def result_url(result: object) -> str:
    data = getattr(result, "data", None)
    if not data:
        die("Images API returned no data")
    url = getattr(data[0], "url", None)
    if not url:
        die("Images API response contains neither a usable URL nor a downloadable image")
    return url


def generate(client: OpenAI, args: argparse.Namespace) -> str:
    result = client.images.generate(
        model=args.model,
        prompt=args.prompt,
        quality=args.quality,
        size=args.size,
        n=1,
    )
    return result_url(result)


def edit(client: OpenAI, args: argparse.Namespace) -> str:
    handles = [path.open("rb") for path in args.image]
    try:
        images: object = handles[0] if len(handles) == 1 else handles
        result = client.images.edit(
            model=args.model,
            image=images,
            prompt=args.prompt,
            quality=args.quality,
            size=args.size,
            n=1,
        )
    finally:
        for handle in handles:
            handle.close()
    return result_url(result)


def add_common(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--model", default="gpt-image-2")
    parser.add_argument("--prompt", required=True)
    parser.add_argument("--quality", choices=("low", "medium", "high", "auto"), default="medium")
    parser.add_argument("--size", default="1024x1024")
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--force", action="store_true")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--env-file", type=Path, default=DEFAULT_ENV)
    subparsers = parser.add_subparsers(dest="command", required=True)
    generate_parser = subparsers.add_parser("generate")
    add_common(generate_parser)
    edit_parser = subparsers.add_parser("edit")
    add_common(edit_parser)
    edit_parser.add_argument("--image", type=Path, action="append", required=True)
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    load_env(args.env_file)
    ensure_config()
    if args.command == "edit":
        missing = [path for path in args.image if not path.is_file()]
        if missing:
            die(f"input image not found: {missing[0]}")

    client = OpenAI()
    print(f"Calling Images API ({args.command}); signed URLs and credentials will not be logged.")
    url = generate(client, args) if args.command == "generate" else edit(client, args)
    download_image(url, args.out, args.force)
    write_metadata(args, args.out)
    with Image.open(args.out) as image:
        print(f"Wrote {args.out} ({image.width}x{image.height}, {image.format})")


if __name__ == "__main__":
    main()
