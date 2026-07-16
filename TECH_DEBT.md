# Technical Debt Register

Compiled 2026-07-16 during a full codebase review. Measurements taken on a
160-entity test project with a debug build. Ordered by leverage: what most
reduces code size, defect rate, and maintenance cost.

## 1. Entity-command duplication (highest leverage)

`crates/tdt-cli/src/cli/commands/` is **42.8k lines**, and 21 entity command
files (req, risk, test, cmp, asm, sup, quote, feat, mate, tol, proc, ctrl,
work, ncr, capa, lot, dev, haz, rslt, ...) each re-implement the same shape:

- 12 hand-copied `output_cached_*` functions
- 20 copies of the `can_use_cache` fast-path predicate
- 8 local `find_entity`/`find_entity_file` lookup helpers
- 14 files carrying their own entity-directory tables (the canonical source
  is `Project::entity_directory` / `risk_directory` / `test_directory`)

This duplication is where this review found whole classes of copy-drift bugs
(`-o path` panics in 22 places, wrong directory tables in bulk ops, a wrong
`QUOTE` prefix, `risk new` writing to a different directory than it printed).

**Consolidation sketch:** extend the existing generic in
`cli/entity_cmd.rs` (`run_edit_generic` already proves the pattern). Define a
per-entity descriptor (prefix, dirs from `Project`, column defs, filter
enum) and write one generic `run_list`/`output_cached`/`run_delete`/`run_new`
parameterized over it. Estimated saving: 10–20k lines and a single place to
fix list/output behavior. Do this as its own reviewed change, not mixed with
feature work.

## 2. Dual git implementations

`core/git/` uses **both** gix (repo.rs, index.rs, commit.rs — ~90 call sites)
and shell-out `git` (shell.rs — 35 call sites). Two code paths, two failure
modes (several shell-parsing bugs were fixed in this review). Pick one:
gix avoids subprocess/locale issues; shell is simpler to extend. Either way,
route new features through a single layer.

## 3. Two table renderers

The custom `cli/table.rs` (SHORT-column tables, wrapping) coexists with the
`tabled` crate (used by `report/rvm.rs`, `report/fmea.rs`,
`report/test_status.rs`). Converge on one. Note: `table.rs` width math is
byte-based, not display-width — CJK/emoji titles misalign columns (cosmetic;
the panic cases were fixed).

## 4. Reserved / unreachable code

- `Analysis3DConfig` (`entities/stackup.rs:435`) — serialized on Stackup,
  never read by any code path. Docs now mark it "reserved". Wire it up or
  remove it (removal needs a schema/doc change in the same commit).
- `LengthToleranceInfo` cross-term path (`core/sdt.rs:386`) — every
  production caller passes `None` (`run_3d_analysis` hardcodes it,
  `feat compute-bounds`/`validate` pass `None`). Only tests exercise it.
  Wire the feature lookup or delete the path.
- 13 `#[allow(dead_code)]` markers worth a quarterly audit.
- ~~`schemas/` (repo root) duplicate directory~~ — **resolved**: deleted; the
  embedded `crates/tdt-core/schemas/` copies are the single source, browsable
  via `tdt schema show <type>`.

## 5. Performance

Measured (debug build, 160 entities): `--help` 4ms, `req list` 10ms,
`search` 10ms, `trace matrix` 10ms, `status` 10ms — **fast, no action**.
The exceptions:

- `tdt validate`: 120 ms / 143 MB RSS — all 20 JSON schemas are compiled on
  every run (`Validator::new` upfront). Compile lazily per entity prefix
  actually present in the project.
- `tol add` rescans the entire `bom/components/` directory once per feature
  added (tol.rs component-name lookup loop). Use the entity cache instead.
- `link suspect list` reads every entity YAML file; fine at hundreds of
  entities, consider indexing suspect flags in the SQLite cache at thousands.
- Cache `auto_sync` staleness uses a global max-mtime heuristic — a file
  restored with a preserved mtime (rsync -a, Syncthing) is never detected as
  changed. Compare per-file mtimes against per-file cached values.

## 6. Architectural

- **Links are modeled as bare `EntityId` in typed entity structs**, so any
  typed load→save collapses `{id, title, suspect…}` link objects. A merge
  step in `ServiceBase::save` now restores the metadata (see
  `preserve_link_metadata`), but the durable fix is a `LinkRef` type on all
  Links structs. Note: `tol analyze` writes its stackup back with a raw
  `fs::write`, bypassing that merge.
- The link cache normalizes YAML field names (`components`→`contains`,
  `parent`→`contained_in`, `created_ncr`→`ncrs`); suspect mark/clear now
  scan-fallback to cope. Storing the originating YAML field name in the
  cache `links` table removes the guesswork.
- YAML mutation paths (`link add`, suspect mark/clear) strip the guidance
  comments that `new` writes, and their formatting differs from service
  saves (churny diffs). A comment/format-preserving YAML editor would fix
  both.
- `bulk.rs` edits YAML via string surgery (now line-anchored, but still
  string-based). Parse-modify-serialize would be robust.

## 7. Known gaps deliberately left (with rationale)

- `tdt import --update` errors as "not yet implemented" (previously a silent
  no-op that duplicated every row). Implement ID-matched updates.
- 3D Monte Carlo has no `--seed` (1D does) — 3D runs aren't reproducible.
- 3D staleness hash omits `functional_direction`/`geometry_3d`/
  `torsor_bounds` (documented in the 3D guide's Limitations section).
- 1D RSS `yield`/`margin` use the unshifted mean while Cpk uses the
  Bender-shifted mean (code comments show this is intentional; the pair
  reads inconsistently — consider shifting both under `mean_shift_k > 0`).
- Short-flag polarity differs between entity commands (`req new -T title
  -t type` vs `proc/ncr new -t title -T type`). Breaking change; align in
  the next major version.
- `risk.schema.json` documents a `links.controls` field written by neither
  the struct nor the link registry (Risk→Ctrl uses `mitigated_by`); decide
  and remove or wire.
- `asm list --columns short` is accepted but renders nothing (SHORT is
  implicit); drop the enum value at the next CLI-breaking release.
- Datum geometry/precedence and MMC datum shift are not modeled in 3D
  (documented); position/orientation controls without `datum_refs` are
  accepted silently — add a validation warning.

## Tooling recommendations

- Add `cargo machete` (unused deps) and `cargo clippy --all-targets -D
  warnings` to CI (all currently clean).
- Consider `typos` and a docs-example checker in CI: this review found 50+
  documented flags/subcommands that had drifted from the real CLI; a script
  that extracts ```bash blocks and validates flags against `--help` output
  (as done in this review) would prevent regressions.
