# NeteaseMusicPlayer-Linux

[![Build](https://github.com/mirinnano/NeteaseMusicPlayer-Linux/actions/workflows/release.yml/badge.svg)](https://github.com/mirinnano/NeteaseMusicPlayer-Linux/actions/workflows/release.yml)

A GTK4 / libadwaita music player for the Netease Cloud Music API.  
Fork of [gmg137/netease-cloud-music-gtk](https://github.com/gmg137/netease-cloud-music-gtk) with:

- **Discord Rich Presence** — now playing + time-synced lyrics
- **Apple Music-style fullscreen player** — blurred background, responsive layout
- **Japanese localization**
- **Time-synced lyrics** with click-to-seek

## Screenshots

> TODO: add screenshots

## Features

- Browse and play music from Netease Cloud Music
- Discover page with banners, top picks, and new albums
- Playlist management (create, edit, import)
- Search songs, albums, artists, playlists
- **Discord Rich Presence** with current track and lyrics
- **Now Playing view** — album art, blurred background, time-synced lyrics
- Japanese UI (`ja` locale)
- System tray integration
- MPRIS support (media keys, desktop integration)

## Build from source

### Dependencies

| Package | Ubuntu/Debian | Fedora | Arch |
|---------|--------------|--------|------|
| meson | `meson` | `meson` | `meson` |
| ninja | `ninja-build` | `ninja-build` | `ninja` |
| GTK4 | `libgtk-4-dev` | `gtk4-devel` | `gtk4` |
| libadwaita | `libadwaita-1-dev` | `libadwaita-devel` | `libadwaita` |
| GStreamer | `libgstreamer1.0-dev` `libgstreamer-plugins-base1.0-dev` | `gstreamer1-devel` `gstreamer1-plugins-base-devel` | `gstreamer` `gst-plugins-base` |
| Rust | `rustc cargo` | `rust cargo` | `rust cargo` |
| gettext | `gettext` | `gettext` | `gettext` |

### Build

```bash
git clone https://github.com/mirinnano/NeteaseMusicPlayer-Linux.git
cd netease-cloud-music-gtk

meson setup build --buildtype=release
ninja -C build
sudo ninja -C build install
netease-cloud-music-gtk4
```

Or use the build script:

```bash
./build-release.sh
# tarball at dist/netease-cloud-music-gtk4-x86_64.tar.gz
```

### Run with logging

```bash
RUST_LOG=info netease-cloud-music-gtk4
```

## Configuration

Settings are stored via GSettings (`com.gitee.gmg137.NeteaseCloudMusicGtk4`):

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `discord-rpc` | bool | false | Enable Discord Rich Presence |
| `desktop-lyrics` | bool | false | Enable desktop lyrics overlay |

### Enable Discord RPC

```bash
gsettings set com.gitee.gmg137.NeteaseCloudMusicGtk4 discord-rpc true
```

Or toggle in Preferences → Discord RPC.

## Download

Pre-built binaries are available from [Releases](https://github.com/mirinnano/NeteaseMusicPlayer-Linux/releases).

```bash
# Download the latest tarball
curl -L -o netease-cloud-music-gtk4.tar.gz \
  https://github.com/mirinnano/NeteaseMusicPlayer-Linux/releases/latest/download/netease-cloud-music-gtk4-x86_64.tar.gz

# Extract and run
tar xzf netease-cloud-music-gtk4.tar.gz
sudo cp -r usr/local/* /usr/local/
glib-compile-schemas /usr/local/share/glib-2.0/schemas
netease-cloud-music-gtk4
```

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).

## Acknowledgements

- [gmg137/netease-cloud-music-gtk](https://github.com/gmg137/netease-cloud-music-gtk) — original project
- [Netease Cloud Music API](https://github.com/gmg137/netease-cloud-music-api) — Rust API bindings
