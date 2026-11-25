#!/bin/sh
game="$(dirname "$0")/super-dr-meerkat"
exec /opt/retropie/emulators/retroarch/bin/retroarch \
    --config /opt/retropie/configs/all/retroarch.cfg \
    --appendconfig "$game/frontends/video.cfg" \
    -L "$game/superdrmeerkat_libretro.so" "$game/super-dr-meerkat.toml"
