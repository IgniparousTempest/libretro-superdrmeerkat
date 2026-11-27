#!/usr/bin/env python3
"""Build a cabinet release and write its distributable archive to target/dist."""
import argparse
import json
import os
import shlex
import subprocess
import tarfile
import tempfile
import zipfile
from pathlib import Path

from config_profile import CONFIG_NAME, host_platform, select_profile

ROOT = Path(__file__).resolve().parents[1]
NAME = "super-dr-meerkat"
CORE = "superdrmeerkat_libretro"
ARMHF = "armv7-unknown-linux-gnueabihf"
ARM64 = "aarch64-unknown-linux-gnu"
PI_CPUS = {"pi2": "cortex-a7", "pi3": "cortex-a53", "pi4": "cortex-a72", "pi5": "cortex-a76"}
TARGETS = {
    **{f"{board}-armhf": ARMHF for board in PI_CPUS},
    **{f"{board}-arm64": ARM64 for board in PI_CPUS if board != "pi2"},
    "pi-armhf": ARMHF,
    "pi-arm64": ARM64,
    "macos-arm64": "aarch64-apple-darwin",
    "windows-x86": "i686-pc-windows-gnu",
    "windows-x64": "x86_64-pc-windows-gnu",
}


def release_files(binary, core, core_name, packages, platform):
    yield binary, binary.name
    yield core, core_name
    yield ROOT / "libretro" / f"{CORE}.info", f"{CORE}.info"
    if platform.startswith("pi"):
        module = Path("scriptmodules/libretrocores/lr-superdrmeerkat.sh")
        yield ROOT / module, module.as_posix()
        for path in sorted((ROOT / "libretro/frontends").iterdir()):
            yield path, f"frontends/{path.name}"
    yield select_profile(platform), CONFIG_NAME
    yield ROOT / "README.md", "README.md"
    yield ROOT / "LICENSE", "LICENSE"
    for path in sorted((ROOT / ".readme").rglob("*")):
        if path.is_file():
            yield path, path.relative_to(ROOT).as_posix()
    for notice in sorted((ROOT / "assets/source/fonts").glob("*-OFL.txt")):
        yield notice, f"licenses/fonts/{notice.name}"
    for package in packages:
        if package["source"] is None:
            continue
        directory = Path(package["manifest_path"]).parent
        dest = f"licenses/{package['name']}-{package['version']}"
        for notice in sorted(directory.iterdir()):
            if notice.is_file() and notice.name.upper().startswith(("LICENSE", "LICENCE", "COPYING", "NOTICE")):
                yield notice, f"{dest}/{notice.name}"


def build_release(platform, sdl_sdk=None):
    target = TARGETS[platform]
    family = platform.partition("-")[0]
    windows = family == "windows"
    features = []
    flags = []
    env = os.environ.copy()
    runtime_files = []
    if family in PI_CPUS:
        flags = ["-C", f"target-cpu={PI_CPUS[family]}"]
        # Rust's generic armhf target disables NEON even when a specific core is selected.
        cpu_features = ["+neon"] if target == ARMHF else []
        if family in ("pi3", "pi4"):
            # LLVM's A53/A72 presets include optional crypto instructions absent on these Pis.
            cpu_features.extend(["-aes", "-sha2"])
        if cpu_features:
            flags += ["-C", "target-feature=" + ",".join(cpu_features)]
        # The standalone executable and the core share the game library.
        env["CARGO_ENCODED_RUSTFLAGS"] = "\x1f".join(
            (env["CARGO_ENCODED_RUSTFLAGS"].split("\x1f") if env.get("CARGO_ENCODED_RUSTFLAGS")
             else shlex.split(env.get("RUSTFLAGS", ""))) + flags)
        flags = []
    elif windows:
        cpu = target.split("-")[0]
        sdl = sdl_sdk.resolve() / f"{cpu}-w64-mingw32"
        flags = ["-L", f"native={sdl / 'lib'}"]
        runtime_files = [(sdl / "bin/SDL2.dll", "SDL2.dll"), (sdl_sdk / "LICENSE.txt", "licenses/SDL2.txt")]
    elif platform == "macos-arm64":
        framework = sdl_sdk.resolve() / "SDL2.framework"
        features = ["--features", "sdl2/use_mac_framework"]
        flags = ["-L", f"framework={framework.parent}", "-C", "link-arg=-Wl,-rpath,@executable_path"]
        runtime_files = [(framework, "SDL2.framework"), (sdl_sdk / "License.txt", "licenses/SDL2.txt")]

    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--format-version", "1", "--filter-platform", target, *features],
        cwd=ROOT))
    target_dir = Path(metadata["target_directory"])
    build_dir = target_dir / "package-builds" / platform
    subprocess.run(["cargo", "rustc", "--release", "--locked", "--target", target, *features,
                    "--target-dir", str(build_dir), "--bin", NAME, "--", *flags], cwd=ROOT, env=env, check=True)

    # Release Pi cores also run on older RetroPie images with an earlier glibc.
    portable = platform.startswith("pi") and env.get("CORE_GLIBC")
    core_target = f"{target}.{portable}" if portable else target
    if portable:
        # Let zigbuild supply its linker instead of SDL's Bookworm cross-linker.
        env.pop(f"CARGO_TARGET_{target.upper().replace('-', '_')}_LINKER", None)
    core_build_dir = build_dir / "core"
    subprocess.run(["cargo", "zigbuild" if portable else "build", "--release", "--locked",
                    "--target", core_target, "--target-dir", str(core_build_dir),
                    "-p", "superdrmeerkat-libretro"], cwd=ROOT, env=env, check=True)

    executable = NAME + (".exe" if windows else "")
    core_name = CORE + (".dll" if windows else ".dylib" if family == "macos" else ".so")
    core = core_build_dir / target / "release" / (core_name if windows else "lib" + core_name)
    files = list(release_files(build_dir / target / "release" / executable, core, core_name, metadata["packages"], platform))
    files.extend(runtime_files)

    dist = target_dir / "dist"
    dist.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(dir=dist) as tmp:
        if windows:
            archive = Path(tmp) / f"{NAME}-{platform}.zip"
            # Cargo sources can have timestamps earlier than the ZIP format allows.
            with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED, strict_timestamps=False) as out:
                for path, name in files:
                    out.write(path, f"{NAME}/{name}")
        else:
            archive = Path(tmp) / f"{NAME}-{platform}.tar.gz"
            with tarfile.open(archive, "w:gz") as out:
                for path, name in files:
                    out.add(path, f"{NAME}/{name}")
        destination = dist / archive.name
        archive.replace(destination)
        return destination


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("platform", choices=TARGETS, nargs="?", help="release target (auto-detected when omitted)")
    parser.add_argument("--sdl-sdk", type=Path, help="directory containing SDL2.framework or the extracted MinGW kit")
    args = parser.parse_args()
    if args.platform is None:
        detected = host_platform()
        # A Pi 2 with a 64-bit OS uses the Cortex-A53 build.
        args.platform = "pi3-arm64" if detected == "pi2-arm64" else detected
        if args.platform not in TARGETS:
            parser.error(f"choose a release target for cross-compiling on {detected}")
    if args.platform.startswith(("macos-", "windows-")) and args.sdl_sdk is None:
        parser.error(f"{args.platform} requires --sdl-sdk")
    print(build_release(args.platform, args.sdl_sdk))
