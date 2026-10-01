#!/usr/bin/env python3
"""Assemble verbatim shipping dependency legal materials from verified local bytes."""

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import subprocess
import tarfile
import tomllib


TARGETS = (
    "x86_64-unknown-linux-gnu",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
)
MAX_LEGAL_BYTES = 2 * 1024 * 1024
LEGAL_PREFIXES = ("LICENSE", "LICENCE", "COPYING", "NOTICE", "COPYRIGHT")


def digest(data):
    return hashlib.sha256(data).hexdigest()


def legal_bytes(data):
    if not data or len(data) > MAX_LEGAL_BYTES or b"\0" in data:
        raise ValueError("invalid or excessive legal material")
    data.decode("utf-8")
    return data


def selected_packages(repo, target):
    """Cargo selects default-feature normal/build dependencies for shipped binaries."""
    if target not in TARGETS:
        raise ValueError("unsupported shipping target")
    roots = ["retonr-cli"]
    if target == TARGETS[0]:
        roots.append("rewrite-runtime-isolation")
    selected = set()
    for root in roots:
        output = subprocess.check_output(
            ["cargo", "tree", "--locked", "--offline", "--target", target,
             "--package", root, "--edges", "normal,build", "--prefix", "none",
             "--format", "{p}"], cwd=repo, text=True, encoding="utf-8"
        )
        for line in output.splitlines():
            match = re.match(r"^(\S+) v(\S+)(?: |$)", line)
            if not match:
                raise ValueError("unexpected dependency tree output")
            selected.add(match.groups())
    lock = tomllib.loads((repo / "Cargo.lock").read_text(encoding="utf-8"))
    packages = {}
    for package in lock["package"]:
        key = (package["name"], package["version"])
        if key not in selected:
            continue
        if key in packages:
            raise ValueError("ambiguous locked dependency source")
        packages[key] = package
    if packages.keys() != selected:
        raise ValueError("dependency absent from lockfile")
    return [packages[key] for key in sorted(packages) if "source" in packages[key]]


def cached_archive(cargo_home, package):
    slug = f'{package["name"]}-{package["version"]}.crate'
    if package["source"] != "registry+https://github.com/rust-lang/crates.io-index":
        raise ValueError("unsupported dependency source")
    matches = list((cargo_home / "registry/cache").glob(f"*/{slug}"))
    verified = [path for path in matches if digest(path.read_bytes()) == package["checksum"]]
    if not verified:
        raise ValueError(f"locked crate archive unavailable: {slug}")
    return sorted(verified)[0]


def crate_materials(archive, package):
    if digest(archive.read_bytes()) != package["checksum"]:
        raise ValueError("crate archive checksum mismatch")
    slug = f'{package["name"]}-{package["version"]}'
    materials = []
    with tarfile.open(archive, "r:gz") as stream:
        members = stream.getmembers()
        if len(members) > 100_000 or len({item.name for item in members}) != len(members):
            raise ValueError("excessive or duplicate crate archive members")
        manifest = stream.getmember(f"{slug}/Cargo.toml")
        if not manifest.isfile() or manifest.size > MAX_LEGAL_BYTES:
            raise ValueError("invalid crate package manifest")
        metadata = tomllib.loads(stream.extractfile(manifest).read().decode("utf-8"))["package"]
        if (metadata["name"], metadata["version"]) != (package["name"], package["version"]):
            raise ValueError("crate package identity mismatch")
        expression = metadata.get("license", "")
        if not expression:
            raise ValueError("crate license declaration missing")
        declared = metadata.get("license-file")
        for member in members:
            path = PurePosixPath(member.name)
            if path.is_absolute() or ".." in path.parts or path.parts[0] != slug:
                raise ValueError("invalid crate member path")
            relative = path.relative_to(slug)
            legal = relative.name.upper().startswith(LEGAL_PREFIXES) or str(relative) == declared
            if not legal or member.isdir():
                continue
            if not member.isfile() or member.size > MAX_LEGAL_BYTES:
                raise ValueError("legal material must be a bounded regular archive member")
            materials.append((str(relative), legal_bytes(stream.extractfile(member).read())))
    return expression, sorted(materials), metadata.get("authors", [])


