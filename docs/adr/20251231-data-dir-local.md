# Store data directory locally for MVP

Date: 2025-12-31
Status: Accepted

## Context
We need a data directory for session state, caches, logs, and exports. For MVP speed, we want the simplest setup.

## Decision
Store the data directory under the project local folder for dev-only; switch to OS-specific dirs later.

## Rationale
- Simple local setup for development.
- Easy to inspect and reset during iteration.

## Consequences
- Must ensure local data is excluded from git.
- We will switch to OS-specific paths later (XDG/AppData/macOS).
