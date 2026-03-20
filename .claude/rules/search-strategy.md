# Search Strategy — RTK Codebase Navigation

Efficient search patterns for RTK's Rust codebase.

## Priority Order

1. **LSP** (semantic, instant) → for symbols, definitions, references, types
2. **Grep** (exact pattern, fast) → for strings, comments, config values
3. **Glob** (file discovery) → for finding modules by name pattern
4. **Read** (full file) → only after locating the right file
5. **Explore agent** (broad research) → last resort for >3 queries

Never use Bash for search (`find`, `grep`, `rg`) — use dedicated tools.

## LSP vs Grep — Decision Table

| Question | Use |
|----------|-----|
| Where is function `filter_git_log` defined? | LSP `goToDefinition` |
| Where is struct `RtkConfig` defined? | LSP `goToDefinition` |
| What type does `execute_command` return? | LSP `hover` |
| Who calls `track_command()`? | LSP `findReferences` |
| What symbols are in `src/git.rs`? | LSP `documentSymbol` |
| Where is trait `Filter` implemented? | LSP `goToImplementation` |
| What calls into `filter_output()`? | LSP `incomingCalls` |
| Find all `lazy_static!` blocks | Grep (macro, not a symbol) |
| Find all `.unwrap()` usages | Grep (text pattern) |
| Find files matching `*_cmd.rs` | Glob |
| Find a comment mentioning "token savings" | Grep |
| Find all `#[cfg(test)]` sections | Grep |

**Rule of thumb**: If it's a Rust symbol (function, struct, trait, enum variant) → LSP first. If it's text/pattern (macros, comments, strings, config) → Grep.

## LSP Operations Reference

```
# Jump to definition (exact file + line, ~50ms)
LSP goToDefinition symbol="filter_git_log"
LSP goToDefinition symbol="RtkConfig"

# Find all call sites before refactoring
LSP findReferences symbol="track_command"
LSP findReferences symbol="execute_command"

# Get type info without reading the file
LSP hover symbol="strip_ansi"

# List all symbols in a file (functions, structs, enums)
LSP documentSymbol file="src/git.rs"

# Find a symbol project-wide (don't know which file)
LSP workspaceSymbol query="FilterRule"
LSP workspaceSymbol query="run"

# Find concrete implementations of a trait
LSP goToImplementation symbol="Filter"

# Trace call hierarchy
LSP incomingCalls symbol="filter_output"
LSP outgoingCalls symbol="run"
```

## RTK Module Map

```
src/
├── main.rs           ← Commands enum + routing (start here for any command)
├── git.rs            ← Git operations (log, status, diff)
├── runner.rs         ← Cargo commands (test, build, clippy, check)
├── gh_cmd.rs         ← GitHub CLI (pr, run, issue)
├── grep_cmd.rs       ← Code search output filtering
├── ls.rs             ← Directory listing
├── read.rs           ← File reading with filter levels
├── filter.rs         ← Language-aware code filtering engine
├── tracking.rs       ← SQLite token metrics
├── config.rs         ← ~/.config/rtk/config.toml
├── tee.rs            ← Raw output recovery on failure
├── utils.rs          ← strip_ansi, truncate, execute_command
├── init.rs           ← rtk init command
└── *_cmd.rs          ← All other command modules
```

## Common Search Patterns

### "Where is command X handled?"

```
# Step 1: Find the routing — LSP workspace search
LSP workspaceSymbol query="Commands"
# or Grep for the enum variant name
Grep pattern="Gh\|Cargo\|Git\|Grep" path="src/main.rs" output_mode="content"

# Step 2: Jump to the handler
LSP goToDefinition symbol="GhArgs"  # or whatever the variant struct is
```

### "Where is function X defined?"

```
# ✅ LSP (preferred — exact, instant)
LSP goToDefinition symbol="filter_git_log"

# Fallback if LSP unavailable
Grep pattern="fn filter_git_log\|fn run\b" type="rust"
```

