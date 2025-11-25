set shell := ["bash", "-euo", "pipefail", "-c"]

project   := "wpe"
# path to the release binary we'll install
release_bin := "target/release/" + project
icon := "linux/io.melechtna.wpe.svg"
desktop   := "linux/io.melechtna.wpe.desktop"
auto   := "linux/io.melechtna.wpe-autostart.desktop"
metainfo  := "linux/io.melechtna.wpe.metainfo.xml"

default: build

build:
	cargo build --release

_install_checks:
	if [ ! -x "{{release_bin}}" ]; then echo "▶ Building release binary" >&2; cargo build --release >&2; fi
	[ -f "{{release_bin}}" ] || { echo "❌ Missing {{release_bin}}" >&2; exit 1; }
	[ -f "{{icon}}" ] || { echo "❌ Missing icon: {{icon}}" >&2; exit 1; }
	[ -f "{{desktop}}" ] || { echo "❌ Missing desktop file: {{desktop}}" >&2; exit 1; }
	[ -f "{{metainfo}}" ] || { echo "❌ Missing metainfo: {{metainfo}}" >&2; exit 1; }

install: _install_checks
	sudo bash -euo pipefail -c 'set -euo pipefail; \
	  install -Dm755 "{{release_bin}}" /usr/bin/{{project}}; \
	  install -Dm644 "{{icon}}" /usr/share/icons/hicolor/scalable/apps/io.melechtna.wpe.svg; \
	  install -Dm644 "{{desktop}}" /usr/share/applications/io.melechtna.wpe.desktop; \
	  install -Dm644 "{{auto}}" /usr/share/applications/io.melechtna.wpe-autostart.desktop; \
	  install -Dm644 "{{metainfo}}" /usr/share/metainfo/io.melechtna.wpe.metainfo.xml; \
	  gtk-update-icon-cache -f /usr/share/icons/hicolor || true; \
	  update-desktop-database -q /usr/share/applications || true;'
	echo "✅ Installed wpe"

uninstall:
	sudo bash -euo pipefail -c 'set -euo pipefail; \
	  rm -f /usr/bin/{{project}}; \
	  rm -f /usr/share/applications/io.melechtna.wpe.desktop; \
	  rm -f /usr/share/applications/io.melechtna.wpe-autostart.desktop; \
	  rm -f /usr/share/metainfo/io.melechtna.wpe.metainfo.xml; \
	  rm -f "/usr/share/icons/hicolor/scalable/apps/io.melechtna.wpe.svg"; \
	  gtk-update-icon-cache -f /usr/share/icons/hicolor || true; \
	  update-desktop-database -q /usr/share/applications || true;'
	echo "✅ Uninstalled wpe"
