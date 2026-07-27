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

**Progress:**
- ~~output_cached_* duplication~~ — **resolved**: one generic
  (`entity_cmd::output_cached_list`) serves 10 entities.
- ~~slow-path output_* duplication~~ — **resolved**: one generic
  (`entity_cmd::output_entity_list`) serves 15 entities (lot/dev keep their
  custom CSV renderers; haz has a fixed column set).
- ~~entity-directory tables~~ — **resolved**:
  `Project::entity_search_directories` is the single source.
- ~~Auto→TSV format resolution~~ — **resolved**: `GlobalOpts::list_format()`
  replaced 26 copies.

- ~~per-entity `ListColumn` enums/impls~~ — **resolved**: `list_columns!`
  macro generates enum + clap value names + `key()` + `Display` consistently
  (help output verified byte-identical).

**Remaining in this area:** the `run_list` filter-building and dispatch
(mostly entity-specific — generifying further has poor risk/reward) and
`new`/`show` ceremony. Generifying `new`/`show` displays is NOT recommended:
they differ meaningfully per entity.

## 2. Dual git implementations

**Resolved (feature-gated):** desktop/CLI builds now use shell `git`
exclusively — `gix` is not compiled at all (129 crates dropped from the
build, debug binary 74.2 → 62.7 MB). Mobile targets keep the gix-backed
local operations via a target-conditional dependency, since iOS/Android
have no `git` binary. The `gix-vc` cargo feature compile-checks the gix
path on desktop CI. Both implementations expose an identical `Git` API
(`shell_local.rs` vs `repo.rs`/`index.rs`/`commit.rs`); behavior parity is
covered by the git test suite, which now runs against the shell path.

## 3. Two table renderers

The custom `cli/table.rs` (SHORT-column tables, wrapping) coexists with the
`tabled` crate (used by `report/rvm.rs`, `report/fmea.rs`,
`report/test_status.rs`). Converge on one. Note: `table.rs` width math is
byte-based, not display-width — CJK/emoji titles misalign columns (cosmetic;
the panic cases were fixed).

## 4. Reserved / unreachable code

- ~~`Analysis3DConfig` never read~~ — **resolved**: `enabled: true` now
  triggers 3D analysis without `--3d`, and `monte_carlo_iterations` is used
  when `--iterations` is left at its default. `method` remains reserved.
- `LengthToleranceInfo` cross-term path (`core/sdt.rs:386`) — every
  production caller passes `None` (`run_3d_analysis` hardcodes it,
  `feat compute-bounds`/`validate` pass `None`). Only tests exercise it.
  Wire the feature lookup or delete the path.
- ~~Blanket `#![allow(dead_code)]` suppressions~~ — **resolved**: the six
  module-level allows are gone; 17 dead functions/items deleted (incl. the
  empty `cli/output.rs` module and `MarkdownTable`). Remaining targeted
  allows carry rationale comments tied to the consolidation.
- ~~`schemas/` (repo root) duplicate directory~~ — **resolved**: deleted; the
  embedded `crates/tdt-core/schemas/` copies are the single source, browsable
  via `tdt schema show <type>`.

## 5. Performance

Measured (debug build, 160 entities): `--help` 4ms, `req list` 10ms,
`search` 10ms, `trace matrix` 10ms, `status` 10ms — **fast, no action**.
The exceptions:

- ~~`tdt validate` upfront schema compilation~~ — **resolved**: lazy per-
  prefix compilation; 120ms/143MB → 70ms/82MB on the 160-entity project.
- ~~`tol add` per-feature directory rescan~~ — **resolved**: O(1) entity
  cache lookup.
- `link suspect list` reads every entity YAML file; fine at hundreds of
  entities, consider indexing suspect flags in the SQLite cache at thousands.
- ~~`auto_sync` global max-mtime heuristic~~ — **resolved**: per-file mtime
  comparison in a single walk (also catches added/deleted files in the same
  pass); a content change carrying an older mtime is now detected.

## 6. Architectural

- ~~Links modeled as bare `EntityId`/`String` in typed entity structs~~ —
  **resolved**: all 21 `*Links` structs now use `LinkRef` (string-or-object,
  with `title` + suspect fields), so typed round-trips preserve link metadata
  natively. The `preserve_link_metadata` merge in `ServiceBase::save` was
  deleted; the raw `fs::write` in `tol analyze` is safe for the same reason.
  Bonus fix: `String`-typed links previously failed to *load* files whose
  links carried titles.
- ~~Cache normalizes link field names with no record of the original~~ —
  **resolved**: the `links` table now stores `field_name` (the actual YAML
  field), suspect marking uses it first, and the scan fallback remains only
  for caches built before the schema bump (v14 forces a rebuild).
- YAML mutation paths (`link add`, suspect mark/clear) strip the guidance
  comments that `new` writes, and their formatting differs from service
  saves (churny diffs). A comment/format-preserving YAML editor would fix
  both.
- `bulk.rs` edits YAML via string surgery (now line-anchored, but still
  string-based). Parse-modify-serialize would be robust.

## 7. Known gaps deliberately left (with rationale)

- ~~`tdt import --update` unimplemented~~ — **resolved**: one generic
  ID-matched update path serves all CSV entity types (schema-validated
  patches, revision bump, dry-run support); SysML re-import now skips
  existing entities unless `--update` is passed (closing the silent
  lossy-overwrite gap).
- ~~3D Monte Carlo not seedable~~ — **resolved**: `--seed` now drives the
  3D run too and the seed is persisted in `analysis_results_3d.mc_seed`.
- 3D staleness hash now covers `functional_direction`; linked feature
  files (`geometry_3d`/`torsor_bounds`) are still uncovered (documented).
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
