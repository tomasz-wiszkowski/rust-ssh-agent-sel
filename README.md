# ssh-agent-sel

`ssh-agent-sel` is a lightweight daemon that presents a single, permanent SSH
agent socket at `~/.ssh/ssh-agent.sock`, regardless of how many agents are
running or how often the underlying socket paths change.

The primary motivation is SSH agent forwarding: every time you run `ssh -A
host`, OpenSSH creates a fresh forwarding socket at a path like
`/tmp/ssh-XXXXXX/agent.1234` and sets `SSH_AUTH_SOCK` to it. Any process that
captured the old value — tmux windows, screen sessions, long-running
background jobs — stops being able to reach your keys. `ssh-agent-sel` absorbs
that churn. Consumers always connect to `~/.ssh/ssh-agent.sock`; the daemon
silently switches the upstream target as sessions open and close.

> **Authorship note:** This tool was designed and implemented almost entirely
> by [Claude](https://claude.ai) (Anthropic's AI assistant) in a single
> interactive session, based on a short design brief. The human author reviewed
> and directed the work but wrote very little code directly.

---

## The problem in detail

Consider this common workflow:

```
local$ ssh -A remote          # SSH_AUTH_SOCK=/tmp/ssh-ABC/agent.1 on remote
remote$ tmux new-session      # tmux captures SSH_AUTH_SOCK at attach time
remote$ ssh -A remote         # reconnect after drop; SSH_AUTH_SOCK=/tmp/ssh-XYZ/agent.2
remote$ tmux attach           # tmux still holds /tmp/ssh-ABC/agent.1 — keys gone
```

Or with a local agent manager (1Password, Secretive, gpg-agent): each tool
writes its socket to a different path, and you need to pick one and hardcode it
everywhere.

`ssh-agent-sel` gives you one canonical path. The daemon registers every agent
you tell it about and always serves traffic from the most recently registered
live one. If that agent disappears the daemon falls back to the previous one.

---

## How it works

### Architecture

The project is a single Rust binary that plays two roles depending on context:
a **daemon** that runs persistently, and a lightweight **client** that
registers the current session's agent.

```
 your SSH session          daemon              upstream agent
 ─────────────────         ──────────────      ──────────────
 ssh, git, gpg …           ~/.ssh/             /tmp/ssh-XX/
      │                    ssh-agent.sock       agent.1234
      └──── connects ───►  (UnixStream)  ────► (UnixStream)
                                               raw bytes, both directions
```

Traffic is proxied byte-for-byte with `tokio::io::copy_bidirectional`. The
daemon never reads, parses, or modifies SSH agent protocol messages. This
makes it compatible with any agent, including those using custom protocol
extensions (1Password, hardware keys, etc.).

### Files created by the daemon

| Path | Purpose |
|---|---|
| `~/.ssh/ssh-agent.ctrl` | Control socket — daemon listens here for registrations |
| `~/.ssh/ssh-agent.sock` | Forwarding socket — SSH clients connect here |
| `~/.ssh/ssh-agent.log`  | Log file |

### Mode detection

The binary determines its own role at startup by sending `PING` to the control
socket and checking for `PONG`:

| Invocation | Daemon reachable? | Behaviour |
|---|---|---|
| `ssh-agent-sel --daemon` | Yes | Exit immediately (idempotent) |
| `ssh-agent-sel --daemon` | No  | Spawn daemon in background, then exit |
| `ssh-agent-sel` | Yes | Register `$SSH_AUTH_SOCK`, print `export …`, exit |
| `ssh-agent-sel` | No  | Run as foreground daemon (for launchd / systemd) |

Stale socket files left by a crashed daemon are detected when the connection
attempt fails and are cleaned up automatically.

### The socket stack

The daemon maintains a LIFO stack of registered sockets. Each entry is
assigned a numeric ID when it is pushed (visible in logs as `[1]`, `[2]`, …).

- **Register:** a client connects to `~/.ssh/ssh-agent.ctrl`, sends
  `REGISTER /path/to/socket`, and holds the connection open. The socket goes
  to the top of the stack.
- **Forward:** every incoming connection on `~/.ssh/ssh-agent.sock` is
  proxied to the top-of-stack socket. If that socket is unreachable the entry
  is dropped and the next one is tried.
- **Deregister:** when the holding client process exits (SSH session closes,
  terminal killed, etc.), the control connection drops and the daemon removes
  the entry from the stack automatically.
- **Proactive cleanup:** a background task checks socket file existence every
  10 seconds and removes any entries whose files have disappeared, without
  waiting for a forwarding attempt.

### Loop prevention

The daemon rejects any registration of its own forwarding socket
(`~/.ssh/ssh-agent.sock`). This prevents an infinite forwarding loop when
`SSH_AUTH_SOCK` already points at the mux socket — which happens if you run
`ssh-agent-sel` a second time in the same session without `eval`.

### Control protocol

A minimal line-based text protocol over the control Unix socket:

```
PING\n            →  PONG\n
REGISTER /path\n  →  OK\n       hold connection for session lifetime
                     ERR <reason>\n
```

---

## Installation

**Prerequisites:** Rust toolchain (stable, edition 2024 requires Rust ≥ 1.85).

```sh
git clone <repo>
cd rust-ssh-agent-selector
cargo build --release
# copy the binary somewhere on your PATH, e.g.:
install -m 755 target/release/ssh-agent-sel ~/.local/bin/
```

Or install directly with Cargo:

```sh
cargo install --path .
```

---

## Recommended usage

### Shell initialisation (`.zshrc` / `.bashrc`)

```sh
# Ensure the daemon is running (no-op if already up).
ssh-agent-sel --daemon

# Register this session's agent and switch SSH_AUTH_SOCK to the stable path.
eval "$(ssh-agent-sel)"
```

`eval "$(ssh-agent-sel)"` outputs and evaluates:

```sh
export SSH_AUTH_SOCK=/home/you/.ssh/ssh-agent.sock
```

From that point on every tool in the session — `ssh`, `git`, `gpg-agent`,
`scp`, etc. — uses the stable socket. When the session ends the registration
is cleaned up automatically.

### Remote machines with agent forwarding

Place the same two lines in your shell rc on the **remote** machine. Then:

```sh
local$ ssh -A remote
# On remote, .zshrc runs:
#   ssh-agent-sel --daemon  →  daemon already running, no-op
#   eval "$(ssh-agent-sel)" →  registers /tmp/ssh-XX/agent.N with the daemon
remote$ ssh-add -l          # your local keys are available
remote$ tmux attach         # tmux sessions also see ~/.ssh/ssh-agent.sock
```

When you disconnect and reconnect, the new forwarded socket replaces the old
one at the top of the stack. If you disconnect without reconnecting the entry
is removed and the daemon waits for the next registration.

### Using a local agent alongside forwarded ones

If you have a local agent running (e.g. from `ssh-agent` in your login
session), register it first:

```sh
ssh-agent-sel --daemon
eval "$(ssh-agent-sel)"   # registers the local agent
```

When you later `ssh -A` into the machine, the forwarded agent goes on top of
the stack. When you disconnect, the local agent becomes active again
automatically.

### Running under launchd (macOS)

To have the daemon managed by launchd rather than started from your shell rc:

```xml
<!-- ~/Library/LaunchAgents/com.example.ssh-agent-sel.plist -->
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
    "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>         <string>com.example.ssh-agent-sel</string>
    <key>ProgramArguments</key>
    <array>
        <string>/path/to/ssh-agent-sel</string>
    </array>
    <key>KeepAlive</key>     <true/>
    <key>RunAtLoad</key>     <true/>
    <key>StandardErrorPath</key>
    <string>/Users/you/.ssh/ssh-agent.log</string>
</dict>
</plist>
```

```sh
launchctl load ~/Library/LaunchAgents/com.example.ssh-agent-sel.plist
```

With launchd managing the daemon, omit `ssh-agent-sel --daemon` from your
shell rc and keep only `eval "$(ssh-agent-sel)"`.

---

## Logging

Logs go to both `~/.ssh/ssh-agent.log` and stderr. The default level is
`info`. To change it:

```sh
ssh-agent-sel --log-level debug   # one session
RUST_LOG=debug ssh-agent-sel      # via environment
```

Typical log output across two SSH sessions:

```
INFO daemon: daemon started
INFO ctrl:   socket [1] connected as "/tmp/ssh-ABC/agent.100"
INFO agent:  forwarding to socket [1]
INFO agent:  forwarding to socket [1]
INFO ctrl:   socket [1] disconnected
WARN agent:  no live upstream agent; closing incoming connection
INFO ctrl:   socket [2] connected as "/tmp/ssh-XYZ/agent.200"
INFO agent:  forwarding to socket [2]
INFO stack:  socket [2] gone; removing
WARN agent:  no live upstream agent; closing incoming connection
INFO daemon: received SIGTERM; shutting down
INFO daemon: daemon stopped
```

---

## Reference

### Command-line flags

| Flag | Default | Description |
|---|---|---|
| `--daemon` | — | Start daemon in background if not running; exit |
| `--socket <path>` | `$SSH_AUTH_SOCK` | Socket to register (client mode) |
| `--log-level <level>` | `info` | Log verbosity: trace, debug, info, warn, error |

### Environment variables

| Variable | Description |
|---|---|
| `SSH_AUTH_SOCK` | Socket path registered in client mode when `--socket` is not given |
| `RUST_LOG` | Overrides `--log-level` (standard `tracing-subscriber` syntax) |

---

## Design

See [PRINCIPLES.md](PRINCIPLES.md) for the rationale behind the key design
decisions: no protocol interpretation, LIFO semantics, connection-hold
lifetime tracking, passive and proactive socket validation, loop prevention,
and graceful degradation.
