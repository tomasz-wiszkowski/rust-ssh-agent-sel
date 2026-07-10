# ssh-agent-sel: Design Principles

## 1. No Protocol Interpretation

The daemon forwards raw bytes between the SSH client and the upstream agent. It never reads, parses, modifies, or inspects SSH agent protocol messages. This ensures compatibility with any agent, including those using proprietary or future protocol extensions.

## 2. LIFO Socket Stack

Registered agent sockets are maintained in a last-in, first-out stack. The most recently registered socket receives all forwarded traffic. When that socket becomes unavailable, the daemon falls back to the previous entry, continuing down the stack until a live socket is found or the stack is exhausted.

## 3. Session-Lifetime via Connection Hold

A client registers its `SSH_AUTH_SOCK` by establishing a persistent connection to the control socket. The daemon treats the connection lifetime as the registration lifetime: when the connection closes (because the session's shell process exited), the daemon automatically removes the corresponding stack entry. No explicit deregistration command is needed.

## 4. Passive + Proactive Validation

Sockets are validated in two ways:

- **Passive (on use):** When forwarding a connection, the daemon attempts to connect to the top-of-stack socket. If the attempt fails, the entry is removed and the next entry is tried.
- **Proactive (background monitor):** A background task periodically checks whether each registered socket file still exists on disk. Entries whose files are gone are pruned without waiting for a forwarding attempt.

## 5. Single Binary, Auto-Detection

The binary determines its own mode at startup by testing whether the control socket is connectable:

- **Connectable:** act as a client — register the current `SSH_AUTH_SOCK` with the daemon.
- **Not connectable (or absent):** act as the daemon — bind both sockets and begin serving.

Stale control socket files (left by a crashed daemon) are detected when a connection attempt fails and are cleaned up before the new daemon binds.

## 6. Graceful Degradation

If no valid upstream socket is available when an SSH client connects to `~/.ssh/ssh-agent.sock`, the daemon closes the connection cleanly. The SSH client interprets this as "agent unavailable" and falls back to password authentication or fails explicitly — no daemon crash, no hang.
