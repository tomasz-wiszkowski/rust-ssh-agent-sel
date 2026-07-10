# ssh-agent-sel

A small daemon that provides a **stable SSH agent socket** at `~/.ssh/ssh-agent.sock`, regardless of which agent is actually active.

Each terminal session registers its `SSH_AUTH_SOCK` with the daemon. The daemon maintains a LIFO stack of registered sockets and always forwards traffic to the most recently registered, currently-alive one. When that socket disappears the daemon falls back to the previous entry automatically.

No SSH agent protocol is parsed or interpreted — the daemon forwards raw bytes only and is compatible with any agent, including those using custom protocol extensions.

---

## Why

SSH agent forwarding (`ssh -A`) assigns a new socket path on every connection. Tools that need a consistent `SSH_AUTH_SOCK` (tmux, screen, long-running processes) break when the path changes between sessions or after reconnecting.

`ssh-agent-sel` solves this by giving every consumer a single, permanent socket path while transparently switching the upstream agent as sessions come and go.

---

## Installation

```sh
cargo install --path .
```

The binary is named `ssh-agent-sel`.

---

## Usage

### Shell setup

Add to your shell initialisation file (`.zshrc`, `.bashrc`, etc.):

```sh
# Start the daemon once (no-op if already running).
ssh-agent-sel --daemon

# Register the current session's agent and point SSH at the stable socket.
eval "$(ssh-agent-sel)"
```

`eval "$(ssh-agent-sel)"` prints and evaluates:

```sh
export SSH_AUTH_SOCK=/home/you/.ssh/ssh-agent.sock
```

After that, all SSH clients in the session use `~/.ssh/ssh-agent.sock` and the daemon handles forwarding.

### Remote machines with agent forwarding

On the remote machine add the same snippet to your shell rc. When you `ssh -A remote`:

1. `ssh-agent-sel --daemon` starts the daemon if it is not running.
2. `eval "$(ssh-agent-sel)"` registers the forwarded socket (the value SSH placed in `SSH_AUTH_SOCK`) with the running daemon.
3. Every SSH client on the remote machine reaches your local keys through `~/.ssh/ssh-agent.sock`.

When you disconnect, the registration is removed automatically and the daemon falls back to any previously registered agent.

---

## How it works

### Files

| Path | Purpose |
|---|---|
| `~/.ssh/ssh-agent.ctrl` | Control socket (daemon ↔ client processes) |
| `~/.ssh/ssh-agent.sock` | Stable forwarding socket (consumed by SSH clients) |
| `~/.ssh/ssh-agent.log`  | Daemon log file |

### Mode detection

The binary detects its own role at startup:

| Condition | Role |
|---|---|
| `--daemon` flag, daemon already running | Exit (idempotent) |
| `--daemon` flag, no daemon | Start daemon in background, exit |
| No `--daemon`, ctrl socket connectable | Client — register `SSH_AUTH_SOCK` |
| No `--daemon`, ctrl socket absent/stale | Foreground daemon (for launchd / systemd) |

### Control protocol

A simple line-based text protocol over the control socket:

```
PING\n          →  PONG\n
REGISTER /path\n →  OK\n   (then connection held for session lifetime)
                    ERR <reason>\n
```

Registration holds the connection open. When the holding process exits (e.g. the SSH session closes), the connection drops and the daemon removes the entry from the stack automatically.

### Socket stack

Sockets are stored in a LIFO stack. Each entry is assigned a numeric ID on registration (visible in logs as `[1]`, `[2]`, …).

The top-of-stack socket receives all forwarded traffic. Dead entries are removed:

- **Passively** — when a forwarding attempt fails to connect.
- **Proactively** — by a background monitor that checks file existence every 10 seconds.

### Loop prevention

The daemon rejects any attempt to register its own forwarding socket (`~/.ssh/ssh-agent.sock`) as an upstream, preventing infinite forwarding loops.

---

## Logging

Logs are written to `~/.ssh/ssh-agent.log` and to stderr. The default level is `info`. Override with `--log-level` or the `RUST_LOG` environment variable.

Typical log output:

```
INFO daemon: daemon started
INFO ctrl:   socket [1] connected as "/tmp/ssh-XXX/agent.123"
INFO agent:  forwarding to socket [1]
INFO ctrl:   socket [1] disconnected
INFO ctrl:   socket [2] connected as "/tmp/ssh-YYY/agent.456"
INFO agent:  forwarding to socket [2]
INFO stack:  socket [2] gone; removing
WARN agent:  no live upstream agent; closing incoming connection
INFO daemon: received SIGTERM; shutting down
INFO daemon: daemon stopped
```

---

## Design principles

See [PRINCIPLES.md](PRINCIPLES.md) for the full rationale behind the design decisions.
