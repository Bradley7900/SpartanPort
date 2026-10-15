# Build de teste da #121 (cena 3D só num retângulo de 512x256 no canto)

Issues: [#121](https://github.com/FelipePrado0/gow3_pc/issues/121) e
[#142](https://github.com/FelipePrado0/gow3_pc/issues/142) (mesmo defeito, fechada).
Tag de teste: `windows-v2.3.0-issue121-test1`, gerada pelo workflow `windows-zip.yml` a partir da
branch `worktree-issue121-test`.

## 1. O que se sabe

- A cena opaca só sai certa num retângulo fixo de 512x256 pixels de render no canto superior
  esquerdo, em 1080p, 720p e 480p. Céu, chuva, partículas, decalques, HUD e textos cobrem a tela
  inteira.
- Só duas máquinas mostram o defeito, as duas RDNA2 com driver AMD de 2024: RX 6600 com Adrenalin
  24.5.1 (#121) e Ryzen 3 7320U com driver 32.0.11020.16 (#142). Em 135 logs coletados, só os
  logs delas não têm `VK_KHR_pipeline_binary`, `VK_KHR_maintenance8`,
  `VK_EXT_image_2d_view_of_3d` e `VK_KHR_compute_shader_derivatives` com `VK_KHR_maintenance5`
  presente.
- GCN no Adrenalin 26.5.2, Intel e Steam Deck também não têm `maintenance8`,
  `image_2d_view_of_3d` e `compute_shader_derivatives` e desenham a tela inteira. Os caminhos
  alternativos do port sem essas extensões, sozinhos, não explicam o defeito.
- `GOW3_NO_MSAA` não muda nada (log do `amd-teste1`).
- No port, a área de render é o mínimo das extensões dos anexos, sempre com origem (0,0)
  (`vk_rasterizer.cpp` 2836-2837, 2885-2886, 2962-2963; `vk_scheduler.cpp` 124-129). Um anexo de
  512x256 num passe de cena daria exatamente o retângulo. Nenhum log ou captura mostrou isso ainda.
- Hipóteses que restam: (1) o port entrega um anexo ou área de 512x256 aos passes opacos só
  nessas máquinas; (2) bug do driver de 2024 num estado que o port usa em todo driver (scissor de
  janela, estado dinâmico de viewport/scissor, estado dinâmico de profundidade com
  `pDepthStencilState` nulo); (3) falta de barreira.

## 2. O que o build faz

Nada muda no render sem as variáveis de teste, além de linhas novas no log e de uma captura de
quadro automática.

### 2.1 Linha do driver (sempre)

`GPU: driver <driverName>, info <driverInfo>, version <versão>, Vulkan <api>, id <driverID>,
pci <vendor>:<device>; maintenance8 0|1, eds3 0|1, depth bounds 0|1, ...`, impressa logo depois de
`CreateDevice` (`vk_instance.cpp`, depois da linha 195). Hoje o driver só aparece em `LOG_INFO`,
que o log padrão esconde. Formatador puro em `renderer_vulkan/vk_driver_report.h`, teste
`tests/test_driver_report.cpp`, alvo `driver-report-test`.

### 2.2 Verificação da área de render (sempre)

Header puro `renderer_vulkan/vk_area_check.h` (só std, como `vk_pass_census.h`), teste
`tests/test_area_check.cpp`, alvo `area-check-test`. Chamada em `BeginRenderingFull` depois do
override de passe reduzido (linha 3069) e antes do bloco do censo.

- Compara a área com a extensão **vinculada** de cada anexo (a view de verdade: proxy do passe
  reduzido, alvo do redirect aceito ou o nativo), nunca com a nativa, para não acusar os
  overrides legítimos.
- Por dimensão, `folga = vinculada - área`: negativa = `AreaExceeds`; de 1 a 63 px = padding
  legítimo (depth 1920x1088 contra área 1920x1080); 64 px ou mais = área menor que o anexo.
- Veredictos, em ordem: `AreaExceeds`, `AreaBelowAll` (algum anexo maior e nenhum explica a área),
  `ViewportBeyondArea` (a borda do viewport do guest passa 64 px ou mais da área: pega o caso em
  que todos os anexos estão errados com 512x256), `MixedSizes` (há anexos maiores e um limitador),
  `Ok`.
- Relato: `GPU: area check: ...` com área, override, hashes de VS/PS, `mrt_mask` e, por anexo,
  endereço do guest, formato, tamanho nativo e vinculado, mip e amostras. Sem repetir a mesma
  assinatura, no máximo 16 relatos e depois uma linha avisando que o resto foi omitido.
  `std::fflush(stdout)` depois de cada linha. Durante uma captura, a linha vai também para
  `FrameCapture::Note`.
- Linha de início `GPU: area check on (issue #121 test build)` na criação do rasterizador, para
  provar que o detector rodou mesmo sem nenhum relato.

### 2.3 Captura automática (sempre)

Uma vez por execução: `FrameCapture::Request()` e `VideoCore::RequestScreenshot(GameOnly)` no
primeiro veredicto grave (`AreaExceeds`, `AreaBelowAll`, `ViewportBeyondArea`, ou `MixedSizes`
num passe de cena: 5 ou mais cores e depth, o mesmo predicado de `scene_started`). Sem veredicto
grave, a captura sai no passe de cena número 3000 (uns 30 a 60 s de 3D), para que todo teste
traga pelo menos um censo de um quadro 3D.

### 2.4 Log do ramo de `CopyColorAndDepth` (sempre)

Header puro `renderer_vulkan/vk_overlap_copy.h` com `Choose(...)` (a mesma lógica dos ramos de
`vk_runtime.cpp` 534-569: `copy`, `via-buffer`, `color-to-ms-depth`, `unimplemented`), teste
`tests/test_overlap_copy.cpp`, alvo `overlap-copy-test`. `CopyColorAndDepth` vira um `switch`
sobre `Choose`, com um parâmetro `reason` (`depth-overlap` em `texture_cache.cpp:663`,
`compute-copy` em `vk_rasterizer.cpp:2041`). Até 32 linhas
`GPU: overlap copy N/32 <ramo> (<motivo>): <src> -> <dst> [size-mismatch]`, depois uma linha de
limite com a contagem por ramo.

### 2.5 Testes por variável: `GOW3_STATE_PROBE`

Header `renderer_vulkan/vk_state_probe.h` no padrão de `vk_amd_paths.h` (tokens separados por
vírgula ou espaço, `all`/`1` liga tudo, tokens desconhecidos ignorados), teste
`tests/test_state_probe.cpp`, alvo `state-probe-test`. Marcador `GPU: state probe on: <tokens>`
na criação do rasterizador.

- `viewport`: grava viewport e scissor de novo dentro do closure do draw, depois do
  `bindPipeline`, nos dois caminhos de draw (`DrawRecord` e `DrawIndirectRecord`). Testa um driver
  que perde o estado dinâmico de viewport/scissor através do `vkCmdBeginRendering` ou do bind.
- `depthstruct`: passa `&depth_stencil_info` mesmo com EDS3 (`vk_graphics_pipeline.cpp` 427-428).
  Testa um driver que trata mal `pDepthStencilState` nulo.
- `depthreemit`: grava de novo todo o estado dinâmico de profundidade e estêncil dentro do closure
  do draw, depois do `bindPipeline`.

`GOW3_FULL_BARRIERS=1` (já existe) é a quarta variação. Deixar o estado de profundidade inteiro
estático no pipeline foi descartado neste build: mexe na chave do pipeline e nas versões do cache
(cerca de 6 arquivos).

### 2.6 Script para o usuário

`packaging/windows/issue121-test.bat` (ASCII, CRLF por `.gitattributes`: `*.bat text eol=crlf`),
copiado para a raiz do zip por `packaging/windows/package.sh`. Menu com as execuções A
(diagnóstico), B (`viewport`), C (`depthstruct`), D (`depthreemit`), E (`GOW3_FULL_BARRIERS=1`),
uma opção para rodar todas em seguida e uma para zipar. Cada execução:

- liga `GOW3_GPU_LOG=info`, `GOW3_FRAME_CAPTURE_KEY=1`, `GOW3_CAPTURE_DIR=<pasta da execução>\captures`
  e a variável da variação;
- roda `God of War III.exe --play` e espera, com a saída em `console.log`;
- copia `last_run.log`, as capturas de tela novas e o `gow3.ini` da pasta de saves (lida de
  `%APPDATA%\gow3-launcher\settings.json`, `user_dir`, com `<pasta do jogo>\user` como padrão);
- grava um `summary.txt` com as linhas `GPU:`, `Frame capture:` e `Screenshot:`.

O zip vai para a Área de Trabalho (`[Environment]::GetFolderPath('Desktop')`), com GPU e versão do
driver do Windows em `system.txt`. `T121_GAME` e `T121_DESKTOP` existem só para o teste
automatizado do script. O script não mexe em `GOW3_USER_DIR`, `GOW3_DATA_DIR` nem
`GOW3_GPU_USER_DIR`.

Testes: `tests/test_issue121_bat.py` (o zip leva o script; ASCII sem LF solto; atributo
`eol=crlf`; toda variável `GOW3_*` do script existe no código; no Windows, execução com um jogo
falso conferindo as variáveis por execução e o zip). `smoke-test.ps1` passa a recusar `.bat` não
ASCII ou com LF solto.

## 3. Ordem

1. Este plano.
2. Linha do driver (2.1).
3. Log de `CopyColorAndDepth` (2.4).
4. Verificação da área e captura automática (2.2, 2.3).
5. `GOW3_STATE_PROBE` (2.5).
6. Script, empacotamento e testes (2.6).
7. Build completo, todos os testes C++ novos e antigos, testes Python, revisão do diff.
8. Push de `worktree-issue121-test`, `gh workflow run windows-zip.yml --ref worktree-issue121-test
   -f tag=windows-v2.3.0-issue121-test1`, notas da release editadas depois (pre-release, nunca
   latest). Mudanças depois disso saem como `issue121-test2`.

## 4. Como ler o resultado

- Linha `GPU: area check:` num passe de cena com área 512x256 ou anexo 512x256: o port entrega o
  tamanho errado. O relato diz qual imagem e em que endereço; a correção vai para o texture cache.
- Nenhum relato, captura com passes de cena de tela inteira e retângulo visível: o driver recorta
  sozinho. Uma variação que faz o retângulo sumir aponta o estado; nenhuma, só atualizar o driver.
- `GPU: overlap copy ... size-mismatch` envolvendo a profundidade da cena: a cópia por
  sobreposição está na cadeia.
