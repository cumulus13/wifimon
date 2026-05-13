# wifimon

[![CI](https://github.com/cumulus13/wifimon/actions/workflows/ci.yml/badge.svg?branch=master)](https://github.com/cumulus13/wifimon/actions/workflows/ci.yml)
[![Release](https://github.com/cumulus13/wifimon/actions/workflows/release.yml/badge.svg?branch=master)](https://github.com/cumulus13/wifimon/actions/workflows/release.yml)
[![Crates.io](https://img.shields.io/crates/v/wifimon.svg)](https://crates.io/crates/wifimon)
[![codecov](https://codecov.io/gh/cumulus13/wifimon/branch/main/graph/badge.svg)](https://codecov.io/gh/cumulus13/wifimon)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![License: APACHE-2](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE-APACHE)

**Professional, robust Wi-Fi monitoring CLI with [Growl/GNTP](http://growl.info/) desktop notifications.**

No PowerShell. No WMI. No COM. Pure native OS APIs on every platform.

---

## Platform backends

| OS | Backend | No PowerShell/WMI? |
|---|---|---|
| **Linux** (primary) | `iw` subprocess + `/proc/net/wireless` | ✅ |
| **macOS** | `airport` private CLI (CoreWLAN) | ✅ |
| **Windows** | Win32 WLAN API (`wlanapi.dll`) directly via `windows` crate | ✅ |

On Windows specifically: `WlanOpenHandle` → `WlanEnumInterfaces` → `WlanScan` →
`WlanGetNetworkBssList` → `WlanFreeMemory` → `WlanCloseHandle`. All direct
DLL calls, zero PowerShell, zero WMI, zero COM.

---

## Features

- 🔍 New network detection (session-based + persistent across restarts)
- 📉 Per-AP signal change tracking with configurable dBm threshold
- ❌ Lost network alerts
- 🔔 Growl/GNTP notifications with custom icon (`wifimon.png`)
- 📡 Multi-interface monitoring (`-i wlan0 -i wlan1 …`)
- 💾 Optional persistent state file (JSON, atomic writes)
- 📤 JSON output mode (`--json`) for scripting
- 🎨 Signal-coloured table output; `NO_COLOR` respected

---

## Installation

```bash
# From crates.io
cargo install wifimon

# One-liner (Linux/macOS)
curl -sSf https://raw.githubusercontent.com/cumulus13/wifimon/main/install.sh | bash

# From source
git clone https://github.com/cumulus13/wifimon && cd wifimon
cargo build --release
```

---

## Usage

```
wifimon [OPTIONS] [COMMAND]

Commands:
  list     List wireless interfaces and exit
  scan     One-shot scan, print results, and exit
  version  Print version

Options:
  -i, --interface <IFACE>        Interface(s) to monitor [repeatable]
  -n, --interval <SECS>          Scan interval [default: 10]
      --growl-host <HOST>        Growl host [default: 127.0.0.1]
      --growl-port <PORT>        Growl port [default: 23053]
      --growl-password <PASS>    Growl password
      --icon <FILE>              Notification icon PNG
      --signal-threshold <DBM>   Min |dBm| change to notify [default: 5]
      --notify-lost              Alert on lost networks [default: true]
      --notify-new               Alert on new networks  [default: true]
      --notify-signal            Alert on signal change [default: true]
      --state-file <FILE>        Persist known-AP state to JSON
      --log-file <FILE>          Write logs to file
  -v, --verbose                  -v = DEBUG, -vv = TRACE
  -q, --quiet                    Suppress all output except errors
      --json                     JSON output (for scripting)
      --no-color                 Disable colour
```

### Examples

```bash
wifimon                              # monitor all interfaces, 10 s interval
wifimon -i wlan0 -n 5               # monitor wlan0 every 5 s
wifimon -i wlan0 -i wlan1           # monitor two interfaces simultaneously
wifimon --growl-host 192.168.1.100  # send notifications to remote Growl
wifimon scan                        # one-shot scan, print table, exit
wifimon scan -i wlan0 --json        # one-shot scan as JSON
wifimon list                        # list wireless interfaces
wifimon --state-file ~/.local/share/wifimon/state.json   # persist state
```

---

## Linux notes

Scanning requires `CAP_NET_ADMIN` or root:

```bash
# Option 1: run as root
sudo wifimon

# Option 2: grant capability to the binary (run as normal user afterwards)
sudo setcap cap_net_admin+eip $(which wifimon)
wifimon
```

---

## Environment variables

| Variable | Description |
|---|---|
| `WIFIMON_GROWL_HOST` | Override `--growl-host` |
| `WIFIMON_GROWL_PORT` | Override `--growl-port` |
| `WIFIMON_GROWL_PASS` | Override `--growl-password` |
| `NO_COLOR` | Disable coloured output |
| `RUST_LOG` | Fine-grained log filter (e.g. `wifimon=debug`) |

---

## Secrets required for CI/CD

| Secret | Where to get it |
|---|---|
| `CRATES_TOKEN` | crates.io → Account Settings → API Tokens |
| `CODECOV_TOKEN` | codecov.io → your repo → Settings → General |

---

## 👤 Author
        
[Hadi Cahyadi](mailto:cumulus13@gmail.com)
    

[![Buy Me a Coffee](https://www.buymeacoffee.com/assets/img/custom_images/orange_img.png)](https://www.buymeacoffee.com/cumulus13)

[![Donate via Ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/cumulus13)
 
[Support me on Patreon](https://www.patreon.com/cumulus13)

## License

[MIT](LICENSE) © [Hadi Cahyadi](https://github.com/cumulus13)
[APACHE 2](LICENSE-APACHE) © [Hadi Cahyadi](https://github.com/cumulus13)

