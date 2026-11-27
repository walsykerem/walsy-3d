---
name: walsy-3d
description: Build fake-3D rotatable pixel art from four flat views with the walsy-3d Rust toolchain: carve a slice strip, verify it, render a crisp turntable for Canvas, Godot, Unity or Phaser. Use for "walsy-3d", "fake 3D pixel art", "pixel art turntable", "stacked slices", "turn a top-down sprite without drawing every angle".
allowed-tools: Read, Write, Edit, Bash, Glob, Grep, walsy_carve, walsy_lint, walsy_preview, walsy_viewer
disable-model-invocation: true
---

# walsy-3d

## Negative scope

One job only: four flat views → carved horizontal slice strip → verified crisp turn in the target renderer. Rust + HTML only; the shipped toolchain is the sole executable surface. Tool ceiling is the frontmatter `allowed-tools`; never call anything outside it. Out of scope: scene extras, new engines, extra assets, a second render pipeline. No writes outside the artifact chain without approval. Explore flexibly — read the real renderer and source; add no ceremonial scaffolding. Long recipes stay in `references/engines.md`; do not copy them here.

### Keep scope exact

Workflows borrowed from elsewhere stay bounded to this job: no adjacent features, extra controls, extra engines or extra assets. If a step needs a new capability, stop and ask.

## Goal and Gate

Goal: a readable rotatable strip in the target renderer, verified at delivery size.

Every step: **entry gate** (inputs present and shaped right) → work → **exit gate** (its `## Done when` observed). **No next step before gate.** Start here; do not begin with project scaffolding or tool installation.

Whole-job entry gate:
- Subject + four views, or a prompt/idea; target engine and delivery size known.
- A supplied sheet that already renders correctly → exit unchanged, no re-carve.
- `BRIEF.md` written before any other artifact.

Whole-job exit gate:
- Delivery-size visual check passed; `check.png` and the full turn reviewed.
- `check` and `verify` both pass (commands in `## Cost and gates`).
- Per-step cost and skipped work reported.
- **Never ship a carved stack without render + visual inspection.**

### Cheap probes

Before any build, generation or export, run these gates in order; a failure stops the step:
1. `walsy_lint` / `check` command — structural audit.
2. Open `check.png` — inspect pixels, not file existence.
3. `diff` artifacts — only intended changes.
4. Render at delivery size and watch the full turn — final arbiter. Green lint ≠ correct geometry; exit 0 ≠ readable art; a `check.png` file existing ≠ four angles correct.

### Early exit and discount gates

- Nothing turns → stop at one still strip.
- Supplied sheet turns correctly → exit unchanged.
- Four views can be drawn or exported → skip image synthesis.
- A flag fixes orientation or geometry → skip geometry edits.
- Renderer already handles strips → skip integration changes.
- Report every discount with the step it skipped.

### Traps and false positives

HiDPI backing size ≠ CSS size; Godot viewport stretch; Unity Pixels Per Unit; flat white vs transparent background; JPEG fringe. Check these before trusting pixels. Never re-run a passing gate "to be sure" — trust results, not logs.

## Creator editing boundary

Text between `<!-- walsy:user-begin -->` and `<!-- walsy:user-end -->` is user-owned. Never overwrite, reformat or reorder it. If a required edit crosses a marker, stop and ask. Generated artifacts belong in `src/outputs/`; marked user regions win.

### Artifact chain

Fixed order, exact names, no ad-hoc renames: `BRIEF.md → specs.json → PLAN.md → src/outputs/`.

- `BRIEF.md`: subject, views, target engine, delivery size, motion.
- `specs.json`: payload keys `title,slices,sliceWidth,sliceHeight,sheet,top,side,front,back` plus output paths. Machine-checkable: `sheet.width === slices * sliceWidth`, `sheet.height === sliceHeight`, every named input exists.
- `PLAN.md`: ordered steps, each with its gate; names only files in this chain.
- `src/outputs/`: strip PNG, `check.png`, `viewer.html`, engine integration files.
- Cleanup before exit: remove format/resolution/file residue — stray scales, wrong-size exports, temporary sheets, duplicate viewers, misnamed files.
- User edits inside a chain file are bracketed by `walsy:user-begin` / `walsy:user-end`; keep them intact. Viewer payload marker `/*__EMBEDDED_STACK__*/null` at `rust/scripts-rs/src/viewer.html:74` stays intact when editing the viewer.

