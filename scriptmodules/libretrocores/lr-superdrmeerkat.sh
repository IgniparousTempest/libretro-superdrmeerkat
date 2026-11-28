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

function _has_rust_lr-superdrmeerkat() {
    compareVersions "$(rustc --version 2>/dev/null | cut -d' ' -f2)" ge 1.85 &&
        compareVersions "$(cargo --version 2>/dev/null | cut -d' ' -f2)" ge 1.85
}

function depends_lr-superdrmeerkat() {
    [[ "$md_mode" == "remove" ]] && return 0
    aptUpdate
    _has_rust_lr-superdrmeerkat && return 0

    if ! aptInstall --no-install-recommends rustc cargo || ! _has_rust_lr-superdrmeerkat; then
        md_ret_errors+=("Super Dr Meerkat requires rustc and cargo 1.85 or newer on PATH.")
        return 1
    fi
}

function sources_lr-superdrmeerkat() {
    gitPullOrClone
}

function build_lr-superdrmeerkat() {
    md_ret_require="$md_build/superdrmeerkat_libretro.so"
    rm -f "$md_ret_require"
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
    defaultRAConfig "$port" "
        video_driver gl
        libretro_info_path $md_inst"
}
