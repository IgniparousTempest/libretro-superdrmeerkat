PYTHON ?= $(if $(filter Windows_NT,$(OS)),python,python3)
CONFIG_OUT ?= super-dr-meerkat.toml

.PHONY: config package

config:
	$(PYTHON) tools/config_profile.py --platform "$(PLATFORM)" --output "$(CONFIG_OUT)"

package:
	$(PYTHON) tools/package_release.py $(if $(PLATFORM),"$(PLATFORM)") $(if $(SDL_SDK),--sdl-sdk "$(SDL_SDK)")
