# jdtui — notes for Claude Code

## Serena first (mandatory for code work)

This repo is set up for the Serena MCP server (`.mcp.json`, `.serena/`). For any task that reads,
explores or changes the Rust code:

1. Load the Serena tools (they are deferred: `ToolSearch` with `+serena`) and call
   `mcp__serena__initial_instructions` once per session.
2. Read the memory `core` (the root of the memory graph), then the ones it points to that the task
   touches (`conventions`, `api_quirks`, `release_procedure`, `task_completion`, …).
3. Navigate and edit by symbol: `get_symbols_overview`, `find_symbol`, `find_referencing_symbols`,
   `replace_symbol_body`, `insert_after_symbol`, `rename_symbol`. Use grep/Read only for non-code
   files or when a symbolic lookup cannot answer.
4. Before declaring a task done, follow the memory `task_completion`; check
   `get_diagnostics_for_file` on the files you changed.
5. When a change makes a memory stale, update it (`edit_memory`), as `memory_maintenance` says.

Questions that do not touch the code (git, release status, config) do not need Serena.
