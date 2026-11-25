# Super Dr Meerkat

An arcade game for one to four players, inspired by Dr. Mario. Drop capsules to clear germs by matching four of the same colour in a row or column.

![Dr Meerkat and his secretary bird on the title screen](.readme/title.png)
![Four players racing to clear their bottles](.readme/four-player.png)

## Installation

Download the [release archive](https://github.com/IgniparousTempest/libretro-superdrmeerkat/releases) for your system. Extract it and run `./super-dr-meerkat` (`super-dr-meerkat.exe` on Windows).

Set `max_players` and `resolution` in `super-dr-meerkat.toml`.

## Building from source

```sh
sudo apt install build-essential libsdl2-dev python3
make config
cargo run --release --locked
```
