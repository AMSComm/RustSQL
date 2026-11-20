#!/usr/bin/env python3
"""
Universal cross-platform packaging script for RustSQL (ruql) release artifacts.
Compatible with Python 3.8+ on Linux, macOS, and Windows runners.
"""

import hashlib
import os
import shutil
import sys
import tarfile
import zipfile

def main():
    if len(sys.argv) < 3:
        print("Usage: package.py <target-triple> <tag-name>")
        sys.exit(1)

    target = sys.argv[1]
    tag = sys.argv[2]
    is_windows = "windows" in target.lower()

    bin_name = "ruql.exe" if is_windows else "ruql"
    bin_path = os.path.join("target", target, "release", bin_name)

    if not os.path.isfile(bin_path):
        # Fallback to default target directory if single-target build was used
        alt_path = os.path.join("target", "release", bin_name)
        if os.path.isfile(alt_path):
            bin_path = alt_path
        else:
            print(f"Error: Binary not found at {bin_path} or {alt_path}")
            sys.exit(1)

    ext = "zip" if is_windows else "tar.gz"
    archive_name = f"ruql-{tag}-{target}.{ext}"

    out_dir = "artifacts"
    os.makedirs(out_dir, exist_ok=True)
    archive_path = os.path.join(out_dir, archive_name)

    stage_dir = os.path.join("target", "package_stage")
    if os.path.exists(stage_dir):
        shutil.rmtree(stage_dir)
    os.makedirs(stage_dir, exist_ok=True)

    dest_bin = os.path.join(stage_dir, bin_name)
    shutil.copy2(bin_path, dest_bin)
    if not is_windows:
        os.chmod(dest_bin, 0o755)

    for doc in ["README.md", "CHANGELOG.md", "LICENSE"]:
        if os.path.isfile(doc):
            shutil.copy2(doc, os.path.join(stage_dir, doc))

    print(f"Packaging files from {stage_dir} into {archive_path}...")
    if is_windows:
        with zipfile.ZipFile(archive_path, "w", compression=zipfile.ZIP_DEFLATED) as zf:
            for fname in os.listdir(stage_dir):
                full_path = os.path.join(stage_dir, fname)
                zf.write(full_path, arcname=fname)
    else:
        with tarfile.open(archive_path, "w:gz") as tf:
            for fname in os.listdir(stage_dir):
                full_path = os.path.join(stage_dir, fname)
                tf.add(full_path, arcname=fname)

    # Calculate SHA256 checksum
    hasher = hashlib.sha256()
    with open(archive_path, "rb") as f:
        while chunk := f.read(65536):
            hasher.update(chunk)
    digest = hasher.hexdigest()

    sha_file = f"{archive_path}.sha256"
    with open(sha_file, "w") as f:
        f.write(f"{digest}  {archive_name}\n")

    print(f"Successfully created: {archive_path} (SHA256: {digest})")

if __name__ == "__main__":
    main()
