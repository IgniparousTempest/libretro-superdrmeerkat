#!/usr/bin/env bash

# This file is part of The RetroPie Project
#
# The RetroPie Project is the legal property of its developers, whose names are
# too numerous to list here. Please refer to the COPYRIGHT.md file distributed with this source.
#
# See the LICENSE.md file at the top-level directory of this distribution and
# at https://raw.githubusercontent.com/RetroPie/RetroPie-Setup/master/LICENSE.md
#

rp_module_id="lr-superdrmeerkat"
rp_module_desc="Super Dr Meerkat - arcade capsule puzzle for one to four players"
rp_module_help="https://github.com/IgniparousTempest/libretro-superdrmeerkat/wiki"
rp_module_licence="GPL3 https://raw.githubusercontent.com/IgniparousTempest/libretro-superdrmeerkat/master/LICENSE"
rp_module_repo="git https://github.com/IgniparousTempest/libretro-superdrmeerkat.git master"
rp_module_section="exp"
rp_module_flags="!all rpi2 rpi3 rpi4 rpi5"

function depends_lr-superdrmeerkat() {
    getDepends build-essential curl
}

function sources_lr-superdrmeerkat() {
    gitPullOrClone
}

function build_lr-superdrmeerkat() {
    md_ret_require="$md_build/superdrmeerkat_libretro.so"
    rm -f "$md_ret_require"

    # Older Raspberry Pi OS releases ship a compiler below the game's Rust 1.85 minimum.
    if ! command -v cargo >/dev/null || ! compareVersions "$(rustc --version 2>/dev/null | cut -d' ' -f2)" ge 1.85; then
        local -x CARGO_HOME="$md_build/.cargo-home" RUSTUP_HOME="$md_build/.rustup"
        local PATH="$CARGO_HOME/bin:$PATH"
        local rustup_version="1.28.2"
        download "https://raw.githubusercontent.com/rust-lang/rustup/$rustup_version/rustup-init.sh" rustup-init.sh || return 1
        echo "17247e4bcacf6027ec2e11c79a72c494c9af69ac8d1abcc1b271fa4375a106c2  rustup-init.sh" | sha256sum -c - || return 1
        RUSTUP_VERSION="$rustup_version" sh rustup-init.sh -y --no-modify-path --profile minimal --default-toolchain 1.85.1 || return 1
    fi

    cargo build --release --locked --jobs "$__jobs" --target-dir target -p superdrmeerkat-libretro || return 1
    cp target/release/libsuperdrmeerkat_libretro.so "$md_ret_require"
}

function install_lr-superdrmeerkat() {
    md_ret_files=(
        'superdrmeerkat_libretro.so'
        'libretro/superdrmeerkat_libretro.info'
        'assets/source/fonts/barlow-OFL.txt'
        'assets/source/fonts/barlowcondensed-OFL.txt'
        'LICENSE'
        'README.md'
    )
}

function configure_lr-superdrmeerkat() {
    setConfigRoot "ports"

    local port="super-dr-meerkat"
    addPort "$md_id" "$port" "Super Dr Meerkat" "$md_inst/superdrmeerkat_libretro.so"
    defaultRAConfig "$port" "video_driver" "gl"
}
