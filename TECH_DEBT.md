# Technical Debt Register

Compiled 2026-07-16 during a full codebase review; last updated 2026-07-30.
Every item from the original register has been resolved except those listed
under **Remaining** below. The resolved-work summary at the bottom records
what was done and where, for archaeology.

## Remaining

Ordered by recommended priority.

### Worth doing when touched

- **`LengthToleranceInfo` cross-term path is unreachable in production**
  (`core/sdt.rs`): every production caller passes `None`; only tests
  exercise it. Wire the feature lookup (so length tolerance feeds angular
  variance) or delete the path.
- **YAML mutation paths strip guidance comments** (`link add`, suspect
  mark/clear): entities are created with helpful comments, and the first
  link operation rewrites the file without them; formatting also differs
  from service saves, producing churny git diffs. A comment/format-
  preserving YAML editor would fix both.
- **`bulk.rs` edits YAML via string surgery**: now line-anchored and safe,
  but parse-modify-serialize would be structurally robust.
- **1D RSS `yield`/`margin` use the unshifted mean while Cpk uses the
  Bender-shifted mean** when `mean_shift_k > 0` (3D shifts consistently);
  consider shifting all three for a coherent story.
- **`risk.schema.json` documents a `links.controls` field** written by
  neither the struct nor the link registry (Risk→Ctrl uses `mitigated_by`);
  decide: remove from schema or wire it.

### Scale-dependent (act when projects grow)

- **`link suspect list` reads every entity YAML file**: fine at hundreds of
  entities; index suspect flags in the SQLite cache at thousands.
- **`cli/table.rs` width math is char-count-based**, not display-width:
  CJK/emoji titles can misalign columns (cosmetic; the old byte-slicing
  panics are fixed).

### Parked for the next CLI-breaking release

- **Short-flag polarity differs between entity commands** (`req new -T
  title -t type` vs `proc`/`ncr new -t title -T type`). Align on one
  convention.
- **`asm list --columns short`** is accepted but renders nothing (SHORT is
  implicit); drop the enum value.

### Out of scope by design (documented, revisit only on demand)

- **3D datum modeling is partial**: the 3-2-1 analysis selects derived-
  bound DOFs (first GD&T control only); datum feature geometry, precedence
  effects, and MMC datum *shift* are not modeled. `tdt validate` warns on
  datumless datum-dependent controls. See the 3D guide's Scope section.
- **3D staleness hash covers stackup-level inputs** (`functional_direction`,
  `measurement_point`) but not linked feature files — re-analyze after
  editing a feature's `geometry_3d`/`torsor_bounds`.
- **Small-displacement linearization** (inherent to SDT).
- **Further entity-command generification** (`run_list` filter building,
  `new`/`show` displays): what remains is genuinely entity-specific;
  generifying it is abstraction for its own sake. The list/output pipeline
  is already fully consolidated.

### CI/tooling recommendations (not yet adopted)

- `cargo clippy --all-targets -D warnings` and `cargo machete` in CI (both
  currently clean; the gate would keep them so).
- A docs-example checker in CI: extract ```bash blocks from README/docs and
  validate each `tdt` command's flags against `--help` output. The original
  review found 50+ drifted examples this way; the script pattern is proven.

## Resolved (2026-07-16 → 2026-07-30)

For history; details in the commit messages on `main`.

| Area | Resolution |
|---|---|
| ~70 verified bugs from the full review | Fixed with regression tests (tolerance math, suspect links, imports, git layer, panics) |
| Entity-command duplication | One generic cached-list output (10 entities), one slow-path output (15), one directory table, one format resolver, one `list_columns!` macro; ~2.1k lines removed |
| Dual git implementations | Feature-gated: shell-only on desktop (gix not compiled, −129 crates, binary 74.2→62.7 MB); gix kept for mobile targets; `gix-vc` CI compile-check |
| Two table renderers | `tabled` dropped; ~50-line `MarkdownTableBuilder` serves the six reports |
| Dead/reserved code | Blanket `allow(dead_code)` gone, 17 dead items deleted, `Analysis3DConfig` wired, duplicate root `schemas/` deleted |
| Links as bare IDs | All 21 `*Links` structs use `LinkRef`; typed round-trips preserve metadata natively; save-path merge workaround deleted |
| Cache link aliasing | `links.field_name` column (schema v14) records the real YAML field |
| `auto_sync` staleness gap | Per-file mtime comparison; older-mtime restores detected |
| `validate` startup cost | Lazy schema compilation (120ms→70ms, 143→82 MB) |
| `import --update` | Implemented: generic ID-matched, schema-validated CSV updates; SysML overwrite gated behind the flag |
| 3D model limitations (7 items) | Rotated feature frames (full rotation Jacobian), measurement point, circular/conical zones, flatness tilt, datum warnings, 3D direction/Bender parity; hand-verified end-to-end |
| Documentation drift | Every bash example in README + 26 docs validated against the real CLI; 3D guide rewritten with a hand-checkable worked example |