Programmatic output contracts are tied to the `check` / `verify` test scripts below: gate fails on missing key, wrong dimension or missing file; no exceptions.

## Step 0. Pick the path

```
What do you have?
├── Slice strip PNG ────▶ verify contract ▶ Step 3
├── Sheet ──────────────▶ Step 1 gate ▶ Step 2 ▶ Step 3
└── Prompt or idea ─────▶ Step 1 ▶ Step 2 ▶ Step 3
```

**Entry gate:** `BRIEF.md` exists. In an existing project, read the renderer first: engine, pixel scale, sprite draw/sort order, which objects turn. Fit the strip into that renderer; never add a second canvas or render pipeline.

## Done when

**Step 0:** path chosen; `specs.json` keys present; target renderer identified.

## Step 1. Get the four views

**If the user has a sheet, use it.** Do not regenerate it. If the user has a MagicaVoxel model, export its layers as four orthographic PNGs (TOP, SIDE, FRONT, BACK) at one scale, then continue. Otherwise draw four views yourself.

One 1x4 row: TOP, SIDE, FRONT, BACK on a flat background, gaps ≥40 px. TOP straight down with front pointing down; SIDE front pointing left; same scale everywhere.

Density from the closest shot, not the output size: **chunky** 15–20 slices, ~25×48 px, shown 8–10x; **detailed** 26–44 slices, 36×72 to 46×107 px; **showcase** 36–56 slices, 30×30 to 148×148 px, 4–6x integer scale on HD; rails, lantern posts, tendrils and waterfalls keep the turn readable. Spend detail where it reads.

Never sharpen or blur enlargements. For own sheets, upscale 8x with nearest-neighbour; one art pixel = one source pixel.

Draw the part that acts: a nozzle needs a lip and opening, a hinge for a moving panel, a contact patch for a wheel. Mark the attachment point and axis; use that pivot for effects and child parts. Design both mating surfaces when pieces latch.

Slicer keeps a voxel only where all views cover it:

- Round parts need a pixel circle in TOP; a square TOP carves a box. Narrow round parts (tower, balloon, pot) also need `--walsy-round`.
- Fins, leaves, wings, irregular plans need their real TOP outline or they carve as blocks.
- SIDE and FRONT silhouettes set the profile; wall detail in SIDE/FRONT/BACK, roof detail in TOP.
- A protrusion from a wall carves a ledge along that whole wall. Paint small protrusions flat; give a chimney a flat ridge.
- Shade with a 3–4 tone hue-shifted ramp: cool deep shadow, saturated mid, warm highlight. No gradients or anti-aliasing.
- Centre the object: the strip spins about its bounding-box middle.

**Entry gate: look at the sheet before slicing.** (1) TOP front down; SIDE front left, else `--walsy-side-nose right`. (2) TOP length = SIDE length, TOP width = FRONT/BACK width, SIDE height = FRONT/BACK height. (3) Flat fills and crisp outlines; airbrushed edges or thin 1 px internal lines smear — redraw. (4) Every gap must reach the view's outside; cavities hidden from all four silhouettes fill in and need hand patching after carving. (5) Run the Lessons table.

## Done when

**Step 1:** sheet path recorded in `specs.json`; orientation, size, edges, gaps verified against pixels.

## Step 2. Carve and inspect

Build from this skill's folder, then run the binary (paths relative to `rust/app`):

```bash
cd rust/app && cargo build --offline --release
target/release/views_to_slices <sheet> <out> [flags]

target/release/views_to_slices views.png slices.png \
  --walsy-slices 36 --walsy-shade 0.2 --walsy-preview check.png --walsy-html viewer.html

target/release/views_to_slices --walsy-compose apple slices.png \
  --walsy-preview check.png --walsy-html viewer.html
```

Binary: `views_to_slices`. It clears background (transparent or flat), strips 1 px view outlines, quantizes to one shared palette, carves slices, writes the strip to `<out>`. `--walsy-preview` writes the check sheet; `--walsy-html` writes the HTML viewer. No authoring runtime ships: Rust tools plus `viewer.html` as the only non-Rust payload.

