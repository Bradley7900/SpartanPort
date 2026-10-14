# Ultrawide mod (21:9 and 32:9) — beta

**English** | [Português (Brasil)](#português-brasil)

Makes God of War III Remastered fill an ultrawide screen **without the zoomed-in picture**. The game's
camera has a fixed horizontal field of view, so a plain resolution change only crops a 16:9 picture top
and bottom. This mod adds render resolutions that also widen the field of view: the world keeps its
proportions and you see more to the sides.

| Render resolution | Aspect | Status |
|---|---|---|
| 3440x1440 | 21:9 | **Tested in game** |
| 2560x1080 | 21:9 | Not tested |
| 3840x1600 | 21:9 | Not tested |
| 5120x1440 | 32:9 | Not tested |

If you try one of the untested sizes, please say how it looks (resolution, and whether anything is
cropped, stretched or missing at the edges) in an issue of the project.

## Install

1. Copy this whole folder (`Ultrawide`) into your mods folder: in the launcher open **Mods** →
   **Open** (the folder is `mods` next to `God of War III.exe`).
2. Open the launcher again. The mod is listed on the Mods page, ticked, with "(game patches)".
3. **Render resolution** now lists `3440x1440 (mod)` and the other sizes. The in-game menu
   (Insert → Performance) lists them too. Pick one, and choose the **Engine frame rate** (60, 120 or 240)
   as usual: it is a separate setting that works with any resolution.
4. Use a display or window of the same size (borderless is the easy way), then **PLAY**.

To go back to normal, untick the mod (or pick another resolution). Nothing of the game is changed.

## Notes

- **Cutscenes keep their borders.** They are pre-rendered 16:9 videos, so they play with black bars at
  the sides; stretching them would distort the picture, and they cannot be re-rendered.
- At 32:9 the horizontal field of view is doubled, so objects at the very edges look stretched, as in any
  game with a very wide field of view.
- It needs extra direct memory (the Video Arena grows with the pixel count); the launcher sets that from the
  patch notes. It works on game builds the patch signatures match (tested on `eboot.bin` sha256 `6e72330e…`).
- How it works and how to make another size: [docs/ultrawide.md](../../../docs/ultrawide.md).

---

## Português (Brasil)

Faz o God of War III Remastered encher uma tela ultrawide **sem a imagem com zoom**. A câmera do jogo tem
campo de visão horizontal fixo, então só trocar a resolução corta a imagem de 16:9 em cima e embaixo. Este
mod adiciona resoluções que também abrem o campo de visão: o mundo mantém as proporções e você vê mais dos
lados.

**Instalar:** copie esta pasta (`Ultrawide`) para a pasta de mods (no launcher: **Mods** → **Open**) e abra o
launcher de novo. Em **Render resolution** aparecem `3440x1440 (mod)` e os outros tamanhos (e no menu do jogo,
tecla Insert). O **Engine frame rate** (60, 120 ou 240) é uma escolha à parte e vale com qualquer resolução.

**As cutscenes continuam com bordas:** são vídeos 16:9 pré-renderizados, então tocam com barras pretas dos
lados. Só o 3440x1440 foi testado; se testar outro tamanho, conte como ficou numa issue do projeto.
