# Data Formats — HeadlessCraft

Reference for Minecraft data formats as decoded by HeadlessCraft.

> This document will be populated as formats are implemented.

## NBT (Named Binary Tag)

13 tag types used for structured data (chunk data, entity data, etc.).

## Chunk Format

Chunks are 16×16 columns of 16×16×16 sections with paletted block storage.

## VarInt / VarLong

Variable-length integer encoding used throughout the protocol.