| Flag | Default | When to change it |
| --- | --- | --- |
| `--walsy-slices` | auto (36 if SIDE taller than 64 px, else native) | 15–20 chunky, 32–48 detailed, native height for drawn sheets, `0` keeps raw pixels |
| `--walsy-order` | `auto` | `auto` matches size ratios (TOP length = SIDE length, TOP width = FRONT width, SIDE height = FRONT height), reading order wins ties; `reading` forces TOP, SIDE, FRONT, BACK |
| `--walsy-top-nose` | `auto` | Auto reads car cabins; pass `down` for anything not a vehicle, `up`/`down` to flip a car |
| `--walsy-side-nose` | `left` | `right` if SIDE faces right |
| `--walsy-colours` | `24` | 32–48 for detailed liveries, `0` to keep every colour |
| `--walsy-compose` | off | `apple`, `banana`, `orange`: generate the four-view sheet in Rust; pass only `<out>`, preset supplies carve flags unless overridden |
| `--walsy-shade` | `0.2` | 0.12–0.15 for richly shaded views, 0.25 for flat ones, 0 for none |
| `--walsy-round` | off | Towers, balloons, pots, bottles: clips each slice to the ellipse SIDE/FRONT spans describe; leave off for fins, wings, boxes |
| `--walsy-dark-thresh` | `70` | `0` to keep outlines on very dark silhouettes |
| `--walsy-bg-tol` | `28` | Raise a little if JPEG noise leaves a fringe |
| `--walsy-preview` | *(none)* | Path for `check.png`: four extracted views, four turntable angles, exploded view |
| `--walsy-html` | *(none)* | Path for the HTML viewer |
| `--walsy-normals` | *(none)* | Save a matching 3D normal-map strip PNG |
| `--walsy-emissive` | *(none)* | Save an auto-detected emissive mask strip PNG |

Open `check.png` immediately: verify all four extracted views, four turntable angles and the exploded view before writing integration code.

`viewer.html` ships pointer drag orbit plus core controls: angle, squash, spacing, scale, slices, time, sun, light, shadow, outline, load PNG, 16-angle export, slice scan, exploded view and keyboard. Nothing else. Payload keys are `title,slices,sliceWidth,sliceHeight,sheet,top,side,front,back`.

## Done when

**Step 2:** strip dimensions match `specs.json`; check sheet inspected; viewer opens and turns under pointer drag.

## Step 3. Render: loop and numbers

Bottom to top, in every renderer:

```js
// sheet: horizontal strip of `count` slices, each w × h, ground slice at z = 0
// pos: ground contact centre, rounded to whole pixels
for (let z = 0; z < count; z++) {
  // 1. rotate the slice about (w / 2, h / 2) by `turn`
  // 2. scale y by `squash` (camera pitch)
  // 3. lift by -round(z * spacing)
  // 4. translate to whole-pixel `pos`, draw with nearest filtering
}
```

Do not guess these:

