# Implementation Phases — HeadlessCraft

This document outlines the implementation roadmap for HeadlessCraft.

Phases are ordered by dependency — each phase builds on the previous ones.

## Phases

| Phase | Title | Status |
|-------|-------|--------|
| 1 | Bootstrap & Project Setup | ✅ Complete |
| 2 | TCP Connection & Framing | Planned |
| 3 | Handshake & Status | Planned |
| 4 | Login & Authentication | Planned |
| 5 | NBT Library | Planned |
| 6 | Configuration Phase | Planned |
| 7 | Core Types | Planned |
| 8 | Play Phase Entry | Planned |
| 9 | Chunk Receiving & Storage | Planned |
| 10 | Entity Tracking | Planned |
| 11 | Player Movement | Planned |
| 12 | Chat | Planned |
| 13 | Bot API & Event System | Planned |
| 14 | Inventory Management | Planned |
| 15 | Block Interaction | Planned |
| 16 | Combat | Planned |
| 17 | Pathfinding | Planned |
| 18 | Multi-Client Support | Planned |

## Phase Dependencies

```
Phase 1 (Bootstrap)
  └─ Phase 2 (TCP)
       └─ Phase 3 (Handshake/Status)
            └─ Phase 4 (Login/Auth)
                 ├─ Phase 5 (NBT)
                 └─ Phase 6 (Configuration)
                      └─ Phase 8 (Play Entry)
                           ├─ Phase 9 (Chunks)
                           ├─ Phase 10 (Entities)
                           ├─ Phase 11 (Movement)
                           └─ Phase 12 (Chat)
                                └─ Phase 13 (Bot API)
                                     ├─ Phase 14 (Inventory)
                                     ├─ Phase 15 (Block Interaction)
                                     ├─ Phase 16 (Combat)
                                     └─ Phase 17 (Pathfinding)
                                          └─ Phase 18 (Multi-Client)
```
