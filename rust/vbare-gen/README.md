# vbare-gen

Code generator for VBARE schemas, used from build scripts through `vbare-compiler`.

## Credits

The schema grammar and parser in `src/bare_proc/` are vendored from
[`bare_proc`](https://git.sr.ht/~chiefnoah/bare_proc) 0.2.0 by `~chiefnoah`, licensed under
the MIT license (see `LICENSE-bare_proc`), with the changes in
`vendor/bare_proc/patches/`.

## Updating the vendored parser

`src/bare_proc/` and `LICENSE-bare_proc` are generated. Never edit them by hand. To change the
parser or grammar, edit an extracted copy and rewrite the patch:

```bash
cd rust/vbare-gen
./vendor/bare_proc/regenerate.py --extract /tmp/bare_proc-edit
# edit /tmp/bare_proc-edit/parser.rs and /tmp/bare_proc-edit/grammar.pest
./vendor/bare_proc/regenerate.py --write-patch /tmp/bare_proc-edit
./vendor/bare_proc/regenerate.py
```

The script downloads the pinned `bare_proc` release from crates.io and verifies its SHA-256.
Pass `--archive path/to/bare_proc-0.2.0.crate` to work offline. Rerunning it on a clean
checkout must produce no diff.