- **`spacing = 1.0`** screen px at art-pixel scale. **`squash = 0.75`** for angled top-down; 1.0 straight down, <0.6 flattens roofs. Tilting camera: `squash = sin(elevation)`, `spacing = cos(elevation)`.
- **Rotation centre `(w - 1) * 0.5, (h - 1) * 0.5`** in pixel-index coordinates; the Rust renderer uses the same mid-slice pivot as `(x, y)` centred at `w/2, h/2`. `Math.round(...)` and `Math.floor(...)` are coordinate-convention choices, not different centres: `floor(x + w/2)` indexes pixel `x + (w - 1) * 0.5`. Never mix index and centred coordinates in one transform.
- **Snap once at the final blit.** Keep bobbing, orbit and child projections in floating-point screen px; round only at the 1-screen-pixel `drawImage` destination (`Math.round(dx), Math.round(dy)`) at integer display `scale`.
- **Validate `sheet.width % count === 0`** and slice dimensions at load, so a denser strip cannot slice mid-frame.
- **Nearest filtering:** `imageSmoothingEnabled = false`, `image-rendering: pixelated`, Nearest/Point in engines.
- **Ground shadow:** vehicles get union footprint at z = 0, offset +2/+3 art px, before slice 0. Tall props project slices along the sun vector per layer with Bayer 4×4 dithered edges.
- **Outline the finished stack once**, in a 1x art-pixel buffer: trace 1 px around its alpha, then scale.
- **Heading:** sheet front points down (+y), so `turn = atan2(vy, vx) - PI / 2`. Other native axes: `+x` → `atan2(vy, vx)`; `-y` → `atan2(vy, vx) + PI / 2`. Seafloor crawler nose +y; deep manta nose +x; submerged dolphin nose -y. Verify each strip's axis visually before binding.
- **Y-sort whole stacks by ground point.** Never interleave slices of two objects. Bake static stacks at load: 200 trees × 30 slices is 6,000 draws a frame.
- **Entrances** animate slices, not opacity fades: slice-bloom bottom-to-top with a ground ring; extend the reveal frontier to `count + width` before returning `null`, or top slices snap in. Glide airborne/waterborne stacks along a travel arc. Use continuous camera moves, not defocus cuts.
- **Sway/tilt:** anchored props offset each slice by `sin(t*freq - z*wave + phase)*amp*(z/(count-1))²`; leaping creatures and rolling hulls offset `(xOffsets, yOffsets)` by `(z - centerZ)*sin(pitchAngle)`.
- **Underwater:** Beer-Lambert desaturation toward the water hue; soften or omit hard outlines; heave/pitch/roll from a damped spring-mass response to the wave field.
- **Articulated strips:** dolphin fluke, manta wings, fish tail, bird wings; frames `f0..fN-1` with identical `(w, h, count)`, same pivot `(w - 1) * 0.5, (h - 1) * 0.5`, Z headroom 18–20 where 12–16 clips, frame chosen by deterministic physical phase.
- **Spheres:** `R_z = R_xy * sqrt(1 - squash²) / spacing` for an exact circle; store albedo only, apply a world-space terminator at blit; draw ring particles with `y_local < 0` before the stack, `y_local >= 0` after, eclipsing those in the shadow cone.

`references/engines.md` has Canvas, React, Godot, Unity and Phaser recipes. `rust/scripts-rs/src/viewer.html` is the web reference.

### When motion leaves a trace

Match trace to material and medium (spray at a paddle, dust at a tire, gas at a thruster); a still object may need none. Drive sprite and effect from one event. Emit at the departure pose with world + exit velocity; after release apply only medium forces. Torque changes angular velocity, not the centre path. A packet's velocity includes rotation about the pivot. Do not drag the trail with the sprite or camera. Group particles near the source, then let them separate. Seed birth times once so seeking back gives the same frame.

## Done when

**Step 3:** straight down (`squash = 1`, `turn = 0`) gives one slice wide, `sliceHeight + count - 1` tall; 0.75 visibly shorter; at 90° width equals `sliceHeight`; spacing 0 flat, 2.5 separates layers; no stripes or dark speckle at 45°; reversing holds heading; every angle watched, not four; at 390 px phone feed and HD delivery size subject, facing and action read without labels; orbital shots keep a quiet sky and thin lit limb, not full-frame stars and dither.

## Step 4. Finish

Readability is the test, not a style mandate. On tiny sprites, RotSprite is an eye check only: zoom in and compare long edges before keeping it.

## Done when

**Step 4:** delivery-size check above passes; `check.png` and full turn reviewed; `check` and `verify` both pass; per-step cost and skipped work reported.

## Cost and gates

- **Cost visibility:** report per-step cost and what was skipped. Cheap probes first; expensive work last.
- **Approval gate:** image generation, dense re-carve, batch exports, installs and writes outside the chain require approval before starting.
- **Hard gates:** before any irreversible or expensive step and before the final report, confirm the gate evidence; stop and ask if missing. A verdict never replaces a real check.
- **Speedup evidence:** if `do-fast-with-same-language` is used, report CPU share and per-unit cost (µs/item or µs/KB), not raw ms. Projected gain = `share × (1 − 1/speedup)`; below ~1% share call it a micro-optimization and stop.
- **Trust results, not logs:** trust result artifacts, not log text. Never re-run a passing gate to "be sure".
- **Lint self-audit:** after edits, run both commands; fix the cause, then re-run only if the artifact changed.

```
cargo run --offline --quiet --manifest-path rust/lint-skill/Cargo.toml -- check
cargo run --offline --quiet --manifest-path rust/lint-skill/Cargo.toml -- verify
```

