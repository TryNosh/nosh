# AI Context and Agentic Mode

## Overview

Enhance nosh's AI capabilities with two features:
1. **Session context** - Send recent conversation history to AI for referential understanding
2. **Agentic mode** - AI can execute commands iteratively to gather information before responding

## Current State

The AI is stateless - each translation is independent:
```
User: "find large files"     → AI returns: find . -size +100M
User: "now delete them"      → AI has no idea what "them" means
```

## Feature 1: Session Context

### Goal

Enable referential queries by sending conversation history to the AI.

### Design

Store recent exchanges in memory (not persisted):

```rust
struct ConversationContext {
    exchanges: VecDeque<Exchange>,
    max_exchanges: usize,  // Default: 10
}

struct Exchange {
    user_input: String,      // "find large files"
    ai_command: String,      // "find . -size +100M"
    output_summary: Option<String>,  // First 500 chars of output (optional)
}
```

### Prompt Format

```
Previous conversation:
User: find large files
Command: find . -size +100M

User: show me the rust ones
Command: find . -size +100M -name "*.rs"

Current request: now delete them
```

### Configuration

```toml
[ai]
context_size = 10          # Number of exchanges to remember
include_output = false     # Include command output in context (uses more tokens)
```

### Implementation

1. Add `ConversationContext` struct to track exchanges
2. Modify `translate()` to accept context and format into prompt
3. After each AI command executes, record the exchange
4. Clear context on `/clear` or new session

## Feature 2: Agentic Mode

### Goal

Allow AI to run commands iteratively, gathering information before providing a final answer.

### Trigger

Questions that need investigation:
```
$ ? what's eating my disk space
$ ? why is this build failing
$ ? what ports are in use
```

vs. direct translations (current behavior):
```
$ ? list files        → ls
$ ? show git status   → git status
```

### Design

Use Mistral's function calling with Devstral models:

```rust
struct AgenticSession {
    max_iterations: usize,      // Default: 10
    timeout_seconds: u64,       // Default: 60
    permissions: PermissionStore,
}

// Tool definition sent to AI
struct RunCommandTool {
    name: "run_command",
    description: "Execute a shell command and return output",
    parameters: {
        command: String,
    }
}
```

### Flow

```
1. User asks: "what's eating my disk space?"

2. Send to AI with tool definition

3. AI responds with tool call:
   { "name": "run_command", "arguments": { "command": "du -sh /* 2>/dev/null | sort -hr | head -10" } }

4. Check permissions:
   - If `du` allowed → execute, capture output
   - If not → prompt user with existing permission UI

5. Send output back to AI

6. AI may call another tool or respond with final answer

7. Repeat until:
   - AI sends text response (done)
   - Max iterations reached
   - Timeout exceeded
   - User cancels (Ctrl+C)
```

### Permission Integration

Reuse existing safety system completely:
- AI-requested commands go through same permission checking
- User sees same prompt: `[O]nce [S]ession [A]lways [D]eny`
- Permission store learns what user is comfortable with AI running
- No new configuration needed

### Output Display

```
$ ? what's eating my disk space

[AI] Running: du -sh /* | sort -hr | head -5
     45G    /Users
     12G    /Library
     8G     /System
     ...

[AI] Running: du -sh /Users/pouya/* | sort -hr | head -5
     32G    /Users/pouya/Library
     8G     /Users/pouya/Projects
     ...

Your /Users/pouya/Library folder is using 32GB. The largest items are:
- Caches: 12GB (safe to clear with: rm -rf ~/Library/Caches/*)
- Application Support: 8GB
- ...
```

### Configuration

```toml
[ai]
agentic_enabled = true     # Enable agentic mode (default: true)
agentic_model = "devstral-small"  # Model for agentic queries
max_iterations = 10        # Max command executions per query
timeout = 60               # Seconds before giving up
```

### Safety Limits

1. **Iteration limit** - Stop after N commands (default: 10)
2. **Timeout** - Stop after N seconds (default: 60)
3. **Token budget** - Track tokens used, warn if expensive
4. **Permission gate** - Every command goes through safety system
5. **User cancel** - Ctrl+C stops the loop

## API Changes

### Ollama

Current `/api/generate` doesn't support tools. Options:
1. Use `/api/chat` with tools (if model supports it)
2. Fallback to non-agentic for Ollama
3. Simulate with structured prompting (less reliable)

Recommendation: Agentic mode requires cloud API with Devstral. Ollama stays translation-only.

### Cloud API

Add new endpoint or parameter:
```
POST /api/ai/complete
{
  "input": "what's eating my disk space",
  "cwd": "/Users/pouya/Projects",
  "mode": "agentic",  // or "translate"
  "context": [...],   // previous exchanges
  "tool_result": {    // if continuing agentic loop
    "output": "..."
  }
}
```

## Implementation Plan

### Phase 1: Session Context
1. Add `ConversationContext` struct
2. Modify `OllamaClient::translate()` to accept context
3. Modify `CloudClient` to accept context
4. Update main loop to track exchanges
5. Add `/clear` to reset context

### Phase 2: Agentic Mode (Cloud Only)
1. Add agentic config options
2. Create `AgenticSession` struct with iteration loop
3. Update cloud API to handle tool calling
4. Add Devstral model support on server
5. Integrate with existing permission system
6. Add output display formatting

### Phase 3: Polish
1. Add token usage tracking/display
2. Add progress indicators during agentic queries
3. Handle edge cases (network errors, model errors)
4. Add `/agent on|off` toggle command

## Open Questions

1. **Output in context**: Should we include command output in context? Pros: better understanding. Cons: token usage.

2. **Agentic trigger**: How does AI decide translate vs. agentic? Options:
   - User prefix (e.g., `?? what's wrong` for agentic)
   - AI decides based on query type
   - Always try agentic, fallback to translate

3. **Local agentic**: Should we try to make Ollama work with agentic via structured prompting? Or keep it cloud-only?

## Non-Goals

- Persistent memory ("this is my projects folder") - separate feature
- File editing capabilities - too dangerous
- Background execution - commands run synchronously
