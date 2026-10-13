<div align="center">

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-green.svg)](https://www.gnu.org/licenses/gpl-3.0)  

**Landscape routes traffic by domain—not just IP. Each flow gets its own DNS server.**

**DNS answers populate kernel eBPF maps. Packets are steered at XDP/TC.**

**No userspace datapath. No iptables.**

> Built with Rust / eBPF.

</div>

[简体中文](./README.zh.md) | [English](./README.md) | [Documentation](https://landscape.whileaway.dev/)

## Screenshot
![Landscape Web UI](./assets/main.png)

## Architecture

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="./assets/arch-dark.svg">
  <source media="(prefers-color-scheme: light)" srcset="./assets/arch-light.svg">
  <img alt="Landscape architecture diagram" src="./assets/arch-light.svg">
</picture>

Landscape separates traffic steering into two planes:

**DNS plane (userspace).** A flow is a policy group that devices join by IP or MAC. Each flow gets its own isolated Hickory DNS server with independent cache, upstream (UDP/DoH/DoT/DoQ), and rules. DNS answers populate per-flow eBPF maps in the kernel.

**Data plane (kernel).** XDP and TC hooks read these maps to steer packets at wire speed. Packets matching a flow are steered according to its policy. Everything else passes through directly — no userspace context switch.

DNS results → eBPF flow maps → TC/XDP in-kernel steering → interface routing

DNS plane decides. Kernel enforces.

## Core Features
* DNS-driven traffic steering via eBPF — DNS answers populate per-flow kernel maps
* Fine-grained NAT — a stricter-than-symmetric NAT policy by default, with per-domain/IP full-cone NAT exceptions ([details](https://landscape.whileaway.dev/features/nat.html))
* Per-flow DNS isolation — independent cache and upstream per flow, no cross-flow leaks
* Redirect packets matching a flow into Docker containers — extend with any TProxy-compatible program
* Geo database management — DAT and TXT format support
* Full REST API — everything in the UI is scriptable

## Why Landscape

**Standard Linux, no lock-in.** Debian, Arch, openSUSE. Your distro, your rules.

**Upgrade without fear.** Single directory. Drop in a new binary, config auto-migrates. Downgrade works too.

**NAT that fits your LAN.** Full-cone NAT for BT/PT where needed, strict NAT everywhere else — domain/IP-level control, no blanket rules.

**One failure, one victim.** Per-flow DNS and traffic policies. A container goes down? Only the traffic routed through it is affected.

## Quick Start

### Prerequisites

- Linux kernel ≥ 6.9 with BTF/BPF enabled, `root` privileges; non-Linux kernels (FreeBSD, macOS) are not supported
- Docker (optional, for container redirection)

### 1. Install

Download the release assets from [Releases](https://github.com/ThisSeanZhang/landscape/releases).

**Debian / Ubuntu (amd64, arm64)**: install the `landscape-router_<version>_<arch>.deb` package. It includes the binary, the web UI and a systemd unit:

```bash
sudo apt install ./landscape-router_<version>_<arch>.deb
sudo systemctl enable --now landscape-router.service
```

The package does not start Landscape on first install, because Landscape reconfigures the host network.

**Other distributions**: download `static.zip` and the `landscape-webserver` binary for your architecture, then install both by hand:

```bash
install -Dm755 landscape-webserver-<arch> /usr/local/bin/landscape-webserver
mkdir -p /usr/share/landscape-router
unzip static.zip -d /usr/share/landscape-router   # -> /usr/share/landscape-router/static
```

### 2. Start Landscape

If you installed by hand, run it as `root`:

```bash
landscape-webserver
```

Defaults: HTTPS on port `6443`, user/pass `root` / `root`. Landscape uses these directories, and creates them when missing:

| Path | Contents |
|---|---|
| `/etc/landscape-router` | `landscape.toml`, `landscape_init.toml` |
| `/var/lib/landscape-router` | database, init lock, metrics, geo data |
| `/var/log/landscape-router` | logs |
| `/usr/share/landscape-router/static` | web UI |
| `/run/landscape-router` | container registration socket, API token, hostapd configs |

`--config_dir <dir>` (or `LANDSCAPE_CONF_PATH`) puts all of these under one directory instead. `--web`, `--log_path` and `--db_url` override individual paths.

**Upgrading from an older release:** if `/root/.landscape-router` exists, Landscape keeps using it, and logs a warning that explains how to move to the layout above. A web UI installed in `/usr/share/landscape-router/static` takes precedence over `/root/.landscape-router/static`.

Landscape can start directly without any pre-created configuration file. If you want to initialize it through `landscape_init.toml`, see the configuration guide on the documentation site.

You can check more options with `landscape-webserver --help`.

### 3. Open the management interface

- `http://landscape.local:6300` automatically redirects to HTTPS
- `https://landscape.local:6443` opens the Web UI
- `https://landscape.local:6443/api/docs` opens the REST API docs

## Run as a systemd Service

The `.deb` package installs `landscape-router.service`. For a manual install, after confirming that Landscape runs correctly, save this as `/etc/systemd/system/landscape-router.service`:

```ini
[Unit]
Description=Landscape Router

[Service]
ExecStart=/usr/local/bin/landscape-webserver
Restart=always
User=root
LimitMEMLOCK=infinity

[Install]
WantedBy=multi-user.target
```

Then run `systemctl enable --now landscape-router.service`.

## Development

Build guide: [BUILD.md](./BUILD.md) | [BUILD.zh.md](./BUILD.zh.md)

## License

- `landscape-ebpf`: [GNU General Public License v2.0](https://www.gnu.org/licenses/old-licenses/gpl-2.0.html)
- Other parts: [GNU General Public License v3.0](https://www.gnu.org/licenses/gpl-3.0.html)

If you have suggestions or run into a problem, please open an [issue](https://github.com/ThisSeanZhang/landscape/issues).
