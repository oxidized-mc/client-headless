# Protocol Packets — HeadlessCraft

Reference for Minecraft Java Edition 26.1 protocol packets (version 775).

> This document will be populated as packets are implemented.
> See [wiki.vg](https://wiki.vg/Protocol) for the community protocol documentation.

## Connection States

| State | Description |
|-------|-------------|
| Handshaking | Initial connection, client declares intent |
| Status | Server list ping |
| Login | Authentication and encryption |
| Configuration | Registry sync, resource packs |
| Play | Main game state |

## Packet Direction

- **Serverbound (C→S)** — Sent by the client (HeadlessCraft) to the server
- **Clientbound (S→C)** — Sent by the server to the client (HeadlessCraft)
