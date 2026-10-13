# Corrupção gráfica na Intel Arc (B580, Arc G3 / B390)

Diagnóstico de 2026-10-12 e 2026-10-13, feito por leitura do código, dos relatos e dos logs.
Nenhuma placa Intel está disponível localmente (a máquina de desenvolvimento tem uma GTX 1650),
então a confirmação depende de quem reportou. Este documento registra o que foi apurado, o que
foi refutado e o que a PR entrega. A investigação das linhas pretas nas RX 6600 está em
`2026-10-12-gpu-corruption-rdna2.md`.

Issues: [#39](https://github.com/FelipePrado0/gow3_pc/issues/39) (Arc B580, canônica),
[#28](https://github.com/FelipePrado0/gow3_pc/issues/28) (Arc G3 Extreme / B390, MSI Claw 8);
shadPS4 eltutz: cuesta4/shadPS4#11.

## 1. Sintoma

| | Intel Arc |
|---|---|
| Placas | Arc B580 (discreta, driver 32.0.101.9033 WHQL), Arc B390 integrada do Panther Lake (driver 101.9034) |
| Aparência | A cena 3D inteira retalhada em faixas horizontais de cerca de 32 px; dentro de cada faixa, tiles de cerca de 64 px deslocados. HUD, Scaleform e vídeos H.264 intactos. Começa já no menu principal. |
| Comportamento | Determinístico: aparece em toda execução, desde o menu. |
| Em outros builds | Igual no shadPS4 eltutz. HD 630 (Gen9, #35) mostra outro defeito (3D preto ou só azul). A770 (#63) perde o device: sintoma diferente. |

Logs dos reporters: o B580 reporta `subgroup size 32`; a B390 também (`Physical device subgroup
size 32`), Vulkan 1.4.356, sem `VK_KHR_pipeline_binary`. Nenhum tem aviso relevante do
`Render.Vulkan` além de formatos sem suporte com fallback (`R8Srgb`, `R8G8Srgb`,
`D16UnormS8Uint`, `R5G5B5A1`, `B4G4R4A4`, `A2B10G10R10Snorm/Sint`) e `Unimplemented
IT_SET_PREDICATION` no B580. Execuções saudáveis: nenhum erro, nenhum device lost.

## 2. Fatos apurados

- O quadro do jogo (docs/upscaler.md): pré-passe de profundidade, luz e sombras, materiais,
  transparências, pós com pirâmide de bloom em meia resolução, combine da pós em **compute**
  (hash `9a9cf8a9`) para a cor da cena RGBA8 sRGB, HUD por cima na mesma imagem.
- Só a Intel fica sem as duas proteções que os shaders wave64 do PS4 têm nos outros hosts:
  - `needs_lds_barriers` liga só para NVIDIA e KosmicKrisp
    (`gpu/shadps4/video_core/renderer_vulkan/vk_pipeline_cache.cpp`). O passe
    `shared_memory_barrier_pass.cpp` retorna cedo sem a flag. Igual no upstream.
  - O pin de subgrupo em 64 (`IsSubgroupSize64Supported()`) só existe onde o máximo é 64. Na
    Intel o máximo é 32, e os módulos são SPIR-V 1.6, então o driver pode compilar compute em
    SIMD16. O lowering de wave64 (`lower_wave64_pass.cpp`) assume 32 lanes fixas (`>> 5`,
    `& 31`) e só roda para compute.
- O pacote de release não inclui cache de shaders; cada reporter compila o próprio.
- O port só grava linhas de GPU em nível Warning no `user\last_run.log`
  (`gpu/shim/common/logging/log.cpp`); `GOW3_GPU_LOG=info` libera. Marcadores para testers
  precisam ser `std::printf("GPU: ...")`.

## 3. Hipóteses

Veredictos de três verificadores adversariais por hipótese em 2026-10-12 (sobrevive com 2 de 3),
seguidos do resultado do teste em hardware em 2026-10-13.

### A-1: workgroups de 64 threads com LDS rodam sem barreira

No GCN um workgroup de 64 threads é uma wave só, em lockstep, e o compilador do PS4 omite o
`s_barrier` entre `ds_write` e `ds_read`. Na Intel o workgroup vira 2 a 4 subgrupos e a leitura
pode preceder a escrita. Ressalvas registradas na época: não há dump de shader que prove que algum
compute do jogo tem workgroup de 64, LDS e nenhum `s_barrier`; o padrão (tiles coerentes
deslocados, névoa e partículas limpas por cima) lembra mais erro de endereçamento; a PR upstream
#1801 que criou o passe causou device lost numa Iris Xe em 2024, provavelmente o motivo da flag
ficar só na NVIDIA.

### A-2: lowering de wave64 assume 32 lanes

`profile.subgroup_size` é o `subgroupSize` reportado (32), não a largura real. Em SIMD16, dois
subgrupos gravam no mesmo slot do scratch do ballot e `readlane` com índice de 16 em diante fica
indefinido. O Xe2 não tem SIMD8; o mínimo é 16. Dawn (Chromium) registrou o mesmo problema:
drivers Intel escolhiam 8, 16 ou 32 lanes e quebravam compute shaders com layout dependente do
tamanho do subgrupo.

### Resultado em hardware (2026-10-13): A-1 e A-2 refutadas como causa

O build de teste `windows-v2.1.6-intel-test1` (commit `7a9e0a7`) ligava as barreiras de LDS e
fixava o compute em 32 lanes em todo host Intel, sem variável nenhuma.

- **B580 (xwesalemao, #39):** "the bug persists in the test version for Intel", corrompido desde
  o menu. O `last_run.log` anexado prova que era o build da branch: as linhas
  `vk_instance.cpp:308 ... Extension ... unavailable` e `vk_instance.cpp:859
  CollectImageFormatInfo` só existem nos binários da branch (a `main`, a 2.1.6 e a 2.1.7 têm
  `:304` e `:855`; o build 3 em diante tem `:309`). As barreiras de LDS estavam ativas. O pin de
  32 só se aplica quando o driver reporta faixa variável (`min < max`) e aceita pin em compute;
  sem `GOW3_GPU_LOG=info` o log não mostra a faixa, mas se o B580 reporta 32 fixo o SIMD16 nem
  é possível, e A-2 cai de qualquer forma.
- **B390 (yisus230291, #28):** os dois logs enviados (23:39 e 01:09) são do binário stock
  (`vk_instance.cpp:285 Physical device subgroup size 32` sem a faixa, `:304`): o build de
  teste não foi executado nessa máquina. O "não funcionou" dele não informa nada.
- O xwesalemao também publicou capturas RenderDoc (pasta no mega.nz linkada em #39, 4 capturas,
  3 com erros de API). Ainda não analisadas: exigem o cliente do Mega ou upload em outro lugar.

Com as duas hipóteses caindo, a causa está em outro ponto do núcleo compartilhado (tiling,
formato, alias, endereçamento de imagem) ou no driver. O padrão de faixas de 32 px com tiles de
64 px deslocados é compatível com erro de detile/endereçamento de uma imagem tiled, o que uma
captura RenderDoc do primeiro evento retalhado decide.

### Refutadas por leitura (2026-10-12)

- Alinhamento zero nos staging do tiling (`tile_manager.cpp`): todo `guest_size` tiled é
  múltiplo de 64 ou 256.
- Formato sRGB em vez de UNORM da cor da cena (HD 630): decidido pelo primeiro descritor que
  cria a imagem, não pelo fornecedor. Vale olhar no log do HD 630 se
  `image format VK_FORMAT_R8G8B8A8_SRGB ... is not supported` aparece (`image.cpp` remove o uso
  STORAGE e a view UNORM do compute fica inválida).
- Concorrência do port: o sintoma reproduz no eltutz, e nenhum recurso tem ramo por fornecedor.

## 4. O que esta PR entrega

Como as duas hipóteses não se confirmaram, **nada muda por padrão**: a política de subgrupo e as
barreiras de LDS ficam exatamente como no shadPS4 (`vk_subgroup_policy.h`, com teste
`tests/test_subgroup_policy.cpp`, alvo `subgroup-policy-test`). O que entra é ferramenta de
teste para quem tem a placa:

- `GOW3_SUBGROUP_PIN=all|allowed|none`. `all` (padrão) é o comportamento atual: pin de 64 em todo
  estágio num host de 64 lanes (AMD), nada nos outros. `allowed` fixa só os estágios que o driver
  lista em `requiredSubgroupSizeStages`, na largura para a qual os shaders foram compilados: 64
  na AMD (só compute), 32 na Intel (compute, se a faixa for variável). `none` não fixa nada.
- `GOW3_LDS_BARRIERS=0|1` força as barreiras de LDS no recompilador. Os módulos recompilados
  ficam no cache de shaders sem essa flag na chave: mudar o valor exige **Clear shader cache**.
- O log passa a imprimir a faixa de subgrupo e os estágios que aceitam pin (`Physical device
  subgroup size N (min to max, pinnable stages {...})`, nível Info) e, em qualquer nível, a
  política escolhida: `GPU: subgroup policy: compute pinned to N, fragment pinned to N, LDS
  barriers on|off (...)`. Essa linha identifica o build e a configuração em qualquer log.
- `ShaderBinaryVersion` continua 7: nenhum usuário recompila o cache por causa desta PR.

## 5. Próximos passos

1. **Capturas RenderDoc do B580** (xwesalemao, #39): o primeiro evento cuja saída já sai
   retalhada decide. Dispatch de compute com `LocalSize` 64, variáveis Workgroup e sem
   `OpControlBarrier` reabriria A-1; draw, cópia ou detile antes de qualquer compute aponta para
   tiling ou formato. Pedir upload num lugar acessível (anexo do GitHub até 25 MB, ou gist do
   `last_run.log` com `GOW3_GPU_LOG=info` junto).
2. **B390 (yisus230291):** rodar um build desta branch pelo `teste.bat` abaixo e anexar o
   `last_run.log`; a linha `GPU: subgroup policy:` confirma o build.
3. Com `GOW3_GPU_LOG=info` no B580: a faixa de subgrupo (`min to max`) e `pinnable stages` da
   Intel, que o log atual não tem.
4. `GOW3_DUMP_SHADERS=1` numa máquina qualquer lista `LocalSize`, `shared_memory_size` e
   ballot/readlane por pipeline: responde se algum compute antes do passe translúcido tem
   workgroup de 64, LDS e nenhum `s_barrier`.
5. Conferir e portar do upstream, se fizer sentido: #5310 (barreiras de storage buffer quando
   subgroup != 64), #5285 (subgroup barrier de LDS) e #5116 (ballot e MBCNT lowered em par).
   Citados pelos agentes, não conferidos.

## 6. Como testar (Intel Arc)

Pasta nova, **Clear shader cache** antes de cada execução, jogo aberto por um `teste.bat` ao
lado de `God of War III.exe`:

```bat
@echo off
set GOW3_GPU_LOG=info
set GOW3_SUBGROUP_PIN=allowed
set GOW3_LDS_BARRIERS=1
start "" "God of War III.exe"
```

Execuções úteis, uma de cada vez: (1) sem as duas linhas `set GOW3_...` (padrão, confirma o
sintoma e grava a faixa de subgrupo); (2) só `GOW3_LDS_BARRIERS=1`; (3) só
`GOW3_SUBGROUP_PIN=allowed`; (4) as duas. Em cada uma: anexar `user\last_run.log` e dizer se a
cena mudou. As duas últimas juntas equivalem ao build de teste 1, que já falhou no B580; valem
como controle para o B390 e para outros drivers.

## 7. Perguntas em aberto

1. Qual é o primeiro evento retalhado numa captura RenderDoc do B580: draw, dispatch, cópia?
2. Na Intel, o padrão é estável ou cintila entre frames?
3. O que explica o HD 630 (#35, 3D preto ou azul) e o device lost da A770 (#63)? São sintomas
   diferentes, possivelmente causas diferentes.
4. A faixa de subgrupo reportada pelo B580 (`min to max`) e os estágios que aceitam pin.
