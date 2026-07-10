# ssh-agent-sel

A daemon that exposes a single, stable SSH agent socket at `~/.ssh/ssh-agent.sock`,
regardless of which agent is active or how often the underlying socket path changes.

SSH agent forwarding (`ssh -A`) creates a new socket at a fresh path on every
connection. Anything that captured the old path — tmux windows, screen sessions,
long-running jobs — loses access to your keys. `ssh-agent-sel` fixes this with a
permanent socket that silently switches its upstream target as sessions come and go.

**Protocol transparency:** the daemon forwards raw bytes and never reads, parses, or
modifies SSH agent protocol messages. It is compatible with any agent, including those
using vendor-specific or custom protocol extensions.

> This tool was designed and implemented almost entirely by
> [Claude](https://claude.ai) (Anthropic), based on a short design brief.

---

## Installation

Requires Rust ≥ 1.85 (edition 2024).

```sh
cargo install --path .
```

---

## Setup

Add to your shell rc (`.zshrc`, `.bashrc`, …):

```sh
ssh-agent-sel --daemon      # start daemon in background if not already running
eval "$(ssh-agent-sel)"     # register this session's agent; set SSH_AUTH_SOCK
```

`eval "$(ssh-agent-sel)"` exports:

```sh
export SSH_AUTH_SOCK=/home/you/.ssh/ssh-agent.sock
```

### Remote machines

The same two lines work on remote machines. When you `ssh -A remote`:

1. `--daemon` starts the daemon if needed (no-op otherwise).
2. `eval "$(ssh-agent-sel)"` registers the forwarded socket with the running daemon.
3. All SSH clients on the remote — including tmux panes — use `~/.ssh/ssh-agent.sock`.

On disconnect, the registration is removed and the daemon falls back to any
previously registered agent.

---

## Options

| Flag | Default | Description |
|---|---|---|
| `--daemon` | — | Start daemon in background if not running; exit immediately |
| `--socket <path>` | `$SSH_AUTH_SOCK` | Socket path to register (client mode) |
| `--log-level <level>` | `warn` | trace / debug / info / warn / error |

`RUST_LOG` overrides `--log-level`.

---

## Logs

Written to `~/.ssh/ssh-agent.log` and stderr.

```
INFO ctrl:   socket [1] connected as "/tmp/ssh-ABC/agent.100"
INFO agent:  forwarding to socket [1]
INFO ctrl:   socket [1] disconnected
INFO ctrl:   socket [2] connected as "/tmp/ssh-XYZ/agent.200"
INFO agent:  forwarding to socket [2]
INFO stack:  socket [2] gone; removing
WARN agent:  no live upstream agent; closing incoming connection
```

---

## Further reading

- [ARCHITECTURE.md](ARCHITECTURE.md) — how the daemon works internally
- [PRINCIPLES.md](PRINCIPLES.md) — design rationale