### "Who calls function X?" (before renaming/changing signature)

```
# Always use LSP findReferences before any refactor
LSP findReferences symbol="filter_git_log"
LSP findReferences symbol="execute_command"
```

### "What type does this return / what fields does this struct have?"

```
LSP hover symbol="RtkConfig"
LSP documentSymbol file="src/config.rs"
```

### "All command modules"

```
Glob pattern="src/*_cmd.rs"
# Then: src/git.rs, src/runner.rs for non-*_cmd.rs modules
```

### "Find all lazy_static regex definitions"

```
# Grep only — lazy_static! is a macro, not a navigable symbol
Grep pattern="lazy_static!" type="rust" output_mode="content"
```

### "Find unwrap() outside tests"

```
Grep pattern="\.unwrap()" type="rust" output_mode="content"
# Then manually filter out #[cfg(test)] blocks
```

### "Which modules have tests?"

```
Grep pattern="#\[cfg\(test\)\]" type="rust" output_mode="files_with_matches"
```

### "Find token savings assertions"

```
Grep pattern="count_tokens\|savings" type="rust" output_mode="content"
```

### "Find test fixtures"

```
Glob pattern="tests/fixtures/*.txt"
```

## RTK-Specific Navigation Rules

### Adding a new filter

1. `LSP workspaceSymbol query="Commands"` → find Commands enum in `src/main.rs`
2. `LSP documentSymbol file="src/gh_cmd.rs"` → see pattern to follow
3. `LSP findReferences symbol="execute_command"` → find shared helpers before reimplementing
4. `Glob pattern="tests/fixtures/*.txt"` → find existing fixture patterns

### Debugging filter output

1. `LSP goToDefinition symbol="run"` in `src/<cmd>_cmd.rs`
2. `LSP outgoingCalls symbol="run"` → trace which filter function is called
3. `Grep pattern="lazy_static!"` in same file for regex patterns
4. `LSP goToDefinition symbol="strip_ansi"` → jump to ANSI stripping logic

### Tracking/metrics issues

1. `LSP goToDefinition symbol="track_command"` → jumps to `src/tracking.rs`
2. `LSP hover symbol="tracking_database_path"` → get field type/docs
3. `RTK_DB_PATH` env var overrides config

### Configuration issues

1. `LSP goToDefinition symbol="RtkConfig"` → jumps to `src/config.rs`
2. `LSP documentSymbol file="src/config.rs"` → list all config fields
3. `LSP goToDefinition symbol="rtk_init"` → find `rtk init` logic
4. Config file: `~/.config/rtk/config.toml`
5. Filter files: `~/.config/rtk/filters/` (global) or `.rtk/filters/` (project)

## TOML Filter DSL Navigation

```
Glob pattern=".rtk/filters/*.toml"         # Project-local filters
LSP workspaceSymbol query="FilterRule"     # Find the type definition
LSP findReferences symbol="FilterConfig"   # See all usages
```

## Anti-Patterns

❌ **Don't** Grep for a function name to find its definition — use `LSP goToDefinition`
❌ **Don't** Grep for all call sites before refactoring — use `LSP findReferences`
❌ **Don't** Read a file to find out what symbols it exports — use `LSP documentSymbol`
❌ **Don't** read all `*_cmd.rs` files to find one function — use LSP or Grep first
❌ **Don't** use Bash `find src -name "*.rs"` — use Glob
❌ **Don't** read `main.rs` entirely to find a module — Grep for the command name
❌ **Don't** search `Cargo.toml` for dependencies with Bash — use Grep with `glob="Cargo.toml"`

## Dependency Check

```
# Check if a crate is already used (before adding)
Grep pattern="^regex\|^anyhow\|^rusqlite" glob="Cargo.toml" output_mode="content"

# Check if async is creeping in (forbidden)
Grep pattern="tokio\|async-std\|futures\|async fn" type="rust"
```
