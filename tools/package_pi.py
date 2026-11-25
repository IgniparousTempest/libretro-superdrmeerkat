#!/usr/bin/env python3
"""Build the generic 64-bit Raspberry Pi package."""
from package_release import build_release


if __name__ == "__main__":
    print(build_release("pi-arm64"))
