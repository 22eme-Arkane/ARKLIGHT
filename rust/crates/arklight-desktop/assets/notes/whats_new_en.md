# What's new in ARKLIGHT

<!-- One "## X.Y.Z · YYYY-MM-DD" section per version, newest first. For EVERY new version:
     write the entry here AND in nouveautes_fr.md. -->

## 1.0.0 · 2026-10-30

**ARKLIGHT 1.0: completely rewritten to be much faster, with everything the previous version
did — and much more.**

- **Faster.** The 3D engine, the lighting, the network and the interface have been rewritten:
  the scene shows and responds instantly, even with many fixtures.
- **Fixture menus in the inspector.** Each fixture's front panel (Astera keypads, ARRI,
  Aputure, Kino Flo, Hive…) is operated right there, keys and knobs included. Then come its
  settings, its DMX patch, its position and its simulated state.
- **Move and rotate with the mouse.** X, Y, Z handles to move, rings to rotate, in 10 cm and 5°
  steps (snap), on everything selected.
- **Look through the camera** (C), **highlight** to see everything in the dark (H), Astera
  **infrared remote** (R).
- **The 2D plot goes further.** Drawn shapes get a height and become volumes in 3D; the
  background image can lie on the floor; a "Display" button shows or hides names, DMX
  addresses, the grid…; "Background and title block" sets the colour, the background image and
  the title block; "Print / PDF" outputs the plot, the equipment list and the patch at once
  (Ctrl+P).
- **A plan drawn on paper becomes a lighting plot.** "Drawn plan…": take a photo of a gaffer's
  plan, the assistant reads it and places the fixtures, grip, set, people, camera and shapes
  (a house, a platform…) on the plot and in 3D.
- **The full range of cloths on frames**: 1/4, 1/2, full silks and China Silk, grids and Silent
  Grid 1/4, 1/2, full, Soft Frost, bleached and unbleached muslin, Magic Cloth, Ultrabounce,
  Griffolyn, silver or gold lamé and soft, Lisa Marie checkerboard, solid, lavender, single,
  double and triple nets, sorted by family with their stop loss. Each one acts on the light.
- **Every image is accepted**: HEIC photos from iPhones and recent phones (no Windows extension
  to buy), JPEG, PNG, WebP, GIF, BMP, TIFF, SVG… and, depending on Windows extensions, AVIF,
  JPEG XL and RAW files. The photo is turned upright and converted automatically.
- **The AI assistant uses AIODOS by default**: requests are computed on this computer.
  Everything happens by itself: AIODOS is started if it is stopped, access is requested (you
  just accept it in AIODOS) and the right model is chosen — one that sees images for a drawn
  plan. If it isn't installed yet, the assistant offers it in one click.
- **Sequences and shots**: a window with a thumbnail of each shot to open, rename, duplicate,
  move or delete.
- **The console** drives several fixtures of the same model, with full, black, rainbow and raw
  DMX values; the **DMX monitor** shows each fixture's name on its channels and its address
  conflicts; the **Log** and **Network** tabs follow what comes in.
- **MIDI controller**: channel choice, presets to save, export and import.
- **Keyboard shortcuts**: every menu shows its key, a drawn AZERTY keyboard shows the keys in
  use, and everything can be changed in Edit › Keyboard shortcuts.
- **More comfortable**: ARKLIGHT opens full screen, resizable panels (their sizes are kept),
  full screen (F11), complete
  right-click menus, one ARKLIGHT window at a time, Quit, Save and Donate windows inside the
  application.
- **Apache-2.0 licence**: ARKLIGHT stays free and open, under a licence that is simpler to
  reuse.
- **Updates from ARKLIGHT.** When a new version is published, a banner offers to download and
  install it, without going through the browser. This "What's new" window then opens the first
  time each new version starts.

## 0.1.0-alpha.1 · 2026-10-06

**First public version.**

- Simulator of Astera fixtures and their on-board menus (Titan, Helios, Hyperion, AX9, NYX…),
  in a 3D scene with floor and cyclorama.
- Art-Net and sACN input: Luminair, Blackout or a console drive the virtual fixtures, which
  answer RDM.
- Other brands (ARRI, Aputure, Kino Flo, DMG, LiteGear, Hive, Dedolight), tungsten and HMI.
- Grip, stands, set and people; 2D lighting plot, equipment list, DMX patch, MVR export.
- AI assistant (Ollama, AIODOS or an online service) and MIDI controller.
