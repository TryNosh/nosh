# Local dependency patches

`brush-core/` contains the published `brush-core` 0.5.0 source from crates.io,
under its included MIT license. The workspace's `[patch.crates-io]` entry makes
both Nosh and `brush-builtins` use this copy.

## Alias word expansion

Upstream splits alias values on ASCII whitespace and inserts them as literal
arguments. For example, SUSE's `alias ls='/usr/bin/ls $LS_OPTIONS'` passes the
literal filename `$LS_OPTIONS` to `ls`.

The local change in `src/interp.rs` tokenizes alias values with Brush's tokenizer
and applies normal word expansion to each token at invocation time. This keeps
quoting, variable expansion, field splitting, and command substitution intact.
It also respects `expand_aliases`, avoids expanding quoted or escaped command
names, and handles empty alias values without indexing an empty argument list.

This is a limited patch to the existing execution-time alias implementation.
It does not implement aliases containing shell operators or recursive alias
resolution. Upstream tracks full parse-time alias resolution at
<https://github.com/reubeno/brush/issues/57>.

Regression tests live in `src/exec/shell.rs` in the Nosh crate. When upgrading
Brush, compare this copy against the published crate and retain the patch until
those tests pass with the upstream implementation.

## Bounded embedded operations

The local `process_scope` module owns process groups and shell-created Tokio
subtasks for completion and captured AI commands. Scope cancellation terminates
those groups and aborts subtasks; completed children unregister promptly.
`commands.rs` and `interp.rs` propagate scope ownership into substitutions and
background shell tasks. Interpreter command-list boundaries yield cooperatively
so even a completion function containing a builtin-only loop can time out.
`sys/tokio_process.rs` isolates scoped external commands into process groups and
uses kill-on-drop.
Scoped command substitutions also cap pipe output before collecting completion
results, closing capped readers before waiting for their writers.

`shell.rs` provides operation snapshots that preserve shell depth and commit
successful operations while retaining the original shell's running jobs. An
interrupted operation is discarded so unfinished function frames cannot corrupt
the interactive session. The `bashdefault` completion option in `completion.rs`
now invokes basic completion instead of silently producing no fallback matches.

The original alias patch remains confined to `interp.rs`. Run the Nosh workspace
tests and `python3 tests/terminal.py target/debug/nosh` after updating this copy.
