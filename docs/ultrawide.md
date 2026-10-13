# Ultrawide (21:9) — beta

**English** | [Português (Brasil)](#português-brasil)

The game's camera has a fixed *horizontal* field of view. Raising the render resolution to a 21:9 size
without touching the camera only crops the 16:9 picture top and bottom, so it looks zoomed in.
`Resolution Patch - 3440x1440` fixes that: the picture fills the whole screen, the world keeps its
proportions, and the vertical view is the same as at 16:9 (you simply see more to the sides).

## Using it

In the launcher, **Render resolution → 3440x1440 (21:9)** (or `render_resolution=3440x1440` in
`gow3.ini`, or the same entry in the in-game menu), then **PLAY**. The patch asks for 4608 MB of
extra direct memory (the Video Arena is 3.25 GiB); `run.py` sets that from the patch note, as for the
other resolution patches. Use a window or display of 3440x1440 (borderless is the easy way).

What you get at 3440x1440:

- The world and the HUD fill the screen; characters keep their proportions.
- Menus are in the right proportion.
- Cutscenes are 16:9 videos: they fill the screen and are cropped at the sides.

Tested on a 3440x1440 monitor, build `6e72330e…` (the full 01.02 update), engine 120 FPS. Other
ultrawide sizes use the same patch with other numbers (see below) but have **not been tested**.

## What it changes

1. The render resolution, as the other `Resolution Patch` entries do, and the Video Arena size.
2. The display aspect ratio, two floats in the video mode (`16/9` → `3440/1440`).
3. The tangent `t` of the field of view, multiplied by `k = (3440/1440) / (16/9) = 43/32`, in the four
   places the cameras read it. The projection is `x = 2/t`, `y = 2A/t` (`A` = aspect ratio), so with the
   new `A` and `t·k` the vertical view stays the one of a 16:9 picture and the horizontal one widens.
   The same `t` feeds the culling vectors (two builders) and the standard camera's `tan(fov/2)`, so
   nothing is culled at the sides.

There is no room for a multiplication in place. Each of the four sites becomes a `jmp` into two chained
15-byte NOP pads between functions ("caves": the first runs the replaced instruction, the second
multiplies by `k` and jumps back), and a ninth pad holds the constant `k`. Every site and pad is found
by a signature (`Pattern`/`At`) like the other patches; a build that does not match skips the patch.

## Other resolutions

`tools/make_ultrawide_patch.py` writes the entry for any size (it needs the 6e72 `eboot.bin` and capstone):

```
python tools/make_ultrawide_patch.py path/to/eboot.bin patches/God_of_War_III_Remastered.xml 2560 1080 --write
```

then add the name to `RESOLUTIONS` in `scripts/graphics_options.py` and `Resolutions` in
`gpu/shim/gow3_graphics.h` (and the labels in the launcher and `gpu/shim/gow3_overlay.cpp`).
If you test one, please say so in an issue.

## Reference build addresses

The `Address` attributes of the new lines are addresses in the `6e72…` build (the author did not have
the `d85c8135…` build). The signatures are what is used. To turn them into the reference build's
addresses, which the reference test compares against, run with that build's `eboot.bin`:

```
python tools/make_ultrawide_patch.py --rebase path/to/eboot_d85c.bin patches/God_of_War_III_Remastered.xml 3440 1440
```

---

## Português (Brasil)

A câmera do jogo tem campo de visão *horizontal* fixo. Aumentar a resolução para um tamanho 21:9 sem
mexer na câmera só corta a imagem de 16:9 em cima e embaixo, e ela parece com zoom. O
`Resolution Patch - 3440x1440` corrige isso: a imagem enche a tela, o mundo mantém as proporções e a
visão vertical é a mesma do 16:9 (você só vê mais dos lados).

**Como usar:** no launcher, **Render resolution → 3440x1440 (21:9)** (ou `render_resolution=3440x1440`
no `gow3.ini`, ou no menu do jogo) e **PLAY**. O patch pede 4608 MB de memória direta extra; o
`run.py` configura isso pela nota do patch. Use uma tela ou janela de 3440x1440.

Cutscenes são vídeos 16:9: enchem a tela e são cortadas nos lados. Testado em monitor 3440x1440, build
`6e72330e…`, motor a 120 FPS. Outros tamanhos ultrawide usam o mesmo patch com outros números
(`tools/make_ultrawide_patch.py`) mas **não foram testados**.
