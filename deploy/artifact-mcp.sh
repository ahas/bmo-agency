#!/bin/sh
# The artifact harness plugin, started from the settings folder its service deploys with. The chat's
# standard input and output are the MCP session's; only the process's working folder changes.
cd "$HOME/.local/share/bmo-artifact/deploy" || exit 1
exec bmo-artifact mcp --address "$1"
