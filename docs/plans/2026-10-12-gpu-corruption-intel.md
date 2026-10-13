# Corrupção gráfica na Intel Arc (A750, B580, 140V, B390)

Diagnóstico de 2026-10-12 e 2026-10-13, feito por leitura do código, dos relatos e dos logs, mais
os builds de teste rodados por quem tem a placa. Nenhuma placa Intel está disponível localmente (a
máquina de desenvolvimento tem uma GTX 1650). Este documento registra o que foi apurado, o que foi
refutado e o que a PR entrega. A investigação das linhas pretas nas RX 6600 está na PR separada da
AMD (`docs/plans/2026-10-12-gpu-corruption-rdna2.md` naquela branch).

Issues com o mesmo sintoma: [#28](https://github.com/FelipePrado0/gow3_pc/issues/28) (Arc G3
Extreme / B390, MSI Claw 8; a issue aberta que acompanha o caso),
[#39](https://github.com/FelipePrado0/gow3_pc/issues/39) (Arc B580; o relato com o histórico dos
builds de teste e as capturas RenderDoc, fechada como duplicada em 2026-10-13, 03:33 UTC),
[#96](https://github.com/FelipePrado0/gow3_pc/issues/96) (Arc 140V, Lunar Lake, MSI Claw 8 AI+;
nos comentários, Arc G3 Extreme no Onexplayer 3 e MSI Claw 8 EX) e
[#99](https://github.com/FelipePrado0/gow3_pc/issues/99) (Arc A750, Alchemist), as duas também
fechadas como duplicadas. Intel com outro
sintoma: [#35](https://github.com/FelipePrado0/gow3_pc/issues/35) (HD 630, Gen9: 3D preto ou só
azul), [#63](https://github.com/FelipePrado0/gow3_pc/issues/63) (A770: device lost).
[#69](https://github.com/FelipePrado0/gow3_pc/issues/69) (iGPU Xe-LP, Core i5) não relata
corrupção, mas as capturas são de cutscene: não serve de evidência nem a favor nem contra.
shadPS4 eltutz: cuesta4/shadPS4#11.

## 1. Sintoma

| | Intel Arc |
|---|---|
| Placas | Arc A750 (Alchemist, driver 101.9150), Arc B580 (Xe2 discreta, driver 32.0.101.9033 WHQL), Arc 140V (Xe2 integrada, Lunar Lake, driver 101.9034), Arc B390 (Xe3 integrada, Panther Lake, driver 101.9034, Vulkan 1.4.356). Cobre Xe-HPG, Xe2 e Xe3: não é só Xe2. |
| Aparência | A cena 3D inteira retalhada em faixas horizontais com período de **128 px** (bordas em y≈130, 255, 385, 510, 640, 770, 895 a 1080p); dentro delas, sub-faixas de 32 px e blocos de cerca de 64 px deslocados horizontalmente. Na A750 aparecem também colunas finas alternadas. HUD, Scaleform e vídeos H.264 intactos. Começa já no menu principal. Padrão idêntico nas três capturas comparadas (B580, A750, 140V). |
| Comportamento | Determinístico: aparece em toda execução, desde o menu. No B580 o reporter viu o clock da GPU preso em 400 MHz durante o jogo. |
| Em outros builds | Igual no shadPS4 eltutz. HD 630 (Gen9, #35) mostra outro defeito (3D preto ou só azul). A770 (#63) perde o device: sintoma diferente. |

Logs dos reporters: o B580 reporta `subgroup size 32`; a B390 reporta 32 lanes, faixa de 16 a 32,
pin aceito em `{ Compute | TaskEXT | MeshEXT }`, sem `VK_KHR_pipeline_binary`. Nenhum tem aviso
relevante do `Render.Vulkan` além de formatos sem suporte com fallback (`R8Srgb`, `R8G8Srgb`,
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
  precisam ser `std::printf("GPU: ...")`. Só o `Play God of War III.exe` (`--play`) grava o
  arquivo; o botão PLAY do launcher passa a gravar com a correção que está na PR da AMD.

### Fallbacks que só a Intel toma

Não há `IsIntel` nem `0x8086` no código. As únicas decisões por fornecedor são as listas de
NVIDIA/KosmicKrisp e os gates de extensão. São três os caminhos que a Intel toma e NVIDIA/AMD não:

- **`VK_KHR_maintenance8` ausente:** `Runtime::CopyColorAndDepth`
  (`gpu/shadps4/video_core/renderer_vulkan/vk_runtime.cpp:534-545`) copia depth↔color por
  `CopyImageWithBuffer`: download para um buffer de 128 MB e upload, com `bufferRowLength=0`,
  aspecto depth sem stencil e o extent do src aplicado ao dst. É chamado de
  `texture_cache.cpp:593` (overlap depth↔color recriado) e de `vk_rasterizer.cpp:1980` (cópia de
  imagem por compute reconhecida).
- **`VK_EXT_attachment_feedback_loop_dynamic_state` ausente:** as duas extensões de feedback loop
  ficam fora (`vk_instance.cpp:358-366`), e `vk_rasterizer.cpp:2423-2430` e `:2800-2808` usam
  `eGeneral` em vez de `eAttachmentFeedbackLoopOptimalEXT`.
- **`VK_NV_framebuffer_mixed_samples` e `VK_AMD_mixed_attachment_samples` ausentes:**
  `vk_pipeline_cache.cpp:996-1004` força `color_samples = depth_samples`, e
  `vk_graphics_pipeline.cpp:305` fica sem `AttachmentSampleCountInfoAMD`.

O período de 128 px pesa: 128 linhas a 32 bpp é a altura de um tile Tile64 da Intel. O layout
interno do driver aparecer na imagem é o que acontece quando a memória de uma VkImage é lida por
um caminho que ignora o layout (cópia por buffer, alias, bug de driver). Um macro tile GCN de
32 bpp não tem 128 linhas, o que afasta erro de detile do lado do PS4.

Fora dos fallbacks, a Intel expõe `VK_EXT_shader_stencil_export` (caminho nativo, igual ao da
AMD, que a NVIDIA não tem) e pode ter bug em extensão que expõe
(`VK_KHR_workgroup_memory_explicit_layout`, `VK_EXT_image_2d_view_of_3d`,
`VK_EXT_depth_clip_control`, ...).

### Qual build gerou um log

Os números de linha dos avisos do `vk_instance.cpp` mais a presença e o formato da linha
`GPU: subgroup policy:` identificam o binário mesmo sem `GOW3_GPU_LOG=info`. A coluna do
`CollectImageFormatInfo` é o aviso `Surface format ... is not fully supported`; o aviso `Depth
format ...` sai 10 linhas depois (`:855`, `:865`, `:869`, `:888`, `:890`, na mesma ordem) e, só
ele, confunde a 2.1.4/2.1.5 com a 2.1.6:

| Binário | `Extension ... unavailable` | `Surface format ...` | Linha `GPU: subgroup policy:` |
|---|---|---|---|
| 2.1.4, 2.1.5 | `:294` | `:845` | não |
| 2.1.6, 2.1.7, `main` | `:304` | `:855` | não |
| `windows-v2.1.6-intel-test1` (`7a9e0a7`) | `:308` | `:859` | não (era LOG_INFO) |
| `windows-v2.1.6-test2` | `:309` | `:878` | sim, em qualquer nível |
| `windows-v2.1.6-test3`, `windows-v2.1.7-test3`, `-test4` | `:309` | `:880` | sim, em qualquer nível |
| esta branch (PR #105) | `:308` | `:859` | sim, terminando em `GOW3_SUBGROUP_PIN=..., mode ...` |

Esta branch e o intel-test1 têm os mesmos números: só a linha de política (ausente no test1,
presente e com `mode` aqui) os separa. Nos builds de teste 1 a 4 a Intel rodava com barreiras de
LDS ligadas sem variável nenhuma (só `GOW3_LDS_BARRIERS=0` as desligava, e nenhum reporter a
usou) e com o compute fixado em 32 lanes quando o dispositivo reporta faixa variável e aceita pin
em compute, como a B390 reporta.

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

- **B580 (xwesalemao, #39, 02:05 UTC):** "The graphics are already corrupted in the game menu"
  (uma revisão anterior do comentário dizia "the bug persists in the test version for Intel").
  O `last_run.log` anexado é do `windows-v2.1.6-intel-test1`: `:308`/`:859` e nenhuma linha
  `GPU: subgroup policy:` (tabela acima). Barreiras de LDS ativas, o que refuta A-1 nessa placa.
  O pin de 32 dependia da faixa e dos estágios que o B580 reporta, e o log (sem
  `GOW3_GPU_LOG=info`) não mostra nenhum dos dois: no B580 o pin fica sem confirmação, e A-2 só
  cai pela B390 abaixo (outra geração, Panther Lake integrada, outro build de driver).
- **B390 (yisus230291, #28, 02:40 UTC):** "exactly the same as the other intel arc person, even
  the main menu looks identical". O log (`last_run.txt`) é de um build 3 ou 4
  (`windows-v2.1.6-test3`, `windows-v2.1.7-test3` ou `-test4`: `:309`/`:880`) e traz a política
  aplicada: `GPU: subgroup policy: compute pinned to 32, fragment pinned to 0,
  LDS barriers on (device 32 lanes, 16 to 32, pinnable stages { Compute | TaskEXT | MeshEXT },
  GOW3_SUBGROUP_PIN=all)`. Pin de 32 em compute e barreiras de LDS, e a cena continua retalhada.
  Os dois logs anteriores dele não eram do build de teste: o de 23:39 UTC é da 2.1.5 ou anterior
  (`:294`/`:845`) e o de 01:09 UTC é da 2.1.6/2.1.7 (`:285 Physical device subgroup size 32`
  sem faixa, `:304`).
- O xwesalemao também publicou capturas RenderDoc (pasta no mega.nz linkada em #39, 4 capturas,
  3 com erros de API). Ainda não analisadas: exigem o cliente do Mega ou upload em outro lugar.

Com barreiras ligadas (B580 e B390) e o compute comprovadamente fixado em 32 lanes (B390) a
corrupção é idêntica, então as duas hipóteses caem. A causa está em outro ponto do núcleo
compartilhado (tiling, formato, alias, endereçamento de imagem) ou no driver. O período de 128 px
aponta para o layout Tile64 da Intel lido por um caminho que o ignora (seção 2, "Fallbacks que só
a Intel toma"), e uma captura RenderDoc do primeiro evento retalhado decide. Os três fallbacks
foram descartados em seguida na GTX 1650 como erro de lógica do port (item abaixo); isso não
exclui bug do driver Intel no mesmo caminho (cópia depth↔color por buffer em
`CopyImageWithBuffer`).

### Resultado na GTX 1650 (2026-10-13): H-B refutada como erro de lógica do port

Hipóteses em teste: **H-B**, um dos três fallbacks que só a Intel toma (seção 2) produz o
retalho; **H-C**, uma extensão que a Intel expõe se comporta mal no driver dela, ou a causa está em
algo que a NVIDIA não consegue imitar.

Máquina e build: GTX 1650, driver 617.14, Vulkan 1.4.351, Windows 11; dump CUSA01623 01.02 com
os patches do repositório; esta branch no commit `0385940`, executada por `run.py` com
`GOW3_GPU_LOG=info` e `user/` novo a cada execução (cache de pipelines limpo). Capturas de tela
aos 50, 70 e 90 s (menu principal, com a cena 3D do Kratos ao fundo). O `run.py` não grava
`user\last_run.log`: as linhas abaixo vêm da saída de cada execução.

- **Execução 0, sem variável (baseline):** menu correto aos 70 e 90 s (Kratos e cenário
  íntegros, sem faixas). Extensões opcionais na 1650:
  - presentes: `VK_KHR_maintenance5`, `VK_KHR_pipeline_binary`, `VK_KHR_maintenance8`,
    `VK_EXT_attachment_feedback_loop_layout`, `VK_EXT_attachment_feedback_loop_dynamic_state`,
    `VK_EXT_depth_range_unrestricted`, `VK_EXT_extended_dynamic_state3`,
    `VK_EXT_custom_border_color`, `VK_EXT_depth_clip_control`, `VK_EXT_depth_clip_enable`,
    `VK_EXT_vertex_input_dynamic_state`, `VK_EXT_primitive_topology_list_restart`,
    `VK_KHR_fragment_shader_barycentric`, `VK_EXT_provoking_vertex`,
    `VK_NV_framebuffer_mixed_samples`, `VK_EXT_shader_atomic_float`,
    `VK_KHR_workgroup_memory_explicit_layout`, `VK_EXT_image_2d_view_of_3d`,
    `VK_EXT_image_view_min_lod`, `VK_EXT_memory_budget`, `VK_KHR_compute_shader_derivatives`,
    `VK_KHR_shader_clock`;
  - ausentes: `VK_AMD_shader_explicit_vertex_parameter`, `VK_EXT_shader_stencil_export`,
    `VK_AMD_shader_image_load_store_lod`, `VK_AMD_gcn_shader`, `VK_AMD_shader_trinary_minmax`,
    `VK_AMD_mixed_attachment_samples`, `VK_EXT_shader_atomic_float2`.

  A 1650 tem as três extensões cuja falta define os fallbacks Intel-only e não tem stencil export
  nem `atomic_float2`, que a Intel tem.
- **Execução 1,
  `GOW3_VK_DISABLE_EXT=VK_KHR_maintenance8,VK_EXT_attachment_feedback_loop_dynamic_state,VK_NV_framebuffer_mixed_samples`:**

  ```
  GPU: device NVIDIA GeForce GTX 1650, driver NvidiaProprietary 617.14.0.0, Vulkan 1.4.351; extensions off: VK_KHR_maintenance8, VK_EXT_attachment_feedback_loop_dynamic_state, VK_NV_framebuffer_mixed_samples (GOW3_VK_DISABLE_EXT=VK_KHR_maintenance8,VK_EXT_attachment_feedback_loop_dynamic_state,VK_NV_framebuffer_mixed_samples)
  GPU: optional extensions unavailable: VK_KHR_maintenance8, VK_EXT_attachment_feedback_loop_layout, VK_EXT_attachment_feedback_loop_dynamic_state, VK_AMD_shader_explicit_vertex_parameter, VK_EXT_shader_stencil_export, VK_AMD_shader_image_load_store_lod, VK_AMD_gcn_shader, VK_AMD_shader_trinary_minmax, VK_NV_framebuffer_mixed_samples, VK_AMD_mixed_attachment_samples, VK_EXT_shader_atomic_float2; ignored names: none
  ```

  No stderr, `vk_instance.cpp:322 Extension VK_KHR_maintenance8 disabled by GOW3_VK_DISABLE_EXT`
  (idem para as outras duas) e `vk_pipeline_serialization.cpp:872 CheckCacheProfile: Pipeline
  cache skipped: GOW3_VK_DISABLE_EXT is active`. A `_layout` aparece em `unavailable` porque o
  `pop_back` a removeu junto com a `_dynamic_state`.

  Resultado: menu principal correto aos 70 s (Kratos e cenário íntegros, sem faixas), idêntico à
  execução 0. Os três fallbacks (cópia depth↔color por buffer, layout `eGeneral` sem feedback
  loop, `color_samples = depth_samples`) rodaram na NVIDIA sem produzir o retalho que a Intel
  mostra desde o menu.

**Veredito: H-B refutada como erro de lógica do port** nos três fallbacks, que são os únicos
caminhos de código que só a Intel toma: a lógica deles está certa quando o driver da NVIDIA a
executa. Isso não exclui bug do driver Intel no mesmo caminho (cópia depth↔color por buffer em
`CopyImageWithBuffer`): uma cópia que ignore o layout Tile64 imprimiria tiles de 128 linhas
(seção 2), e a NVIDIA não reproduz isso. Resta também H-C: uma extensão que a Intel expõe se comportando mal, ou algo que a NVIDIA não
consegue imitar (`VK_EXT_shader_stencil_export`, `VK_EXT_shader_atomic_float2`). Só hardware
Intel bissecciona isso (seção 6).

- **Execução 2, `GOW3_VK_DISABLE_EXT=all` (20 opcionais desligadas na 1650;
  `VK_KHR_pipeline_binary` e `VK_EXT_attachment_feedback_loop_dynamic_state` nunca são pedidas
  depois que `VK_KHR_maintenance5` e `_layout` estão desligadas, por isso 20):** device criado,
  `window and Vulkan presenter ready`, `Pipeline cache skipped`, e o processo morreu sem
  mensagem logo depois da inicialização de troféus/save, antes do vídeo de abertura (685 linhas
  de stdout contra 845 na execução 1). Sem registro no Event Log. Enquanto a extensão
  responsável não for isolada, `all` não serve de oráculo. Bisseção da morte:

| Execução | Desligadas | Resultado |
|---|---|---|
| B | maintenance5, maintenance8, feedback_loop_layout, depth_range_unrestricted, extended_dynamic_state3, custom_border_color, depth_clip_control, depth_clip_enable, vertex_input_dynamic_state, primitive_topology_list_restart | morre no mesmo ponto (675 linhas) |
| B2 | custom_border_color, depth_clip_control, depth_clip_enable, vertex_input_dynamic_state, primitive_topology_list_restart | morre no mesmo ponto (684 linhas) |
| VI | vertex_input_dynamic_state | ok: vídeo de abertura aos 50 s (845 linhas) |
| DC | depth_clip_control, depth_clip_enable | falha antes do jogo: `cannot create 8128 MiB of guest memory (error 1450)`, processo anterior ainda liberando memória; repetida como DC2 |
| DC2, DC3 | depth_clip_control, depth_clip_enable | mesmo `error 1450`: commit livre do Windows em 7.087 MB (limite 35.053 MB) contra os 8.128 MiB que o jogo reserva; ambiente, não extensão |
| DC4 | idem, com `GOW3_DMEM_MB=5056` | o jogo com patches precisa do pool de 8.128 MiB: fault no código do jogo logo após `UserService initialized`; execução inválida |

Estado da bisseção (parada): a morte com `all` está em uma ou mais de
`VK_EXT_custom_border_color`, `VK_EXT_depth_clip_control`, `VK_EXT_depth_clip_enable` e
`VK_EXT_primitive_topology_list_restart` (B2 menos VI). As execuções param aqui por falta de commit
de memória do Windows na máquina de desenvolvimento; retomar quando houver 8,5 GB livres de commit.

### Log do shadPS4 eltutz no B580 (cuesta4/shadPS4#11)

Detilers criados: `Thin2DThin_32`, `Display2DThin_32`, `Depth2DThin64_32` e
`Thin1DThin_8/32/64/128`. O único aviso de GPU é `Unimplemented IT_SET_PREDICATION`, e faltam as
mesmas extensões que no port.

### Refutadas por leitura (2026-10-12)

- Alinhamento zero nos staging do tiling (`tile_manager.cpp`): todo `guest_size` tiled é
  múltiplo de 64 ou 256.
- Concorrência do port: o sintoma reproduz no eltutz, e nenhum recurso tem ramo por fornecedor.

## 4. O que esta PR entrega

Como as duas hipóteses não se confirmaram, **nada muda por padrão**: a política de subgrupo e as
barreiras de LDS ficam exatamente como no shadPS4 (`vk_subgroup_policy.h`, helper puro com teste
`tests/test_subgroup_policy.cpp`, alvo `subgroup-policy-test`). O que entra é ferramenta de
teste para quem tem a placa:

- `GOW3_SUBGROUP_PIN=all|allowed|none`. `all` (padrão) é o comportamento atual: pin de 64 em todo
  estágio num host de 64 lanes (AMD), nada nos outros. Na AMD isso fixa também vertex e fragment,
  que o driver não lista em `requiredSubgroupSizeStages`: uso inválido pela especificação
  (VUID-VkPipelineShaderStageCreateInfo-pNext-02755), que o shadPS4 mantém porque o driver AMD
  aceita na prática. `allowed` é a variante conforme: fixa só os estágios que o driver lista, na
  largura para a qual os shaders foram compilados: 64 na AMD (só compute), 32 na Intel (compute,
  se a faixa for variável). `none` não fixa nada.
- `GOW3_LDS_BARRIERS=0|1` força as barreiras de LDS no recompilador. A flag faz parte do
  `Shader::Profile` gravado com o cache de pipelines: ao mudar o valor, a próxima abertura detecta
  o perfil diferente, limpa e reconstrói o cache sozinha (`Pipeline cache isn't compatible with
  current system: rebuilding it` no log). Cada troca custa uma recompilação completa; o botão
  **Clear shader cache** só é necessário com cache arquivado.
- O log passa a imprimir a faixa de subgrupo e os estágios que aceitam pin (`Physical device
  subgroup size N (min to max, pinnable stages {...})`, nível Info) e, em qualquer nível, a
  política escolhida: `GPU: subgroup policy: compute pinned to N, fragment pinned to N, LDS
  barriers on|off (device N lanes, min to max, pinnable stages {...}, GOW3_SUBGROUP_PIN=<valor ou
  unset>, mode all|allowed|none)`. O `mode` é o que foi interpretado: `Allowed` ou um espaço a
  mais no `.bat` aparecem como `mode all`. Essa linha identifica o build e a configuração em
  qualquer log.
- `GOW3_VK_DISABLE_EXT=<nomes separados por vírgula>|all` desliga extensões opcionais do device
  na criação (`vk_instance.cpp`; parser em `vk_extension_policy.h`, com teste
  `tests/test_extension_policy.cpp`, alvo `extension-policy-test`). Os nomes são exatos
  (maiúsculas contam); `all` desliga toda opcional. A extensão desligada se comporta como
  ausente: o código toma o mesmo fallback (log `Extension X disabled by GOW3_VK_DISABLE_EXT`,
  nível Warning). As quatro obrigatórias (swapchain, push descriptor, vertex attribute divisor,
  robustness2) nunca são desligadas; as extensões que o DLSS pede também obedecem (sem elas o
  DLSS se desliga). Em qualquer nível de log saem duas linhas:
  `GPU: device <nome>, driver <id> <versão>, Vulkan <x.y.z>; extensions off: <lista ou none>
  (GOW3_VK_DISABLE_EXT=<valor ou unset>)` e `GPU: optional extensions unavailable: <lista ou
  none>; ignored names: <lista ou none>`. `unavailable` junta as ausentes no device, as
  desligadas e a `VK_EXT_attachment_feedback_loop_layout` quando falta a `_dynamic_state` (o
  `pop_back` remove a `_layout` junto); `ignored` são os nomes da variável que não tiveram efeito (desconhecidos,
  obrigatórios, ausentes no device ou nunca pedidos), o que pega erro de digitação no `.bat`.
  Enquanto a variável desliga alguma coisa, o cache de pipelines em disco e os pipeline binaries
  ficam fora da sessão (`Pipeline cache skipped: GOW3_VK_DISABLE_EXT is active`): extensões que
  não entram no `Shader::Profile` (maintenance8, feedback loop, mixed samples, vertex input
  dinâmico, dynamic state 3, ...) também moldam as chaves gravadas, que seriam inválidas sob
  outro conjunto. Nada é lido nem gravado; cada execução com a variável compila tudo de novo
  (travadas esperadas), e o cache normal volta intacto quando ela sai. O `Shader::Profile` não
  muda.
- `ShaderBinaryVersion` continua 7 e o `Shader::Profile` mantém o tamanho: nenhum usuário
  recompila o cache por causa desta PR.

## 5. Próximos passos

1. **Bisseção de extensões na Intel** com `GOW3_VK_DISABLE_EXT` (protocolo da seção 6). Com H-B
   refutada na GTX 1650 como erro de lógica do port nos três fallbacks (não exclui bug do driver
   Intel no mesmo caminho, a cópia depth↔color por buffer em `CopyImageWithBuffer`), é o teste
   que separa as extensões que a Intel expõe; pedir em #28.
2. **Terminar a bisseção do `all` na GTX 1650** entre `VK_EXT_custom_border_color`,
   `VK_EXT_depth_clip_control`, `VK_EXT_depth_clip_enable` e
   `VK_EXT_primitive_topology_list_restart` (precisa de 8,5 GB livres de commit) e corrigir o
   fallback que derruba o jogo. É bug do port/shadPS4 por si só, reproduzível sem placa Intel.
3. **Capturas RenderDoc do B580** (xwesalemao, #39): o primeiro evento cuja saída já sai
   retalhada decide. Dispatch de compute com `LocalSize` 64, variáveis Workgroup e sem
   `OpControlBarrier` reabriria A-1; draw, cópia ou detile antes de qualquer compute aponta para
   tiling ou formato. Pedir upload num lugar acessível (anexo do GitHub até 25 MB, ou gist do
   `last_run.log` com `GOW3_GPU_LOG=info` junto). O reporter disse que 3 das 4 capturas têm "API
   errors": pedir também o texto da aba Errors and Warnings do RenderDoc. Conferir também se o
   primeiro evento retalhado vem logo depois de um par `vkCmdCopyImageToBuffer` /
   `vkCmdCopyBufferToImage` de `CopyImageWithBuffer` (imagem depth → buffer → imagem de cor):
   seria o bug do driver Intel no fallback de maintenance8 que a GTX 1650 não consegue excluir.
4. **Clock preso em 400 MHz no B580:** confirmar com o Intel Graphics Software ou o Gerenciador
   de Tarefas durante o jogo. Não explica a corrupção (a B390 integrada tem o mesmo retalho), mas
   explica desempenho ruim e pode indicar o driver em estado de baixa potência.
5. Com `GOW3_GPU_LOG=info` no B580: a faixa de subgrupo (`min to max`) e `pinnable stages`, que
   o log atual não tem (a B390 reporta 16 a 32 e `{ Compute | TaskEXT | MeshEXT }`).
6. `GOW3_DUMP_SHADERS=1` numa máquina qualquer lista `LocalSize`, `shared_memory_size` e
   ballot/readlane por pipeline: responde se algum compute antes do passe translúcido tem
   workgroup de 64, LDS e nenhum `s_barrier`.
7. Conferir e portar do upstream, se fizer sentido: #5310 (barreiras de storage buffer quando
   subgroup != 64, aberta), #5285 (subgroup barrier de LDS, merged 2026-10-06) e #5116 (ballot e
   MBCNT lowered em par, merged 2026-09-25). Conferidos: nenhum cita Intel.
8. Upstream desde o commit vendorado `b4e7ae77` (2026-09-24), a conferir no próximo merge, fora
   desta PR: `7e77898` "video_core: Fix mip layout and detiling of tiled textures" (#5196),
   `995e37e` "Read CMask metadata" (#5046), `c679ce6` "texture_cache: initialize added
   depth-overlap subresources" (#5229), `fecfbed` "texture_cache: Avoid associating color images
   with stencil" (#5118) e `69b6041` "video_core: Link the stencil plane of DepthStencilCopy
   destinations" (#5296). Nenhum cita Intel.

## 6. Como testar (Intel Arc)

Pasta nova, com a pasta do jogo escolhida uma vez no launcher. Jogo aberto por um `teste.bat` ao
lado de `Play God of War III.exe` (é esse executável que grava o `user\last_run.log`; o botão
PLAY do launcher só grava com a correção da PR da AMD):

```bat
@echo off
set GOW3_GPU_LOG=info
set GOW3_SUBGROUP_PIN=allowed
set GOW3_LDS_BARRIERS=1
start "" "Play God of War III.exe"
```

Execuções úteis, uma de cada vez: (1) sem as duas linhas `set GOW3_...` (padrão, confirma o
sintoma e grava a faixa de subgrupo); (2) só `GOW3_LDS_BARRIERS=1`; (3) só
`GOW3_SUBGROUP_PIN=allowed`; (4) as duas, que equivale aos builds de teste 1 a 4 e já falhou no
B580 e na B390. Em cada uma: anexar `user\last_run.log` e dizer se a cena mudou. A linha `GPU:
subgroup policy:` confirma o build e a configuração; trocar `GOW3_LDS_BARRIERS` entre execuções
faz o cache de pipelines ser reconstruído na abertura seguinte.

### Bisseção de extensões (`GOW3_VK_DISABLE_EXT`)

Mesmo `teste.bat`, com `set GOW3_GPU_LOG=info`, sem as outras linhas `set GOW3_...` e com uma
linha `set GOW3_VK_DISABLE_EXT=<lista>` por execução. `all` não serve de oráculo: na GTX 1650 ele
derruba o jogo antes do vídeo de abertura (seção 3), e uma morte não diz nada sobre o retalho.
Execuções:

- (A) sem a variável: confirma o sintoma e grava a lista de extensões do device.
- (B) e (C): as duas metades das opcionais que a Intel expõe, **sem** as quatro candidatas da
  morte com `all`:
  - (B) `VK_KHR_workgroup_memory_explicit_layout,VK_EXT_shader_stencil_export,VK_EXT_image_2d_view_of_3d,VK_EXT_vertex_input_dynamic_state,VK_EXT_extended_dynamic_state3,VK_EXT_provoking_vertex,VK_EXT_image_view_min_lod,VK_EXT_depth_range_unrestricted`
  - (C) `VK_KHR_fragment_shader_barycentric,VK_EXT_shader_atomic_float,VK_EXT_shader_atomic_float2,VK_KHR_maintenance5,VK_EXT_memory_budget,VK_KHR_compute_shader_derivatives,VK_KHR_shader_clock`
- (D) só as quatro candidatas:
  `VK_EXT_custom_border_color,VK_EXT_depth_clip_control,VK_EXT_depth_clip_enable,VK_EXT_primitive_topology_list_restart`.
  Uma morte aqui é provavelmente a mesma da GTX 1650, atribuível ao crash conhecido, e não deve
  ser lida como "corrigiu"; se D chegar ao menu, vale o que a cena mostrar. Se D morrer, as quatro
  candidatas ficam sem veredito na Intel até esse crash ser corrigido (seção 5, item 2).

A execução que mudou a cena se divide ao meio de novo, e assim por diante até sobrar um nome. Em cada uma: anexar `user\last_run.log` e dizer se a cena mudou; a linha `GPU:
device ...; extensions off:` confirma o que foi desligado, e `ignored names` precisa sair `none`
(senão algum nome foi digitado errado). Cada execução compila os shaders de novo, porque o cache
fica fora da sessão.

Na GTX 1650 da máquina de desenvolvimento, a receita
`VK_KHR_maintenance8,VK_EXT_attachment_feedback_loop_dynamic_state,VK_NV_framebuffer_mixed_samples`
põe a NVIDIA nos três fallbacks que só a Intel toma (seção 2). Rodada em 2026-10-13, o menu saiu
correto (seção 3): os fallbacks não reproduzem o retalho.

Riscos registrados:

- `VK_KHR_maintenance5` desligada desliga também os pipeline binaries.
- `VK_AMD_shader_explicit_vertex_parameter` desligada na AMD faz o código pedir
  `VK_KHR_fragment_shader_barycentric` no lugar.
- `VK_KHR_fragment_shader_barycentric` desligada na NVIDIA liga
  `needs_integer_interpolation_fix` (o caminho das Pascal).
- A NVIDIA não ganha extensões que só a Intel expõe (`VK_EXT_shader_stencil_export`): bug de
  driver Intel dentro de uma extensão exposta só se bissecciona na Intel.
- `fsr4-bench` e os testes que linkam `gow3gpu` também leem a variável.
- Para a validação durante a bisseção: `setAttachmentFeedbackLoopEnableEXT` é gravado
  (`vk_scheduler.h:343`) sem `eAttachmentFeedbackLoopEnableEXT` nos dynamic states do pipeline
  (`vk_graphics_pipeline.cpp:155-176`). Se a validação acusar algo ali, vem do caminho padrão
  (extensão ligada), não da variável: com a extensão desligada a chamada nem é gravada.

## 7. Perguntas em aberto

1. Qual é o primeiro evento retalhado numa captura RenderDoc do B580: draw, dispatch, cópia?
   Ele vem logo depois de um par `vkCmdCopyImageToBuffer` / `vkCmdCopyBufferToImage` de
   `CopyImageWithBuffer` (imagem depth → buffer → imagem de cor)?
2. Na Intel, o padrão é estável ou cintila entre frames?
3. O que explica o HD 630 (#35, 3D preto ou azul) e o device lost da A770 (#63)? São sintomas
   diferentes, possivelmente causas diferentes.
4. Por que a cor da cena sai `R8G8B8A8Srgb` no HD 630 e `Unorm` na NVIDIA no mesmo frame, com os
   mesmos passes? A explicação anterior ("decidido pelo primeiro descritor que cria a imagem, não
   pelo fornecedor") não cobre uma diferença no mesmo frame, então saiu das refutadas. Vale olhar
   no log do HD 630 se `image format VK_FORMAT_R8G8B8A8_SRGB ... is not supported` aparece
   (`image.cpp` remove o uso STORAGE e a view UNORM do compute fica inválida).
5. A faixa de subgrupo e os estágios que aceitam pin no B580 (a B390 responde 16 a 32 e
   `{ Compute | TaskEXT | MeshEXT }`).
6. O clock de 400 MHz no B580 é do driver, do jogo ou da medição?
