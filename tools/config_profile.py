"""Select a checked-in cabinet configuration for a release target or the local machine."""
import argparse
import re
import shutil
import sys
import tempfile
from pathlib import Path
from platform import machine

PROFILES = Path(__file__).resolve().parents[1] / "config"
CONFIG_NAME = "super-dr-meerkat.toml"


def host_platform():
    if sys.platform == "linux":
        try:
            model = Path("/proc/device-tree/model").read_bytes()
        except OSError:
            return "linux"
        board = re.match(rb"Raspberry Pi (?:Compute Module )?(\d+)\b", model)
        if board:
            arch = "arm64" if sys.maxsize > 2**32 else "armhf"
            return f"pi{int(board[1])}-{arch}"
    if sys.platform == "darwin":
        return f"macos-{machine().lower()}"
    if sys.platform == "win32":
        return "windows-x64" if sys.maxsize > 2**32 else "windows-x86"
    return sys.platform


def select_profile(platform):
    profiles = {path.stem: path for path in PROFILES.glob("*.toml")}
    family = platform.partition("-")[0]
    candidates = [platform, family]
    if family in ("pi2", "pi3"):
        candidates.append("pi2-pi3")
    for name in candidates:
        if name in profiles:
            return profiles[name]
    return profiles["default"]


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--platform", help="target platform (auto-detected when omitted)")
    parser.add_argument("--output", type=Path, default=Path(CONFIG_NAME))
    args = parser.parse_args()
    target = args.platform or host_platform()
    source = select_profile(target)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=args.output.parent) as tmp:
        staged = Path(tmp) / CONFIG_NAME
        shutil.copy(source, staged)
        staged.replace(args.output)
    print(f"{target}: config/{source.name} -> {args.output}")