## Lessons

Fix the mistake, then fix its cause so it cannot recur: change the tool or make it warn with the fix. Only judgment calls code cannot see stay as rules. Delete a row once the tool makes the mistake impossible.

| Date | Mistake | Rule | Enforced by |
| --- | --- | --- | --- |
| 2026-11-20 | Thin posts, ropes, antennas blink as the strip turns | Draw thin parts ≥2 px wide | Slicer warning (`thin_runs`), verify |
| 2026-11-20 | Narrow round part carves square | `--walsy-round`; gapped rows stay square | `--walsy-round`, verify |
| 2026-11-20 | Auto `--walsy-top-nose` flips non-vehicles | Pass `--walsy-top-nose down` | Rule |
| 2026-11-20 | Wall protrusion carves a ledge along the whole wall | Paint small protrusions flat; give a chimney a flat ridge | Rule |
| 2026-11-20 | Enclosed background between ropes, fins, legs carves solid | Every gap must reach the view's outside | Rule |
| 2026-11-20 | Off-centre base orbits off-axis | Centre the object in its views | Rule |
| 2026-11-20 | Mixed half-pixel and centred coordinates wobble at 90°/180°/270° | Use `(w - 1) * 0.5, (h - 1) * 0.5` for pixel indices; `Math.round(...)` on centred `(x, y)` looks up the same pixel | Rule, verify |
| 2026-11-20 | Quantized bobbing/orbit/child projection jitters | Keep motion continuous; snap at the final 1-screen-pixel blit | Rule |
| 2026-11-20 | Tiny 12–24 px art blown 7–9x on HD looks chunky | Denser strips (36–56, 30–148 px), 4–6x integer scale | Rule |
| 2026-11-20 | Slice-count change without loader change slices mid-frame | Validate `sheet.width % count === 0` at load | Rule, verify |
| 2026-11-20 | Fading mid-air or defocus cuts flatten pixel art | Slice-bloom + ground ring; glide orbiters; continuous camera moves | Rule |
| 2026-11-20 | `lifts` frontier stopping at `count` snaps top slices | Extend frontier to `count + width` before returning `null` | Rule |
| 2026-11-21 | Assuming strips face `+y` sends custom ships and fish sideways | Verify native nose axis before `turn` | Rule |
| 2026-11-21 | Full saturation and hard ink underwater break depth | Beer-Lambert red loss, soften outlines, couple to wave field | Rule |
| 2026-11-21 | Single rigid pose or clipped flukes and wings look stiff | Multi-frame strips, identical `(w,h,count)`, shared pivots, phase-synced frames | Rule |
| 2026-11-21 | Equal-radius spheres stretch into eggs | `R_z = R_xy * sqrt(1 - squash²) / spacing` | Rule |
| 2026-11-21 | Baked sun shading spins with planet surface | Pure albedo strip, world-space terminator at blit | Rule |
| 2026-11-21 | Ring particles float wholly in front of or behind a planet | Back before strip, front after, eclipse in shadow cone | Rule |

## MCP server

Rust stdio server at `rust/mcp/src/main.rs`; copy `mcp/opencode.json.example` into the OpenCode MCP config, restart the client. Use MCP (`walsy-3d` server) for repeated calls and CLI (`views_to_slices`) for one-offs. Arguments are tool-specific, not CLI mirrors; results are short text summaries with output paths and byte sizes.

| Tool | Call with | Returns |
| --- | --- | --- |
| `walsy_carve` | `{"sheet":"views.png","out":"slices.png","slices":36,"shade":0.2}`; also `order`, `top_nose`, `side_nose`, `preview`, `html`, `compose` (`apple`/`banana`/`orange`; pass `sheet:"-"`) | `"36 slices; strip slices.png (N bytes)"`, plus preview/viewer paths when requested |
| `walsy_preview` | `{"sheet":"views.png","out":"check.png"}` | `preview check.png (N bytes)` |
| `walsy_viewer` | `{"sheet":"views.png","out":"viewer.html"}` | `viewer viewer.html (N bytes)` |
| `walsy_lint` | `{"mode":"check"}` or `{"mode":"verify"}` | `lint check helper completed` or `lint verify helper completed` |