def retained_fallback(repo, package, expression):
    root = repo / "support/third-party-licenses"
    manifest = json.loads((root / "manifest.json").read_text(encoding="utf-8"))
    key = f'{package["name"]}@{package["version"]}'
    entry = manifest["packages"].get(key)
    if entry is None or entry["archive_sha256"] != package["checksum"]:
        raise ValueError(f"upstream legal material missing for {key}")
    if entry["declared_spdx_expression"] != expression or not entry["license_terms"]:
        raise ValueError("retained license declaration does not match the archive")
    path = PurePosixPath(entry["file"])
    if path.is_absolute() or ".." in path.parts:
        raise ValueError("invalid fallback path")
    data = legal_bytes((root / path).read_bytes())
    if digest(data) != entry["sha256"]:
        raise ValueError("retained legal material checksum mismatch")
    materials = [(f'upstream license declaration or notice ({entry["url"]})', data)]
    for identifier in entry["license_terms"]:
        terms = manifest["canonical_terms"][identifier]
        path = PurePosixPath(terms["file"])
        if path.is_absolute() or ".." in path.parts:
            raise ValueError("invalid retained canonical license path")
        data = legal_bytes((root / path).read_bytes())
        if digest(data) != terms["sha256"]:
            raise ValueError("canonical license terms checksum mismatch")
        materials.append((f'{identifier} canonical terms ({terms["url"]})', data))
    return materials


def rust_materials(sysroot):
    root = sysroot / "share/doc/rust"
    # Preserve complete library copyright information, including third-party
    # attribution, and the distribution's actual referenced legal texts.
    paths = [root / "COPYRIGHT-library.html"]
    paths += sorted((root / "licenses").glob("*.txt"))
    if not all((root / "licenses" / name).is_file() for name in ["MIT.txt", "Apache-2.0.txt"]):
        raise ValueError("Rust distribution legal texts missing")
    return [(str(path.relative_to(root)), legal_bytes(path.read_bytes())) for path in paths]


def document(target, packages, cargo_home, repo, sysroot):
    chunks = [f"Third-party legal notices\nShipping target: {target}\n\n".encode()]
    for package in packages:
        expression, materials, authors = crate_materials(cached_archive(cargo_home, package), package)
        if not materials:
            materials = retained_fallback(repo, package, expression)
        chunks.append((f'{"=" * 72}\n{package["name"]} {package["version"]}\n'
                       f'Source: {package["source"]}\nArchive SHA-256: {package["checksum"]}\n'
                       f'Declared license: {expression}\n'
                       f'Package authors: {json.dumps(authors, ensure_ascii=False)}\n\n').encode())
        for path, data in materials:
            chunks.extend([f"--- {path} ---\n".encode(), data, b"\n\n"])
    chunks.append(b"========================================================================\nRust standard library distribution\n\n")
    for path, data in rust_materials(sysroot):
        chunks.extend([f"--- {path} ---\n".encode(), data, b"\n\n"])
    return b"".join(chunks)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=TARGETS, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    repo = Path(__file__).resolve().parent.parent
    cargo_home = Path(os.environ.get("CARGO_HOME", str(Path.home() / ".cargo")))
    sysroot = Path(subprocess.check_output(["rustc", "--print", "sysroot"], text=True).strip())
    packages = selected_packages(repo, args.target)
    data = document(args.target, packages, cargo_home, repo, sysroot)
    with args.output.open("xb") as output:
        output.write(data)
    print(f"Third-party notices: {len(packages)} registry packages; {len(data)} bytes")


if __name__ == "__main__":
    main()
