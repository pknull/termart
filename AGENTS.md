# AI Assistant Context

This project uses the **Asha** framework for session coordination and memory persistence.

## Framework

Asha plugin provides operational protocols via session hooks. Context is automatically injected when `.asha/config.json` exists.

## Memory Bank

Project context stored in `Memory/*.md` files:
- `activeContext.md` - Current project state
- `projectbrief.md` - Project scope and objectives
- `workflowProtocols.md` - Validated patterns

## Session Workflow

1. Read `Memory/activeContext.md` for context
2. Follow existing patterns in the codebase
3. Use authority markers when uncertain: `[Inference]`, `[Speculation]`, `[Unverified]`

