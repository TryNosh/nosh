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
