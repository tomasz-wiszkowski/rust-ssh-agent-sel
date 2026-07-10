# Architecture

## Files

| Path | Purpose |
|---|---|
| `~/.ssh/ssh-agent.ctrl` | Control socket — daemon listens for registrations |
| `~/.ssh/ssh-agent.sock` | Forwarding socket — consumed by SSH clients |
| `~/.ssh/ssh-agent.log`  | Log file |

---

## Mode detection

The binary sends `PING` to the control socket at startup and checks for `PONG`
to determine its role:

| Invocation | Daemon reachable? | Behaviour |
|---|---|---|
| `--daemon` | Yes | Exit (idempotent) |
| `--daemon` | No  | Spawn `_run_daemon` in background, exit |
| _(none)_ | Yes | Client: register `$SSH_AUTH_SOCK`, print `export …`, exit |
| _(none)_ | No  | Foreground daemon (for launchd / systemd) |

Stale socket files from a crashed daemon are detected when the connection
attempt fails and removed before the new daemon binds.

---

## Socket stack

Registered sockets are kept in a LIFO stack. Each entry gets a numeric ID on
push, visible in logs as `[1]`, `[2]`, etc.

The top-of-stack socket receives all forwarded traffic. Dead entries are
removed in two ways:

- **Passively** — when a forwarding attempt fails to connect.
- **Proactively** — a background task checks file existence every 10 seconds.

---

## Forwarding

```
SSH client  ──►  ~/.ssh/ssh-agent.sock  ──►  top-of-stack socket  ──►  agent
```

The daemon connects the two sides with `tokio::io::copy_bidirectional` and
forwards raw bytes in both directions. No SSH agent protocol framing is read
or interpreted. This is what makes the tool compatible with any agent,
including those using non-standard protocol extensions.

If the top-of-stack socket is unreachable the entry is dropped and the next
one is tried. If the stack is empty the incoming connection is closed cleanly.

---

## Control protocol

Line-based text over the control Unix socket:

```
PING\n            →  PONG\n
REGISTER /path\n  →  OK\n
                     ERR <reason>\n
```

A `REGISTER` command holds its connection open for the lifetime of the
registration. When the holding process exits the connection drops and the
daemon removes the entry. No explicit deregistration command exists.

---

## Loop prevention

The daemon rejects `REGISTER` if the supplied path matches its own forwarding
socket (`~/.ssh/ssh-agent.sock`). This prevents a forwarding loop when
`SSH_AUTH_SOCK` already points at the mux socket.

---

## Source layout

```
src/
├── main.rs     — startup, mode detection, logging init
├── cli.rs      — clap argument definitions
├── paths.rs    — ~/.ssh/* path helpers
├── stack.rs    — AgentStack (LIFO, numeric IDs)
├── daemon.rs   — socket binding, task orchestration, shutdown
├── ctrl.rs     — control socket accept loop and REGISTER handler
├── agent.rs    — forwarding socket accept loop and byte proxy
├── monitor.rs  — background liveness monitor
├── client.rs   — client mode: spawn holder, print export line
└── hold.rs     — internal: hold ctrl connection for session lifetime
```
