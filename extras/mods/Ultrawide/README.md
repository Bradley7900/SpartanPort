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
| 1920x1200 | 16:10 | Not tested (taller than 16:9: see Notes) |
| 2560x1600 | 16:10 | Not tested (taller than 16:9: see Notes) |

If you try one of the untested sizes, please say how it looks (resolution, and whether anything is
cropped, stretched or missing at the edges) in an issue of the project.

## How to install

**Versions after 2.2.0 (the easy way: one file).**

1. Put `Ultrawide.zip` into the `mods` folder, next to `God of War III.exe` (in the launcher: **Mods** →
   **Open**; create the folder if it is not there). Do not unzip it: the game does that by itself.
2. Open the launcher. The mod is listed on the Mods page, ticked, with "(game patches)".
3. In **Render resolution** (Home or Performance) pick `3440x1440 (mod)` or another size, then **PLAY**. The
   in-game menu (Insert → Performance) lists them too. The **Engine frame rate** (60, 120 or 240) is a
   separate choice that works with any resolution.
4. Use a display or window of the same size (borderless is the easy way).

**Version 2.2.0 (it cannot read zip mods yet): use the installer, `Ultrawide-installer-2.2.0.zip`**
(attached to pull request #202).

1. Close the game and the launcher. Unzip `Ultrawide-installer-2.2.0.zip` anywhere.
2. Double-click **`INSTALL.bat`**. Paste the game folder when it asks (the one with `God of War III.exe`; if you
   unzipped inside that folder it finds it by itself), then pick a resolution from the menu.
3. Start the game with **`Play God of War III.exe`**. Do not use the launcher on 2.2.0: it does not list the new
   sizes and can reset the setting. To pick another size later, run `Ultrawide - choose resolution.bat` in the
   game folder.
4. To undo everything: **`UNINSTALL.bat`** (it puts the original files back).

The installer only touches `run.py` and three files of `scripts\`, after backing them up to
`ultrawide-backup`, and refuses to run if they are not the stock 2.2.0 files (it never overwrites a newer
version). Its files are plain text: read them before you run them.

To go back to normal: untick the mod (or pick another resolution, or `UNINSTALL.bat`). Nothing of the game
is changed.

## Notes

- **Cutscenes keep their borders.** They are pre-rendered 16:9 videos, so they play with black bars at
  the sides; stretching them would distort the picture, and they cannot be re-rendered.
- **16:10 (taller than 16:9).** The game's camera keeps the horizontal view, so a taller picture simply shows
  more at the top and bottom, without distortion. These entries only set the resolution and the aspect ratio
  (no field of view change). 4:3 is not included.
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

**Como instalar.** Nas versões **depois da 2.2.0**: coloque o `Ultrawide.zip` (sem extrair) na pasta `mods`, ao
lado do `God of War III.exe` (no launcher: **Mods** → **Open**), abra o launcher e escolha o tamanho em
**Render resolution**. Na **2.2.0** (que ainda não lê mods em zip): baixe o instalador
`Ultrawide-installer-2.2.0.zip` (anexado ao pull request #202), extraia, dê dois cliques no **`INSTALL.bat`**, cole
a pasta do jogo, escolha a resolução no menu e inicie o jogo pelo **`Play God of War III.exe`** (não pelo
launcher). Para desfazer: **`UNINSTALL.bat`**. Em **Render resolution** aparecem `3440x1440 (mod)` e os outros tamanhos (e no menu do jogo,
tecla Insert). O **Engine frame rate** (60, 120 ou 240) é uma escolha à parte e vale com qualquer resolução.

**As cutscenes continuam com bordas:** são vídeos 16:9 pré-renderizados, então tocam com barras pretas dos
lados. Só o 3440x1440 foi testado; se testar outro tamanho, conte como ficou numa issue do projeto.
Os tamanhos 16:10 (1920x1200 e 2560x1600) só mudam a resolução e o aspect ratio: como a câmera mantém o campo
horizontal, uma imagem mais alta mostra mais em cima e embaixo, sem deformar.
