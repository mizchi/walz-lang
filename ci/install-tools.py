"""Install the pinned Linux CI tools; only extract the requested executables."""
import hashlib
import io
import json
import os
import platform
from pathlib import Path
import subprocess
import tarfile
import urllib.request

if platform.system() != "Linux" or platform.machine() != "x86_64":
    raise SystemExit("CI tools are pinned for Linux x86_64")
configuration = json.loads(Path("ci/tools.json").read_text())
destination = Path("target/ci-tools").resolve()
destination.mkdir(parents=True, exist_ok=True)
for tool in configuration["tools"]:
    binary = destination / tool["binary"]
    if binary.exists():
        continue
    request = urllib.request.Request(tool["url"], headers={"User-Agent": "wlt-ci"})
    with urllib.request.urlopen(request, timeout=120) as response:
        data = response.read()
    if hashlib.sha256(data).hexdigest() != tool["sha256"]:
        raise SystemExit(f"checksum mismatch: {tool['binary']}")
    with tarfile.open(fileobj=io.BytesIO(data)) as archive:
        entries = [entry for entry in archive if entry.isfile() and Path(entry.name).name == tool["binary"]]
        if len(entries) != 1:
            raise SystemExit(f"ambiguous executable: {tool['binary']}")
        binary.write_bytes(archive.extractfile(entries[0]).read())
    binary.chmod(0o755)
elan = Path.home() / ".elan/bin/elan"
if not elan.exists():
    subprocess.run([str(destination / "elan-init"), "-y", "--no-modify-path", "--default-toolchain", "none"], check=True)
subprocess.run([str(elan), "toolchain", "install", configuration["lean"]], check=True)
subprocess.run([str(elan), "default", configuration["lean"]], check=True)
with open(os.environ["GITHUB_PATH"], "a") as output:
    output.write(str(destination) + "\n" + str(elan.parent) + "\n")
