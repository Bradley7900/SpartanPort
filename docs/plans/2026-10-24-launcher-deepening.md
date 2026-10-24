# Aprofundamento dos modules do launcher Rust: plano de implementação

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Aplicar os 11 candidatos de aprofundamento da spec ao `launcher-rust/`. O comportamento
fica igual, salvo as mudanças marcadas com ⚠ na spec.

**Architecture:** Um candidato por vez, de 01 a 11. Cada candidato troca modules shallow por um
module deep com interface pequena, e os testes migram para essa interface. Os bytes de paridade
com o Python ficam fixos o tempo todo.

**Tech Stack:** Rust (edition do crate), iced 0.14, `serde_json`, `indexmap`, `tempfile`; pytest
para os mapas de paridade.

**Spec:** `docs/specs/2026-10-24-launcher-deepening-design.md`. A spec é a fonte das interfaces
(§4, um bloco por candidato): este plano as cita em vez de copiá-las. Leia o bloco do candidato
antes de cada task.

## Global Constraints

- **Branch:** `rust-launcher`, direto. Sem push. Sem `git commit --amend`.
- **Diretórios:** comandos `cargo` rodam em `launcher-rust/`; comandos `py`/`git` rodam na raiz
  do repo.
- **Gate G** (fim de toda task, nesta ordem; tudo precisa passar):
  1. `cargo fmt --check`
  2. `cargo clippy --all-targets -- -D warnings`
  3. `cargo test`
  4. `py -3 -m pytest tests/test_launcher_parity_doc.py tests/test_runner_parity_doc.py tests/test_cutover_parity_doc.py -q`
- **Carga do PC:** o `~/.cargo/config.toml` já tem `jobs = 8` e `RUST_TEST_THREADS = 4`.
  - Um `cargo test` completo e um clippy por task.
  - Reruns focados: `cargo test --test <binário> <nome>` ou `cargo test --lib <nome>`.
  - Nunca rode build release, pacote ou script de empacotamento.
- **Dump real** (só onde a task manda):
  `SPARTAN_GAME_DIR="C:/Users/Lucas Emiliano/Documents/My Games/GOW III/CUSA01623" cargo test --test dump`.
- **Testes `#[cfg(unix)]`** (Tasks 23, 24 e 36): no Windows eles não compilam. Rode-os focados
  no WSL (`Ubuntu-24.04`), com um target separado para não misturar com o do Windows:
  `wsl -d Ubuntu-24.04 -- bash -lc 'cd "/mnt/c/Users/Lucas Emiliano/Documents/Projetos/gow3_pc/launcher-rust" && CARGO_TARGET_DIR=$HOME/sp-target cargo test --test <binário> <nome>'`.
  Se o WSL não tiver toolchain ou o build morrer, escreva no corpo do commit
  `unix tests not run locally` e siga.
- **Bytes intocáveis:** os esperados de `tests/runner_fixtures.rs`, `tests/dump.rs` e
  `tests/fixtures.rs` não mudam. Se um deles quebrar, a task está errada.
- **Mapas de paridade:** quem renomeia ou move um teste citado em
  `docs/specs/2026-10-2*-parity.md` atualiza a linha no mesmo commit (formato
  `caminho::módulo::nome`, checado por `tests/parity_doc.py`).
- **Fronteira:** `src/core` nunca usa `iced` nem `crate::runner` (`tests/core_boundary.rs`).
- **Python congelado:** nada em `launcher/`, `run.py`, `scripts/` ou `tools/*.py` muda.
- **Commits:**
  - um por task;
  - mensagem gerada com a skill `caveman-commit`, no estilo do repo (`Refactor(Launcher): …`,
    `Fix(Launcher): …`, `Test(Launcher): …`, `Docs(Launcher): …`);
  - nunca `Co-Authored-By`, nem menção a IA, agente ou assistente;
  - autor = usuário do git, sem `--author`.
- **Estilo:** comentários em inglês, curtos, no tom do arquivo vizinho. Nomes de teste em inglês
  descritivo (`a_thing_does_x`), como os existentes.

## Review Focus

1. **settings.json antigo, salvo pelo Python 2.6/2.7.** Load, set de uma chave e save mudam só
   essa chave. Nenhuma chave nova aparece (`launcher_effects` e `controls_device` são
   `written: false`). Teste na Task 3.
2. **Caminho do port com espaço** (`C:\Users\Lucas Emiliano\…`) depois que o `set_current_dir`
   sai. Um `SPARTAN_DATA_DIR` relativo continua caindo dentro do port. Teste na Task 12.
3. **Digitar a pasta do jogo letra a letra.** Cada tecla manda `Browsed`, e uma leitura velha de
   Mods/Trophies nunca sobrescreve a da pasta final. Teste na Task 21.
4. **Sem rede.** `list` falhando deixa o Updater em `Failed` com a mensagem, sem panic, e o
   PLAY continua. Teste na Task 33.
5. **`boot-linked.bin` corrompido.** O ISA rewrite falha com a mensagem e o runner segue sem a
   tabela (não fatal). Teste na Task 17.

---

## Candidato 01: catálogo de settings (spec §4 / 01)

### Task 1: catálogo, `Settings::{flag,text}` e `Store`

**Files:**
- Create: `launcher-rust/src/core/catalog.rs`, `launcher-rust/src/core/store.rs`
- Modify: `launcher-rust/src/core/mod.rs` (declarar `catalog` e `store`),
  `launcher-rust/src/core/settings.rs` (somar `flag` e `text`; `get_bool` fica até a Task 3)

**Interfaces:**
- Produces: `catalog::{File, Kind, Default, Env, Setting, SETTINGS, setting, run_default}`,
  `Settings::flag(&self, &str) -> bool`, `Settings::text(&self, &str) -> String`,
  `Store::{load, flag, text, json, set, replace, app, ini, ini_to_write}`, com as assinaturas da
  spec §4/01.
- Conteúdo da tabela:
  - uma linha `App` por chave de `app_defaults` (43 linhas, mesma ordem, `written: true`);
  - mais `launcher_effects` (Flag, default true) e `controls_device` (Text, default `""`),
    ambas `written: false`;
  - uma linha `Ini` por chave de `INI_DEFAULTS` (23), com `Kind::Flag` para as 8 de `INI_FLAGS`.
- O `env` de cada linha copia o nome que `core/env.rs` escreve hoje. O `absent` é o literal do
  run.py: `SPARTAN_MODS_ENABLED` → `"1"`, `SPARTAN_STREAM_MEMORY` → o default de
  `runner/mod.rs:621`, `SPARTAN_GLYPH_STYLE` → o default do runner.
- Regras de leitura:
  - truthiness App = `bool()` do Python: `null`, `false`, `0`, `0.0`, `""`, `[]` e `{}` são
    falsos; todo o resto é verdadeiro;
  - Ini é ligado quando `== "1"`;
  - chave ausente lê o default da tabela.

- [ ] **Step 1: Write the failing tests** (em `catalog.rs` e `store.rs`, `#[cfg(test)] mod tests`)

```rust
#[test]
fn every_key_is_described_once() {
    let mut seen = std::collections::HashSet::new();
    assert!(SETTINGS.iter().all(|s| seen.insert(s.key)));
    let app: Vec<_> = SETTINGS.iter().filter(|s| matches!(s.file, File::App) && s.written).map(|s| s.key).collect();
    let defaults = crate::core::settings::app_defaults(&|_| None);
    assert_eq!(app, defaults.keys().map(String::as_str).collect::<Vec<_>>());
}

#[test]
fn flag_follows_python_truth_and_defaults() {
    let mut s = Settings(serde_json::Map::new());
    assert!(s.flag("mods_enabled"));            // default true
    assert!(!s.flag("hdr"));                     // default false
    for (v, on) in [(json!("0"), true), (json!("no"), true), (json!(2), true), (json!("true"), true),
                    (json!(""), false), (json!(0), false), (json!(null), false), (json!(true), true)] {
        s.set("hdr", v.clone());
        assert_eq!(s.flag("hdr"), on, "{v}");
    }
}

#[test]
fn an_ini_flag_is_stored_as_one_or_zero() {           // store.rs
    let dir = tempfile::tempdir().unwrap();
    let (mut store, _) = Store::load(&Paths::from_env(dir.path(), &|_| None));
    assert!(store.set("vsync", json!(false)));
    assert_eq!(store.ini().values["vsync"], "0");
    assert!(!store.flag("vsync"));
    assert!(store.set("vsync", json!(true)));
    assert_eq!(store.ini().values["vsync"], "1");
    assert!(!store.set("vsync", json!(true)));        // unchanged
    assert!(store.app().get("vsync").is_none());
}

#[test]
fn an_unknown_key_goes_to_settings_json() {
    // Store::set("my_new_key", json!("x")) → app().get == Some("x"), ini sem a chave
}

#[test]
fn replace_names_the_changed_keys() {
    // replace(app com game_dir trocado, ini com engine_fps "60") == vec!["game_dir", "engine_fps"]
}

#[test]
fn ini_to_write_normalizes_only_shown_flags() {
    // ini com vsync = "true" e upscaler = "xx": ini_to_write()["vsync"] == "1"; "upscaler" ausente
}
```

- [ ] **Step 2: Run, expect FAIL** (não compila: os itens não existem)

Run: `cargo test --lib catalog store`

- [ ] **Step 3: Implement** a tabela e o `Store`.
  - `Store::load` usa `read_app_settings` e `read_ini` e devolve os arquivos ilegíveis do jeito
    que `ui/app.rs` monta hoje.
  - `ini_to_write` aplica a normalização que hoje está em `persist::gather` (`persist.rs:136-139`)
    só às chaves de `SHOWN_INI_KEYS`.

- [ ] **Step 4: Gate G**
- [ ] **Step 5: Commit** (`Refactor(Launcher): describe every setting once`)

### Task 2: defaults e listas de chaves derivados da tabela

**Files:**
- Modify: `src/core/settings.rs:50` (`app_defaults` monta a partir de `SETTINGS`),
  `src/core/ini.rs:11-46` (`INI_FLAGS` e `INI_DEFAULTS` vêm da tabela),
  `src/ui/persist.rs:34` (`SHOWN_INI_KEYS` vem da tabela)

**Interfaces:**
- Consumes: Task 1.
- Produces: as três listas continuam existindo com o mesmo tipo e conteúdo; só a origem muda.
  Ficam como `fn` ou `LazyLock` se `static` não for possível.
- Os callers de `INI_FLAGS`/`INI_DEFAULTS`/`SHOWN_INI_KEYS` só mudam se o tipo mudar.

- [ ] **Step 1: Guard test** em `catalog.rs`: `ini_lists_follow_the_table`. Ele compara
  `INI_FLAGS`, `INI_DEFAULTS` e `SHOWN_INI_KEYS` (as listas escritas à mão de hoje) com as linhas
  Ini da tabela, na ordem. As linhas Ini `written: true` são as de `SHOWN_INI_KEYS`.
- [ ] **Step 2: Run, expect PASS:** `cargo test --lib ini_lists_follow_the_table`. O teste fixa o
  conteúdo de hoje: é um refactor, então ele passa antes e depois.
- [ ] **Step 3: Implement** a derivação. `app_defaults` continua recebendo o `env` para o
  `SPARTAN_GAME_DIR`. O guard continua verde.
- [ ] **Step 4:** O teste `persist.rs::shown_ini_keys_are_the_18_controls_of_the_window` vai para
  `catalog.rs` com o mesmo nome. Atualize a linha no mapa de paridade, se ela existir.
- [ ] **Step 5: Gate G.** `cargo test --test fixtures` precisa continuar verde: são os bytes do
  Python.
- [ ] **Step 6: Commit** (`Refactor(Launcher): derive setting defaults from the catalog`)

### Task 3: `State.store`, um checkbox, o fim das cópias

**Files:**
- Modify: `src/ui/app.rs` (campo `store: Store` no lugar de `settings` e `ini`; `apply_set`
  vira `state.store.set`; sai `State.effects`), `src/ui/persist.rs` (`gather` usa
  `store.ini_to_write()`), `src/ui/widgets/checkbox.rs` (sai `checkbox_ini`),
  `src/ui/pages/{graphics,performance,game,controls,advanced}.rs`, `src/ui/home.rs` e todo arquivo
  em `src/ui` e `tests/` que leia `state.settings` ou `state.ini`
- Delete (código): `Settings::get_bool`, `advanced::effects_enabled`, os 4 `ini_on`
  (graphics.rs:17, performance.rs:257, game.rs:275, controls.rs:672), `home::ini_value`
  (home.rs:494) e o roteamento de `apply_set` (app.rs:538-559)

**Interfaces:**
- Consumes: `Store` (Task 1).
- Produces: `State.store: Store`, mais `Message::Set(key, Value)` e um `checkbox(key, …)` que
  manda `Value::Bool`.
- Os ifs por chave que ficam em `apply_set` (`mods_enabled`, `game_dir`/`update_dir`,
  `render_resolution`) são mantidos até a Task 21.
- `launcher_effects` passa a ser lido por `state.store.flag("launcher_effects")`.

- [ ] **Step 1: Failing tests**

```rust
// tests/ui_widgets.rs, substitui checkbox_ini_emits_one_and_zero
#[test]
fn a_check_box_sends_a_bool() {
    // messages(&state, "<rótulo do VSync>") contém Message::Set("vsync".into(), json!(false))
}

// tests/ui_save.rs (Review Focus 1)
#[test]
fn a_python_settings_file_saves_unchanged_but_for_the_key_set() {
    // copia launcher-rust/fixtures/collected/settings.json para a pasta do State,
    // abre o State, Set("hdr", true), Save; o JSON relido é igual ao original exceto "hdr",
    // e não contém "launcher_effects" nem "controls_device".
}

// tests/ui_pages.rs
#[test]
fn a_stored_string_zero_shows_checked_like_the_game_gets_it() {   // ⚠ truthiness
    // settings.json com "hdr": "0" → o checkbox HDR aparece marcado e
    // game_environment(...)["SPARTAN_HDR"] == "1"
}
```

- [ ] **Step 2: Run, expect FAIL:**
  `cargo test --test ui_widgets a_check_box_sends_a_bool` e os outros dois.
- [ ] **Step 3: Implement** a migração.
  - Leitura: troque `state.settings.get_str(k)` por `state.store.text(k)`,
    `state.settings.get_bool(k)` e `ini_on(state, k)` por `state.store.flag(k)`, e
    `state.ini.values[k]` por `state.store.text(k)`.
  - Escrita: `state.settings.set` e `state.ini.values.insert` viram `state.store.set`.
  - Onde um caller precisa do objeto (`game_environment`, `runner::play`, `persist`), use
    `state.store.app()` ou `state.store.ini()`.
- [ ] **Step 4:** Nos testes, as asserções `state.ini.values["k"]` viram `state.store.text("k")`.
  A exceção são as de `ui_save.rs`, que testam o arquivo.
- [ ] **Step 5: Gate G**
- [ ] **Step 6: Commit** (`Refactor(Launcher): read and write settings through one store`)

### Task 4: `env.rs` sobre o catálogo e o contrato launcher → runner

**Files:**
- Modify: `src/core/env.rs` (sai `truthy`/`flag` em env.rs:38-52; nomes, defaults e escolhas vêm
  do catálogo; sai a lista inline de env.rs:217), `src/runner/mod.rs:433, :621`,
  `src/runner/graphics_options.rs:25`
- Create: `launcher-rust/tests/env_contract.rs`

**Interfaces:**
- Consumes: `catalog::{setting, run_default}` e `Settings::flag`.
- Produces:
  - `pub fn environment(settings: &Settings, paths: &Paths, base: &IndexMap<String, String>, windows: bool) -> IndexMap<String, String>`
    (pura: não escreve arquivo);
  - `game_environment` (mesma assinatura de hoje) = `write_trophy_texts` + `environment`.
- O runner lê `catalog::run_default("SPARTAN_MODS_ENABLED")` e
  `catalog::run_default("SPARTAN_STREAM_MEMORY")`. `DEFAULT_ENGINE_FPS` vem do default da linha
  Ini `engine_fps`.

- [ ] **Step 1: Failing tests** em `tests/env_contract.rs`

```rust
#[test]
fn every_env_setting_round_trips() {
    // para cada Setting com env: Some(e) e kind Flag, environment() com o valor true dá "1" em
    // e.name e com false não dá "1"; para Choice, cada choice.value aparece como está.
}
#[test]
fn runner_defaults_are_run_pys() {
    assert_eq!(catalog::run_default("SPARTAN_MODS_ENABLED"), Some("1"));
    // e um assert por linha com env, com o literal do run.py citado no comentário
}
#[test]
fn environment_writes_no_file() {
    // tempdir vazio; environment(...) não cria trophy_texts.txt; game_environment(...) cria
}
```

- [ ] **Step 2: Run, expect FAIL:** `cargo test --test env_contract`
- [ ] **Step 3: Implement.** A ordem de inserção das chaves no `IndexMap` continua a de hoje:
  `tests/fixtures.rs` compara com o env do Python.
- [ ] **Step 4: Gate G**
- [ ] **Step 5: Commit** (`Refactor(Launcher): build the game environment from the catalog`)

---

## Candidato 02: plano e aplicação de patches (spec §4 / 02)

### Task 5: `core/python.rs` e `graphics_options` no core (moves)

**Files:**
- Create: `src/core/python.rs`, com `py_repr`, `py_list`, `py_hex`, `py_int`, `ensure_ascii` e
  `python_json`, vindos de `src/core/patches.rs:423-523` e `:1629-1690` (os testes vão junto)
- Move: `git mv src/runner/graphics_options.rs src/core/graphics_options.rs`
- Modify:
  - os 7 importadores de `python.rs`: `content_profile.rs:13`, `graphics_options.rs:16`,
    `link_libc.rs:27`, `glyphs.rs:1946`, `pause_menu.rs:42` e `:2260`, `runner/mod.rs:35`;
  - `src/core/options.rs`: recebe as constantes de nome de patch repetidas em
    `graphics_options.rs:22-24` e `patches.rs:22, :27, :359`;
  - os mapas de paridade (caminho `runner::graphics_options` → `core::graphics_options`);
  - a tabela §3.2 de `docs/specs/2026-10-22-rust-runner-design.md`.

**Interfaces:**
- Produces: `crate::core::python::*` e `crate::core::graphics_options::*`, com os mesmos nomes de
  hoje.

- [ ] **Step 1:** Este é um move puro, sem teste novo. Os testes que se movem são a prova.
- [ ] **Step 2: Gate G.** O pytest dos mapas confirma os caminhos novos.
- [ ] **Step 3: Commit** (`Refactor(Launcher): move the Python helpers and graphics options to core`)

### Task 6: `plan` e `PatchPlan`

**Files:**
- Modify: `src/core/patches.rs` (somar `PlanInput`, `Selection`, `External`, `PatchPlan`, `plan`
  e um `fn facts(game: &Path)` privado com title ID, versão e profile)

**Interfaces:**
- Produces: as assinaturas de `plan`/`PatchPlan`/`PlanInput`/`Selection`/`External` da spec
  §4/02.
- `Selection::Launch` faz o que `runner/mod.rs:805-830` faz hoje, nesta ordem:
  1. seleção salva;
  2. `default_enabled` dos mods;
  3. resolução do ini via `graphics_options::patch_names`;
  4. o patch de FPS mais alto;
  5. `restrict_with_log`.
- O resultado passa pelo join/split com `;`.
- `plan` escreve `out/patches-merged.xml` uma única vez.

- [ ] **Step 1: Failing tests** em `patches.rs` (`mod plan_tests`)
  - `script_selection_is_patches_py`: `Selection::Script{extra: &["X"], only: false}` sobre
    `dump_with_patches`-like dá os defaults mais "X", como `PatchJob` dá hoje.
  - `launch_selection_adds_ini_resolution_and_top_frame_rate`
  - `launch_selection_keeps_a_mods_default_patch`
  - `a_missing_frame_rate_patch_falls_back_with_two_log_lines`: o log capturado tem as 2 linhas
    "restrict" de hoje.
  - `mod_lines_are_logged_once` (⚠): o log tem exatamente 1 linha "from the mod file" por
    arquivo de mod.
- [ ] **Step 2: Run, expect FAIL:** `cargo test --lib plan_tests`
- [ ] **Step 3: Implement** `plan` reaproveitando o merge, a seleção e o `restrict` existentes.
  `PatchPlan` guarda o XML merged, os nomes finais, o profile e o caminho de saída.
- [ ] **Step 4: Gate G**
- [ ] **Step 5: Commit** (`Refactor(Launcher): plan the patches in one call`)

### Task 7: `apply` e `Applied`; o runner, `runner_fixtures` e `dump` passam a usá-los

**Files:**
- Modify:
  - `src/core/patches.rs`: somar `Applied`, `apply` e `Applied::environment`; saem `PatchJob` e
    `write_game_patches` (patches.rs:1700-1793) e os campos `engine_fps`, `dmem_mb` e `vblank_hz`
    de `PatchStatus` (:66-71);
  - `src/runner/mod.rs`: `patches()` vira `plan` + `apply` + `Applied::environment`. Saem
    `engine_fps` (:112-118), `split_patches` (:931), a releitura do env (:838, :847) e a do
    status (:858). A linha "Engine frame rate" (:594-607) usa `Applied::engine_fps`;
  - `tests/runner_fixtures.rs:195-205`, `tests/dump.rs:274-285` e `tests/runner.rs:512-523`.

**Interfaces:**
- Consumes: `plan` (Task 6).
- Produces: `apply(&PatchPlan, &Path, &mut dyn Write) -> Result<Applied, PatchError>`.
  `Applied::environment()` devolve os pares na ordem de `run.py:300-311`.

- [ ] **Step 1: Failing tests** em `patches.rs`
  - `apply_without_profile_writes_empty_patches_bin_and_no_status`
  - `a_skipped_frame_rate_patch_leaves_engine_at_60`
  - `applied_notes_give_dmem_and_vblank`
  - `apply_removes_a_stale_status_first`
  - `applied_environment_in_run_py_order`: substitui
    `tests/runner.rs::a_corrupt_patch_status_defaults_the_engine_fps`. Apague esse teste e repasse
    a linha do mapa de paridade para o novo.
- [ ] **Step 2: Run, expect FAIL:** `cargo test --lib apply`
- [ ] **Step 3: Implement** e troque os três callers.
- [ ] **Step 4: Gate G** e depois o **dump real**. `runner_fixtures::patches_match_python`
  continua com os mesmos bytes e o mesmo log, menos as linhas duplicadas de mod: se o fixture
  tiver mod, o esperado do log perde a segunda linha, e isso é a única mudança aceitável.
- [ ] **Step 5: Commit** (`Refactor(Launcher): apply the patch plan and return its needs`)

### Task 8: interface pública reduzida e testes de compilação sobre `plan`/`apply`

**Files:**
- Modify: `src/core/patches.rs`. Um `#[doc(hidden)] pub mod compiler` reexporta
  `compile_patches`, `eboot_image`, `eboot_segments` e `Write`. Viram privados: seleção, merge,
  requisitos, `write_*`, constantes e busca de profile. Sai `game_patchable` (:1166). Ficam
  públicos `PatchMeta`, `PatchStatus`, `patch_status`, `game_patch_metas`, `selectable`,
  `resolution_names`, `external_patches`, `external_enabled`, `builtin_xml`, `game_title_id` e
  `PatchError`.
- Modify: `tests/dump.rs:13-15`, que passa a importar de `patches::compiler`.

**Interfaces:**
- Produces: a interface pública da lista acima, e mais nada.

- [ ] **Step 1:** Reescreva sobre `plan`/`apply` os ~15 testes de compilação que chamam
  internals: `spartanport_defaults_and_extras`, `spartanport_resolution_patch_replaces_texture_fix`,
  `profiles_by_title_id`, `profile_by_title_for_any_region`, `merge_*`,
  `a_broken_mod_file_is_left_out`, `default_enabled_patches_of_a_mod`,
  `requirements_from_patch_notes`, `write_patches_*`, `write_status_*`,
  `a_game_without_a_profile_*`, `third_party_*`, `selection_and_unsupported_lines` e
  `a_repeated_external_name_*`. **Mantenha os nomes.**
- [ ] **Step 2:** Torne privados os itens da lista. `cargo test --lib patches` precisa compilar e
  passar.
- [ ] **Step 3: Gate G**
- [ ] **Step 4: Commit** (`Refactor(Launcher): narrow the patches interface`)

### Task 9: `mods::enabled_patch_files` devolve `Result` e o runner o usa

**Files:**
- Modify: `src/core/mods.rs:706` (`enabled_patch_files(...) -> Result<Vec<PathBuf>, ModsError>`,
  com o mesmo erro que o runner reporta hoje); `src/runner/mod.rs:436-446` passa a chamá-la

- [ ] **Step 1: Failing test** em `mods.rs`: `enabled_patch_files_reports_a_bad_selection`. Um
  `mods.json` ilegível dá `Err`, e não uma lista vazia.
- [ ] **Step 2: Run, expect FAIL:** `cargo test --lib enabled_patch_files_reports_a_bad_selection`
- [ ] **Step 3: Implement** e troque o runner.
  `tests/runner.rs::a_broken_mods_json_stops_with_the_mods_message` continua passando com o mesmo
  texto.
- [ ] **Step 4: Gate G** e depois o **dump real**.
- [ ] **Step 5: Commit** (`Refactor(Launcher): share the enabled mod patch files`)

---

## Candidato 03: runner atrás de um Host (spec §4 / 03)

### Task 10: `core/platform/tool.rs`

**Files:**
- Create: `src/core/platform/tool.rs` (declarado em `platform/mod.rs`)
- Modify: `src/runner/mod.rs`. `run_within` (:964-1011) e a chamada de cpu-features (:488-492)
  passam a usar `run_tool`.
- Test: `launcher-rust/tests/runner.rs`

**Interfaces:**
- Produces: `ToolCall`, `Stderr`, `Window`, `ToolOutput`, `ToolError` e `run_tool`, como na spec
  §4/03.
- O `Display` de `TimedOut` é `Command '[...]' timed out after N seconds`, igual ao texto que
  `tests/runner.rs:838` checa hoje.
- O polling fica em 20 ms.

- [ ] **Step 1: Failing test**

```rust
#[test]
fn a_hanging_tool_is_stopped_at_its_limit() {
    let call = ToolCall { program: fake_probe().into(), args: vec!["hang".into()],
        env: None, cwd: None, limit: Some(Duration::from_secs(1)),
        stderr: Stderr::Null, window: Window::Hidden };
    let start = Instant::now();
    let err = run_tool(&call).unwrap_err();
    assert!(matches!(err, ToolError::TimedOut { .. }));
    assert!(err.to_string().ends_with("timed out after 1 seconds"));
    assert!(start.elapsed() < Duration::from_secs(5));
}
```

- [ ] **Step 2: Run, expect FAIL:** `cargo test --test runner a_hanging_tool_is_stopped_at_its_limit`
- [ ] **Step 3: Implement.** Use spawn, um poll de `try_wait` a cada 20 ms e `kill` no limite.
  `Window::Hidden` aplica `CREATE_NO_WINDOW` no Windows.
- [ ] **Step 4: Gate G**
- [ ] **Step 5: Commit** (`Refactor(Launcher): run tools with a limit in one place`)

### Task 11: `Host`, `SystemHost` e `run_with`

**Files:**
- Create: `src/runner/host.rs`
- Modify: `src/runner/mod.rs`. `run` vira `catch_interrupt` + `run_with(ctx, &SystemHost)`.
  `Runner` guarda `&dyn Host`. Os `interrupted()` (504, 565, 610, 904, 81), `this_exe` e
  `process::id` (639), `physical_memory_mb` (622), `wait_for_pid` (685) e o spawn mais
  `wait_or_interrupt` (907-926, 949-959) passam pelo host.
- Create: `launcher-rust/tests/common/host.rs`, com `FakeHost` (incluído por `tests/runner.rs`)

**Interfaces:**
- Consumes: `ToolCall`, `run_tool` (Task 10).
- Produces: `trait Host`, `SystemHost` e `run_with`, como na spec §4/03 (sem `stream`).
- `FakeHost`:
  - campos `calls: RefCell<Vec<Called>>`, `cpu: String`,
    `gamepad: Result<String, ToolError>`, `exit: i32`, `interrupt: Interrupt`,
    `memory_mb: Option<u64>` e `on_play: Option<Box<dyn Fn(&ToolCall)>>`;
  - `struct Called { program: OsString, args: Vec<OsString>, cwd: Option<PathBuf>, limit: Option<Duration> }`,
    a cópia owned de cada `ToolCall`;
  - `enum Interrupt { Never, Always, DuringGame }`.
- No fixture `Port` de `tests/runner.rs:76`, soma
  `fn context<'a>(&'a self, out: &'a mut dyn Write) -> RunContext<'a>`, com o port e o env de
  hoje.

- [ ] **Step 1: Failing test** em `tests/runner.rs`: reescreva
  `a_broken_mods_json_stops_with_the_mods_message` para rodar em processo.

```rust
let port = Port::new();                     // sem copiar fake-probe
port.write("data/mods.json", "{");
let host = FakeHost::default();
let mut out = Vec::new();
let code = run_with(port.context(&mut out), &host);
assert_eq!(code, 2);                        // o código de hoje
assert!(String::from_utf8_lossy(&out).contains("Mods:"));
assert!(host.calls.borrow().is_empty());    // parou antes do probe
```

- [ ] **Step 2: Run, expect FAIL:** `cargo test --test runner a_broken_mods_json_stops_with_the_mods_message`
- [ ] **Step 3: Implement** `Host` e `SystemHost` com o código movido sem mudar a lógica.
- [ ] **Step 4: Gate G**
- [ ] **Step 5: Commit** (`Refactor(Launcher): put the runner's OS effects behind a host`)

### Task 12: sai o `set_current_dir`; caminhos relativos a partir do port

**Files:**
- Modify: `src/runner/mod.rs`. Sai a linha 409. `path_or`, `SPARTAN_CONFIG`, `SPARTAN_PATCH_DIR`
  e `SPARTAN_PROBE` juntam valores relativos ao port. Toda `ToolCall` leva `cwd: Some(port)`.
  `mods::resolve` (mods.rs:129) recebe a base explícita do runner.

**Interfaces:**
- Produces: nenhuma assinatura nova. ⚠ Argv e log mostram os caminhos absolutos.

- [ ] **Step 1: Failing tests** em `tests/runner.rs`
  - `a_relative_data_dir_is_from_the_port`: o port fica numa pasta **com espaço**, `"port with
    space"` (Review Focus 2), e `SPARTAN_DATA_DIR=data2`. Depois do `run_with` com `FakeHost`,
    existe `port/data2/out/boot.bin`, e toda `call.cwd == Some(port)`.
  - `run_starts_the_probe_in_the_port` (ponta a ponta, exe real e fake-probe): o probe grava o
    `cwd` em `out/cwd.txt`, que precisa ser igual ao port.
- [ ] **Step 2: Run, expect FAIL:** `cargo test --test runner relative_data_dir`
- [ ] **Step 3: Implement**
- [ ] **Step 4: Gate G**
- [ ] **Step 5: Commit** (`Fix(Launcher): resolve the runner's relative paths from the port`)

### Task 13: os testes do runner em processo

**Files:**
- Modify:
  - `tests/runner.rs`: os casos 1-8, 11 e 13-17 (na ordem do arquivo) passam a usar `run_with`
    com `FakeHost`. Os únicos ponta a ponta que ficam:
    `a_restart_from_the_game_starts_the_exe_again`, `ctrl_c_cleans_up_and_ends_with_130`
    (unix) e `run_starts_the_probe_in_the_port`;
  - `tests/common/mod.rs`: o fixture de Port grava um `bin/spartanport-probe` vazio;
  - `src/runner/mod.rs`: saem os unit tests das linhas 1210, 1219-1239, 1242, 1298 e 1578.
    Viram privados `Runner`, `run_within`, `wait_or_interrupt`, `this_exe`, `which`,
    `is_executable`, `parse_cpu_features`, `isa_rewrite_wanted`, `launch_prefix` e
    `find_executable`.
- Delete: `tests/runner_wait.rs`, absorvido por `tests/runner.rs` sobre `FakeHost::wait_for_pid`
- Docs: as linhas dos mapas de paridade que citam os unit tests apagados apontam para os
  substitutos.

- [ ] **Step 1:** Escreva os substitutos em processo, cada um falhando antes de existir:
  - `a_missing_movbe_gets_the_isa_table`: `cpu = "sse4_2 avx2"` faz o host ver `--isa-rewrite`;
  - `isa_rewrite_0_skips_it`;
  - `spartan_probe_is_the_program_started`;
  - `a_ctrl_c_stops_before_the_game`: com `Interrupt::Always`, nenhum `play` é chamado e o código
    é 130;
  - `a_pool_above_ram_warns`: `memory_mb: Some(4096)` faz o aviso aparecer no `out`;
  - `a_hanging_gamepad_probe_keeps_the_game_icons`: o host recebe
    `limit == Some(GAMEPAD_PROBE_LIMIT)` e responde `TimedOut`; o log tem a linha de hoje.
- [ ] **Step 2:** Mova os demais casos, privatize e apague os antigos.
- [ ] **Step 3: Gate G.** Registre no commit quantos testes de `tests/runner.rs` ainda sobem o
  exe: precisam ser 3, mais os 2 de `--play` que a Task 25 move.
- [ ] **Step 4: Commit** (`Test(Launcher): run the runner tests in process`)

---

## Candidato 04: leitor SCE e codec G3PROBE (spec §4 / 04)

### Task 14: `runner/sce.rs` com a camada de contêiner (move)

**Files:**
- Create: `src/runner/sce.rs`. Recebe `parse_self`, `read_self`, `SelfInfo`, `ProgramHeader` e os
  tipos de cabeçalho de `prepare.rs:337-447`, com seus testes.
- Modify: `src/runner/prepare.rs`. Reexporta (`pub use super::sce::{parse_self, read_self, …}`)
  para `tests/dump.rs:18` e `tests/runner_fixtures.rs:15`.

- [ ] **Step 1:** Move puro, sem teste novo.
- [ ] **Step 2: Gate G**
- [ ] **Step 3: Commit** (`Refactor(Launcher): give SCE containers their own module`)

### Task 15: `SceModule::read`, `relocations()` e `ids()`

**Files:**
- Modify: `src/runner/sce.rs`. Soma `Symbol`, `SceModule` e os métodos da spec §4/04, mais
  `Target`, `ImportKind` e `SceError`.
- Modify: `src/runner/link_libc.rs:115-372`. `load` vira wrapper sobre `SceModule::read`, e
  `module` continua com a mesma assinatura.

**Interfaces:**
- Produces: as assinaturas de `SceModule`, `Target` e `ImportKind` da spec.
- Regras:
  - a dinâmica é lida até `DT_NULL`;
  - as tabelas de símbolos e de relocações precisam ser múltiplas de 24, senão o erro é
    `"bad symbol table size"` / `"bad relocation table size"` (texto de prepare.py:201-202 e
    :210-211);
  - `relocations()` e `ids()` são lidos sob demanda.

- [ ] **Step 1: Failing tests** em `sce.rs`
  - `stops_at_dt_null_for_every_stage`: um STRTAB isca depois do `DT_NULL` não é usado.
  - `a_partial_symbol_table_is_named_by_every_stage`: o erro contém "bad symbol table size".
  - `a_partial_relocation_table_is_named`
  - `conflicting_ids_are_refused`
  - `plain_names_refused_or_unlinkable`
  - `resolve_gives_absolute_defined_or_undefined`
  - `import_kind_and_addend_rule`
- [ ] **Step 2: Run, expect FAIL:** `cargo test --lib sce`
- [ ] **Step 3: Implement** reaproveitando o código de `link_libc::load`.
- [ ] **Step 4: Gate G.** `runner_fixtures` link continua com os mesmos bytes.
- [ ] **Step 5: Commit** (`Refactor(Launcher): read an SCE module once`)

### Task 16: `prepare` e `inspect_libc` sobre `SceModule`; um interpretador de relocações

**Files:**
- Modify:
  - `src/runner/prepare.rs`: saem :676-741 e o braço de símbolo de `relocate`; `inspect_libc`
    usa `SceModule` (⚠ passa a parar no `DT_NULL`);
  - `src/runner/link_libc.rs:760-800` e `:602-612`;
  - `src/runner/link_modules.rs:200-260`.

**Interfaces:**
- Consumes: `SceModule::{read, relocations, resolve}` e `ImportKind` (Task 15).
- No link_modules, o caso weak-null continua antes da checagem de addend (:237-239 antes de
  :241-243).

- [ ] **Step 1: Failing test** em `prepare.rs`: `inspect_libc_stops_at_dt_null`. Uma tag
  `_init_env` falsa depois do `DT_NULL` é ignorada.
- [ ] **Step 2: Run, expect FAIL:** `cargo test --lib inspect_libc_stops_at_dt_null`
- [ ] **Step 3: Implement**
- [ ] **Step 4: Gate G.** Nos mapas de paridade, as mensagens "out of bounds" de link_libc,
  link_modules e inspect_libc que algum teste cite passam para "bad … table size".
- [ ] **Step 5: Commit** (`Refactor(Launcher): resolve relocations through the SCE module`)

### Task 17: codec `probe_image`, os quatro estágios sobre ele e a visibilidade

**Files:**
- Create: `src/runner/probe_image.rs`
- Modify:
  - `prepare.rs`: sai `boot_image` (:934-978);
  - `link_libc.rs`: saem :383-622 (`Boot`, `read_boot`, …) e :641-668;
  - `link_modules.rs`: `patch_fs_loads` (:58-82) vai para `ProbeImage`, o que desfaz o ciclo
    `link_libc.rs:17`;
  - `isa_rewrite.rs`: saem :30-37 (`MODULE_SIZE`) e :326-372; `read_boot_linked` fica com
    ~4 linhas sobre `decode`;
  - a visibilidade: `pub(super)` de prepare e link_libc vai a ~0;
  - a tabela §3.2 de `docs/specs/2026-10-22-rust-runner-design.md`: entram `sce.rs` e
    `probe_image.rs` como "compartilhados por prepare, link_libc, link_modules e isa_rewrite".

**Interfaces:**
- Produces: `Version`, `ProbeImage`, `Extension`, `ImageError`, `decode`, `encode`, `relocate`,
  `map_module` e `patch_fs_loads`, como na spec §4/04.
- Textos de erro por versão:
  - magic errado: `"linker expects freshly prepared G3PROBE2"` (Libc espera Prepared) e
    `"not a G3PROBE5 image"`;
  - nome com mais de 127 bytes: `"import name too long"` no Prepared e `"name too long"` nos
    demais.

- [ ] **Step 1: Failing tests** em `probe_image.rs`
  - `round_trips_every_version`
  - `decode_checks_the_magic_before_anything`
  - `decode_refuses_a_trailer_and_non_utf8_names`
  - `name_of_128_bytes_is_refused_with_each_versions_text`
  - `value_past_i64_is_refused`
  - `decodes_the_python_package_fixture`, que decodifica o G3PROBE de `tests/common/packages.rs:29`
  - `linked_module_records_are_56_bytes`
  - `patch_fs_loads_only_in_executable_loads`

  E em `isa_rewrite.rs` / `tests/runner.rs`:
  - `read_boot_linked_refuses_a_trailer` (⚠ decode estrito);
  - `a_corrupt_boot_linked_skips_the_isa_table` (Review Focus 5): com um `boot-linked.bin`
    corrompido, o runner segue, o log tem a mensagem de erro do ISA rewrite e o código não muda.
- [ ] **Step 2: Run, expect FAIL:** `cargo test --lib probe_image`
- [ ] **Step 3: Implement** e troque os quatro estágios.
- [ ] **Step 4: Gate G** e depois o **dump real**. Os bytes de prepare, link e isa ficam idênticos.
- [ ] **Step 5: Commit** (`Refactor(Launcher): encode and decode probe images in one codec`)

---

## Candidato 05: Installation do jogo (spec §4 / 05)

### Task 18: `GameFolder`

**Files:**
- Modify:
  - `src/core/game.rs`: soma `GameFolder`; saem `game_sfo` (:194), `game_title_id` (:201),
    `game_profile_of` (:212) e `game_patchable` (:218);
  - `src/core/patches.rs`: o `facts()` privado da Task 6 usa `GameFolder`; saem
    `game_app_version` (:1150), `game_title_id` (:1155) e `game_profile_for` (:1160);
  - `src/runner/mod.rs:799`, `src/runner/game_update.rs:115` (o fn injetado) e
    `src/ui/play.rs:190`.

**Interfaces:**
- Produces: `GameFolder::{read, title_id, app_version, profile, info}` (spec §4/05).
  `patches::game_title_id` continua público e passa a delegar.

- [ ] **Step 1: Failing test** em `game.rs`: `a_game_folder_reads_param_sfo_once`. Grave um
  `sce_sys/param.sfo` com `TITLE_ID=CUSA01623` e `APP_VER=01.02`; leia; apague o arquivo; os
  getters continuam respondendo.
- [ ] **Step 2: Run, expect FAIL:** `cargo test --lib a_game_folder_reads_param_sfo_once`
- [ ] **Step 3: Implement** e migre os callers.
- [ ] **Step 4: Gate G.** `runner_fixtures` fica verde.
- [ ] **Step 5: Commit** (`Refactor(Launcher): read a game folder's facts once`)

### Task 19: `Installation` e `Verdict`

**Files:**
- Create: `src/core/installation.rs`
- Modify:
  - `src/core/game.rs`: saem `ready_to_play` (:104), `folder_or_here` (:85) e `meta_folder`
    (:159);
  - `src/core/trophies.rs:153`: `trophy_package` passa a ser `Installation::trophy_package`;
  - `src/main.rs:53`, `src/core/env.rs:122, :145` e `src/ui/pages/advanced.rs:538`.

**Interfaces:**
- Consumes: `GameFolder` (Task 18) e `Store` (Task 1).
- Produces: `Installation`, `Verdict` e os métodos da spec §4/05.
- Uma constante `pub const SEPARATE_FOLDERS: &str`, com o texto de hoje de `setup.rs:166`.
- Regras:
  - pasta do jogo vazia = `"."`;
  - `meta_folder` = a pasta do update quando ela tem `eboot.bin`, senão a do jogo;
  - `system_file` e `trophy_package` escolhem o arquivo do update quando ele existe, arquivo a
    arquivo;
  - `passes()` = jogo completo, pastas diferentes e update completo (o `ready_to_play` de hoje).

- [ ] **Step 1: Failing tests** em `installation.rs` (tempdir)
  - `meta_folder_is_the_update_only_with_its_eboot`
  - `facts_come_from_the_meta_folder`
  - `system_files_prefer_the_update_file_by_file`
  - `verdict_lists_missing_files_per_folder`
  - `same_folder_counts_an_empty_game_folder_as_here` (⚠): `read("", ".")` dá
    `same_folder == true`
  - `passes_is_ready_to_play`, com os casos de `game.rs:406-467`
- [ ] **Step 2: Run, expect FAIL:** `cargo test --lib installation`
- [ ] **Step 3: Implement.** Apague os testes de `game.rs` substituídos (`a_complete_dump_is_ready`,
  `update_in_the_same_folder_is_not_ready` e as metades meta_folder/profile) e os asserts de
  `trophies.rs:348-353` e `patches.rs:2354-2359`. Atualize os mapas.
- [ ] **Step 4: Gate G**
- [ ] **Step 5: Commit** (`Refactor(Launcher): answer the game's facts from one installation`)

### Task 20: `State.installation` na UI

**Files:**
- Modify:
  - `src/ui/app.rs`: campo `installation: Installation`; saem `game_ready` (:142, :257);
  - `src/ui/splash.rs`: sai `Check::GameFolder` (:59, :171, :190-193);
  - `src/ui/home.rs`: saem `HomeFacts.{info, title_id, patchable}` (:140-142) e
    `refresh_facts` lê de `state.installation`;
  - `src/ui/setup.rs`: saem os campos e `check_folders` (:96-135); o setup mostra o
    `state.installation.verdict()`;
  - `src/ui/play.rs`: saem `folder_or_here` (:157) e as checagens de :245-276, trocadas pelo
    `Verdict`, na ordem do Python (has_game, ofertas do Sc0, arquivos do jogo, mesma pasta,
    arquivos do update);
  - `src/ui/pages/{game,mods,trophies}.rs` nas linhas da spec;
  - `tests/ui_splash.rs:58-62`: o assert sai.
- O refresh de `state.installation` fica nos lugares onde hoje se chama `refresh_facts`, até a
  Task 21.

- [ ] **Step 1: Failing test** em `tests/ui_setup.rs`:
  `setup_and_play_give_the_same_verdict`. Para 3 pares de pastas (completo; jogo sem
  `eboot.bin`; mesma pasta), o setup bloqueia se e somente se o PLAY recusa, e com o mesmo texto.
- [ ] **Step 2: Run, expect FAIL:** `cargo test --test ui_setup setup_and_play_give_the_same_verdict`
- [ ] **Step 3: Implement**
- [ ] **Step 4: Gate G**
- [ ] **Step 5: Commit** (`Refactor(Launcher): let the window read one installation`)

---

## Candidato 06: views derivadas seguem os settings (spec §4 / 06)

### Task 21: a tabela `follow`; `Set` e `Browsed` passam por ela

**Files:**
- Create: `src/ui/follow.rs` (declarado em `ui/mod.rs`)
- Modify:
  - `src/ui/app.rs`: `Message::Set` e `Message::Browsed(k, Some(p))` fazem
    `if state.store.set(k, v) { return follow(state, Cause::Keys(&[k])) }`; saem os ifs por chave
    de `apply_set`;
  - `src/ui/pages/game.rs`: saem `folder_chosen` (:77-91) e `game_changed` (:94-100);
  - `src/ui/setup.rs`: sai `refresh_folders` (:139);
  - `src/ui/pages/mods.rs:69-74` e `trophies.rs:68-73`: saem `FOLDERS`/`folders()`, trocados por
    `follow::inputs`.
- Create: `launcher-rust/tests/ui_follow.rs`

**Interfaces:**
- Consumes: `Store` (Task 1) e `Installation` (Task 19).
- Produces: `View`, `Cause`, `follow` e `inputs`, como na spec §4/06, com a tabela da spec na
  ordem dada.

- [ ] **Step 1: Failing tests** em `tests/ui_follow.rs`
  - `each_view_follows_exactly_its_keys`, por tabela, para cada `View`:
    1. mude o disco que a view lê;
    2. mande `Set` de uma chave fora da linha dela: a view não muda;
    3. mande `Set` de uma chave da linha: a view muda.
  - `typing_and_browsing_a_folder_give_the_same_views`
  - `typing_a_game_folder_letter_by_letter_keeps_the_last_read` (Review Focus 3): mande
    `Browsed("game_dir", "C:/g")`, depois `"C:/ga"`, depois `"C:/game"`, e entregue as leituras
    de Mods fora de ordem com `load_mods`. A lista final é a da pasta `"C:/game"`.
  - Mova para cá `a_read_for_a_folder_typed_over_is_dropped` (ui_pages.rs:597), com o mesmo nome
    e a linha do mapa atualizada.
- [ ] **Step 2: Run, expect FAIL:** `cargo test --test ui_follow`
- [ ] **Step 3: Implement**
- [ ] **Step 4: Gate G**
- [ ] **Step 5: Commit** (`Refactor(Launcher): refresh derived views from one key table`)

### Task 22: os demais gatilhos sobre `follow`

**Files:**
- Modify:
  - `src/ui/app.rs`: `State::new` → `follow(Start)` (:182-187, :262-268); o foco da janela
    (:470) → `Keys(&["game_dir","update_dir"])`;
  - `src/ui/persist.rs`: `collect` (:269-274) → `follow(Keys(&store.replace(..)))`; `rebuild`
    (:377-381) → `follow(Rebuild)`;
  - `src/ui/setup.rs`: Skip (:204-208) → `follow(Keys(&store.replace(..)))`;
  - `src/ui/play.rs`: `exited` (:538-539) → `Keys(&["user_dir"])` (⚠ relista os backups);
  - `src/ui/pages/game.rs`: `Restored` (:173-174) → `Keys(&["user_dir"])`;
  - AdoptDone → `Keys(&["game_dir","update_dir"])`;
  - os `refresh_*` viram privados.
- Modify (testes):
  - `ui_pages.rs:491` e `ui_home.rs:218, 236, 245` passam a usar `Message::Set`;
  - `ui_home.rs:231, 416` passam a usar `follow(Keys(&["user_dir"]))`;
  - `ui_pages.rs:461, 468, 484, 488, 1713, 1766` passam a usar `follow(Keys(&["mods_dir"]))`.

- [ ] **Step 1: Failing tests** em `tests/ui_follow.rs`
  - `skip_follows_the_restored_folders`
  - `collect_follows_a_resolution_changed_on_disk`
  - `the_game_exit_lists_the_new_backup` (⚠): crie um backup em `savedata_backups/`, mande o
    `Exited`, e ele aparece em `state.backups`.
  - `a_game_folder_change_keeps_the_chosen_backup` (⚠): mudar o `game_dir` não reseta
    `backup_choice`.
- [ ] **Step 2: Run, expect FAIL:** `cargo test --test ui_follow`
- [ ] **Step 3: Implement** e migre os testes.
- [ ] **Step 4: Gate G**
- [ ] **Step 5: Commit** (`Refactor(Launcher): follow every settings change through one table`)

---

## Candidato 07: um spawner do jogo (spec §4 / 07)

### Task 23: `returncode` (-N para sinal)

**Files:**
- Modify:
  - `src/core/launch.rs`: soma `pub fn returncode(status: ExitStatus) -> i32`; `spawn_game`
    passa a usá-lo no lugar de `unwrap_or(-1)` (:101);
  - `src/runner/mod.rs`: sai `exit_code` (:284-295); a espera do runner (:952) usa `returncode`;
  - `tests/launch.rs:106`: o esperado vira `if cfg!(windows) { 1 } else { -15 }`;
  - o doc de `GameEvent::Exited`.

- [ ] **Step 1: Failing test** em `tests/launch.rs`

```rust
#[cfg(unix)]
#[test]
fn a_signal_exit_is_minus_the_signal() {
    // spawn_game de `sh -c 'kill -9 $$'`; o Exited recebido é -9
}
```

- [ ] **Step 2: Run, expect FAIL** no WSL (ver Global Constraints):
  `cargo test --test launch a_signal_exit_is_minus_the_signal`
- [ ] **Step 3: Implement.** No unix: `status.code().unwrap_or_else(|| -status.signal().unwrap_or(1))`.
  No Windows: `status.code().unwrap_or(-1)`.
- [ ] **Step 4: Gate G**
- [ ] **Step 5: Commit** (`Fix(Launcher): report a signal exit as minus the signal`)

### Task 24: `Launch`, `Group`, `Exit` e o conselho no log; a UI usa o novo spawner

**Files:**
- Modify:
  - `src/core/launch.rs`: `spawn_game(launch: Launch<'_>)`; somam `Group`, `Exit` e
    `GameEvent::Exited(Exit)`; a thread leitora aplica `exit_hint` (última linha que casar) e grava
    o conselho de saída no `RunLog` quando o código não é 0;
  - `src/core/platform/{windows,unix}.rs`: `Group::Caller` não cria job nem sessão;
  - `src/ui/play.rs`: `start()` passa `Group::Own` (:357); saem os `hint` de `GameState`
    (:52, :59); `output()` não chama `exit_hint` (:479, :502-506); `exited(Exit)` (:513);
    o leitor sumido dá `Exit { code: -1, hint: None }` (:403);
  - `src/bin/fake_game.rs`: somam `--say TEXT` (imprime TEXT e sai) e `--env NAME` (imprime o
    valor da variável NAME);
  - `tests/launch.rs` (os 5 testes passam a usar `Launch`) e `tests/ui_play.rs:471, 489, 498`.

**Interfaces:**
- Produces: `Launch`, `Group::{Own, Caller}`, `Exit { code, hint }` e `spawn_game(Launch)`
  (spec §4/07). `lang` escolhe o texto do conselho (`ExitHint` + i18n de hoje).

- [ ] **Step 1: Failing tests** em `tests/launch.rs`
  - `the_last_hint_line_reaches_the_exit`: `fake_game --say "<linha de hint de memória>"` com
    saída 1 dá `Exit.hint == Some(ExitHint::…)`.
  - `a_failed_exit_writes_the_advice_to_the_run_log` (⚠): o `last_run.log` termina com
    `"\n{conselho}\n"`.
  - `a_clean_exit_writes_no_advice`
  - `caller_group_keeps_the_process_group` e `own_group_has_its_own` (unix, `getpgid`).
- [ ] **Step 2: Run, expect FAIL:** `cargo test --test launch`
- [ ] **Step 3: Implement**
- [ ] **Step 4: Gate G**
- [ ] **Step 5: Commit** (`Refactor(Launcher): carry the exit hint in the game session`)

### Task 25: `play_with` sobre `spawn_game(Group::Caller)`

**Files:**
- Modify:
  - `src/runner/mod.rs`: `play_with(context, settings, paths, command: &[String], notify)`
    (:210); sai o loop de pipe, `Command` e leitura (:220-269); `play` passa
    `run_command(&this_exe(), &[])`;
  - `tests/runner.rs`: `play_writes_last_run_log_with_the_advice` e
    `play_starts_the_game_with_the_saved_stream_memory` passam a usar o `fake_game` (`--say`,
    `--env SPARTAN_STREAM_MEMORY`), com os mesmos nomes e sem Port.

**Interfaces:**
- Consumes: `spawn_game`, `Launch`, `Group::Caller` e `GameEvent` (Task 24).

- [ ] **Step 1:** Reescreva os 2 testes para o `fake_game` e veja-os falhar: a assinatura ainda
  recebe `exe`.
- [ ] **Step 2: Run, expect FAIL:** `cargo test --test runner play_`
- [ ] **Step 3: Implement.** Uma falha de start continua imprimindo `"{program}: {error}"` com
  saída 1. O conselho no `--play` só sai com código diferente de 0, via `notify`.
- [ ] **Step 4: Gate G** e depois o **dump real**.
- [ ] **Step 5: Commit** (`Refactor(Launcher): play through the shared game session`)

---

## Candidato 08: `GameState::is_live()` (spec §4 / 08)

### Task 26: `is_live` e `refuse_while_live`

**Files:**
- Modify:
  - `src/ui/play.rs`: soma `impl GameState { is_live, is_idle, is_copying }`, `LiveAction` e
    `refuse_while_live`; sai `game_live` (:114);
  - saem `game::restore_blocked` (:111), `trophies::game_running` (:136) e
    `advanced::clear_cache_blocked` (:60);
  - os call sites: game.rs:146, trophies.rs:173, advanced.rs:362 e 562, performance.rs:249,
    update.rs:256 e 300, app.rs:826, home.rs:528 e 631-635, play.rs:201, 435 e 456.

**Interfaces:**
- Produces: `GameState::is_live(&self) -> bool` (`Preparing | Running`),
  `refuse_while_live(&mut State, LiveAction) -> bool` e
  `LiveAction::{RestoreSaves, ResetTrophies, ClearShaderCache, Update}`. Cada ação usa o texto de
  hoje.

- [ ] **Step 1: Failing test** em `tests/ui_play.rs`, que substitui :705-738

```rust
#[test]
fn every_live_action_is_refused_while_the_game_runs() {
    for action in [LiveAction::RestoreSaves, LiveAction::ResetTrophies,
                   LiveAction::ClearShaderCache, LiveAction::Update] {
        // jogo Running (fake_game) → refuse_while_live == true e o modal Info tem o texto de hoje
        // depois de Stop e Exited → refuse_while_live == false e nenhum modal
    }
}
```

- [ ] **Step 2: Run, expect FAIL:** `cargo test --test ui_play every_live_action`
- [ ] **Step 3: Implement.** Os asserts de `ui_pages.rs:686` e `:1828` viram
  `!state.game.is_live()`.
- [ ] **Step 4: Gate G**
- [ ] **Step 5: Commit** (`Refactor(Launcher): ask the game state whether it is live`)

### Task 27: construtores de `Modal`

**Files:**
- Modify:
  - `src/ui/widgets/modal.rs`: `Modal::{info, error, ask, confirm}`;
  - saem os helpers play.rs:143, update.rs:174, advanced.rs:402, game.rs:127, mods.rs:286 e
    trophies.rs:143;
  - os 11 literais `Modal { title: APP_NAME … }` em 7 arquivos passam a usar os construtores.

- [ ] **Step 1: Failing test** em `modal.rs`:
  `constructors_title_the_dialog_with_the_app_name`. Os quatro construtores dão
  `title == APP_NAME` e o `kind` certo.
- [ ] **Step 2: Run, expect FAIL:** `cargo test --lib constructors_title_the_dialog`
- [ ] **Step 3: Implement** e migre.
- [ ] **Step 4: Gate G**
- [ ] **Step 5: Commit** (`Refactor(Launcher): build dialogs with one constructor each`)

---

## Candidato 09: contexto da execução (spec §4 / 09)

### Task 28: payloads da `Route`

**Files:**
- Modify:
  - `src/startup.rs:22-62`: `Route` com payloads (spec); `route_on` monta os payloads;
    `InstallUpdate { target, pid }` com pid ilegível = 0 (como main.rs:79);
  - `src/main.rs`: saem os `args[1]`, `args[2]` (:70-71), `skip(1)` (:41) e `&args[1..]` (:87);
  - `tests/startup.rs:17-55, :349-352`.

- [ ] **Step 1: Failing tests** em `tests/startup.rs`
  - `install_update_carries_target_and_pid`: `["--install-update", "C:/t", "42"]` no Windows dá
    `InstallUpdate { target: "C:/t".into(), pid: 42 }`, e `"x"` dá `pid: 0`.
  - `run_carries_everything_after_it`
  - os testes de rota existentes passam a checar o payload.
- [ ] **Step 2: Run, expect FAIL:** `cargo test --test startup`
- [ ] **Step 3: Implement**
- [ ] **Step 4: Gate G**
- [ ] **Step 5: Commit** (`Refactor(Launcher): carry each route's arguments`)

### Task 29: `Context` e `start`; encaminhar antes do `Paths` em toda rota

**Files:**
- Modify:
  - `src/startup.rs`: somam `Context`, `start` e `apply_to_process`; sai `forward_environment`
    (:103);
  - `src/main.rs`: uma chamada a `start`; saem os 4 `Paths::from_env` (:33, :49, :69, :97);
  - `src/runner/mod.rs`: `RunContext` recebe `&Context` (:47-52); o upper-case do Windows
    (:326-342) e o forwarding (:412) saem do runner.

**Interfaces:**
- Produces: `startup::{Context, start, apply_to_process}` (spec §4/09).
- Ordem em `start`:
  1. `environment_of`;
  2. maiúsculas no Windows;
  3. `start_up_environment`;
  4. `Paths::from_env`, que no `--run` usa o port resolvido.

- [ ] **Step 1: Failing tests** em `tests/startup.rs`
  - `every_route_forwards_old_names_before_paths`: com `GOW3_DATA_DIR=<tmp>/d`, as quatro rotas
    (Run, Play, InstallUpdate, Launcher) dão `context.paths.data_dir == <tmp>/d`. Corrige o
    `--install-update`.
  - `run_paths_start_at_the_resolved_port`
  - `windows_names_are_upper_case`: `start(..., [("spartan_x","1")], true)` dá
    `env["SPARTAN_X"] == "1"`.
- [ ] **Step 2: Run, expect FAIL:** `cargo test --test startup forwards`
- [ ] **Step 3: Implement**
- [ ] **Step 4: Gate G**
- [ ] **Step 5: Commit** (`Fix(Launcher): forward the old names before every route's paths`)

### Task 30: `RunPlan`

**Files:**
- Create: `src/runner/plan.rs`
- Modify: `src/runner/mod.rs`. As leituras com default próprio passam a ler o plano: 414, 431-435,
  453, 468, 479, 621, 626, 650, 764, 811-812, 839-840, 1031 e 1101. O
  `Paths::from_env(&port, &|_| None)` de :468 usa o `Paths` do `Context`.

**Interfaces:**
- Consumes: `Context` (Task 29).
- Produces: `RunPlan` e `RunPlan::read(&Paths, &IndexMap<String, String>) -> RunPlan` (spec
  §4/09).
- Consts do run.py em `plan.rs`: `TIMEOUT = "0"`, `CONTENT_SKU = "full"`, `MSYS2_ROOT` (o valor
  de hoje) e o jogo padrão `port.parent().join("CUSA01623")`.
- Valor vazio = ausente (⚠).

- [ ] **Step 1: Failing tests** em `plan.rs`
  - `defaults_are_the_paths_ones`: sem variáveis, `plan.data == paths.data_dir`,
    `plan.mods_config == paths.mods_json()` e assim por diante.
  - `variables_override_each_default`
  - `relative_values_are_from_the_port`
  - `empty_values_count_as_unset` (⚠): `SPARTAN_DATA_DIR=""` dá o default.
- [ ] **Step 2: Run, expect FAIL:** `cargo test --lib plan`
- [ ] **Step 3: Implement**
- [ ] **Step 4: Gate G**
- [ ] **Step 5: Commit** (`Refactor(Launcher): read the runner's settings into one plan`)

### Task 31: o plano lê os defaults do catálogo

**Files:**
- Modify: `src/runner/plan.rs`. `mods_switch`, `stream_memory` e `glyph_style` usam
  `catalog::run_default`. O `mods_switch` mantém as duas leituras: `!= "0"` no passo 4, e outro
  valor dá saída 2 no passo 9 (runner/mod.rs:533-535).

- [ ] **Step 1: Failing test:** `plan_reads_the_catalog_run_defaults`. Sem variáveis,
  `plan.mods_switch == catalog::run_default("SPARTAN_MODS_ENABLED").unwrap()`, e o mesmo para os
  outros dois.
- [ ] **Step 2: Run, expect FAIL:** `cargo test --lib plan_reads_the_catalog_run_defaults`
- [ ] **Step 3: Implement**
- [ ] **Step 4: Gate G**
- [ ] **Step 5: Commit** (`Refactor(Launcher): take the runner defaults from the catalog`)

---

## Candidato 10: Updater atrás de `ReleaseHost` (spec §4 / 10)

### Task 32: o port, `GitHubReleases`, `FakeReleases` e `State::with_host`

**Files:**
- Modify:
  - `src/core/update.rs`: somam `InstallTarget`, `trait ReleaseHost` e `GitHubReleases`;
  - vêm de `src/ui/update.rs:186-192, :354-425` para o core: `install_windows`,
    `install_appimage`, `find_launcher` e `is_packaged`;
  - `src/ui/app.rs`: `State::with_host(paths, host: Arc<dyn ReleaseHost>)`; `State::new` usa
    `GitHubReleases`;
  - `tests/common/mod.rs`: `state()` usa `FakeReleases`.
- Create: `launcher-rust/tests/common/releases.rs`, com `FakeReleases`. Ele tem canais que
  reportam cada chamada (`list`, `install`, `open_page`), respostas roteirizadas e `installed`
  configurável.

**Interfaces:**
- Produces: `ReleaseHost` (`installed`, `target`, `list`, `install`, `open_page`),
  `InstallTarget` e `State::with_host` (spec §4/10).
- Antes de começar: confira se o iced 0.14 (feature `advanced`) expõe `Task::into_stream` ou um
  equivalente. Se expuser, some `common::run_task(task) -> Vec<Message>`; se não, os testes
  alimentam `Checked`/`Done` à mão. Registre a escolha na mensagem do commit.

- [ ] **Step 1: Failing tests** (`tests/ui_pages.rs`)
  - `install_on_a_read_only_appimage_offers_the_page`: `target() = AppImage { writable: false }`
    faz o Install abrir o diálogo de página.
  - `install_from_a_source_tree_opens_the_page`: `target() = WindowsSource` faz o fake registrar
    `open_page(RELEASES_PAGE)`.
  - `ui_home` não toca mais a rede: `ui_home.rs:738-747` e `:942-945` passam pelo fake, e o teste
    checa que o fake recebeu `list` e `open_page`.
- [ ] **Step 2: Run, expect FAIL:** `cargo test --test ui_pages install_on_a_read_only_appimage`
- [ ] **Step 3: Implement**
- [ ] **Step 4: Gate G**
- [ ] **Step 5: Commit** (`Refactor(Launcher): reach releases through a host`)

### Task 33: o sub-state `Updater`

**Files:**
- Modify:
  - `src/ui/update.rs`: somam `Updater`, o `Phase` privado e os getters da spec;
    `start_up_check(state)`; saem os campos `update`, `installed`, `update_progress` e
    `self_update` de `State` (app.rs:109-113, :135, :192-199, :235-237, :253), trocados por
    `updater`;
  - `src/ui/splash.rs:194-201`: usa `start_up_check`;
  - `src/ui/version_menu.rs`, `src/ui/pages/advanced.rs:139, :322`: leem do `Updater`.

- [ ] **Step 1: Failing tests** em `tests/ui_pages.rs`
  - `a_confirmed_install_shows_progress`: o fake manda 42; depois de drenar, o rótulo contém
    `"42%"`.
  - `a_silent_check_asks_the_host`
  - `install_while_the_game_runs_says_close_it`
  - `the_start_up_check_needs_self_update`
  - `a_failed_listing_shows_the_error_and_play_still_works` (Review Focus 4): `list() = Err("offline")`
    deixa `listing() == Listing::Failed` e o texto aparece. Em seguida, `Message::Play` com uma
    instalação válida ainda inicia o jogo.
- [ ] **Step 2: Run, expect FAIL:** `cargo test --test ui_pages progress`
- [ ] **Step 3: Implement**
- [ ] **Step 4: Gate G**
- [ ] **Step 5: Commit** (`Refactor(Launcher): keep the update flow in one sub-state`)

### Task 34: testes do updater pela interface; helpers privados

**Files:**
- Modify:
  - `tests/ui_pages.rs`: saem :2421, :2469, :2482 e :2491 (helpers puros);
  - os 14 `state.update =`, 12 `state.installed =` e os `self_update` de :1985-1999 passam a
    usar mensagens ou o `installed` do fake;
  - `src/ui/update.rs`: `offer_decision`, `install_decision`, `Decision` e `InstallPlan` ficam
    privados.

- [ ] **Step 1:** Migre os testes. Um teste que só testava um helper puro e tem equivalente na
  Task 33 é apagado; a linha do mapa de paridade, se houver, passa a apontar o equivalente.
- [ ] **Step 2: Gate G**
- [ ] **Step 3: Commit** (`Test(Launcher): drive the updater through messages`)

---

## Candidato 11: seam de plataforma completo (spec §4 / 11)

### Task 35: `Tools`, `FakeTools` e `Host: Tools`; gpu, steam e legacy

**Files:**
- Modify:
  - `src/core/platform/tool.rs`: somam `trait Tools: Send + Sync`, `SystemTools` e, sob
    `#[cfg(test)]`, `FakeTools`. Os testes que o usam são unit tests do lib (gpu, steam,
    legacy). O `FakeHost` de `tests/common/host.rs` implementa `Tools` por conta própria;
  - `src/runner/host.rs`: `trait Host: Tools`; sai `Host::tool`;
  - `src/core/gpu.rs:99-133`: `list_gpus(tools: &dyn Tools, …)`, `list_displays` e
    `connected_gamepad` recebem `&dyn Tools`;
  - `src/core/legacy.rs:345-373` e `src/core/steam.rs:170-184`;
  - os callers dessas funções passam `&SystemTools`.

**Interfaces:**
- Consumes: `ToolCall`, `ToolOutput`, `ToolError` e `run_tool` (Task 10).
- Produces: `Tools::run(&self, &ToolCall) -> Result<ToolOutput, ToolError>` e `SystemTools`.
  `FakeTools` registra as chamadas e responde por programa.

- [ ] **Step 1: Failing tests** em `gpu.rs`, `steam.rs` e `legacy.rs`
  - `gpus_come_from_the_tool_rows`
  - `a_failing_gpu_tool_lists_nothing`
  - `displays_run_under_the_chosen_backend`
  - `a_gamepad_line_means_connected`
  - `steam_running_reads_the_task_list`
  - `retarget_gives_up_on_timeout`: `FakeTools` responde `TimedOut` sem espera real.
- [ ] **Step 2: Run, expect FAIL:** `cargo test --lib gpus_come_from_the_tool_rows`
- [ ] **Step 3: Implement**
- [ ] **Step 4: Gate G**
- [ ] **Step 5: Commit** (`Refactor(Launcher): run every tool through one seam`)

### Task 36: funções simétricas e as duplicatas removidas

**Files:**
- Modify:
  - `src/core/platform/{mod,windows,unix}.rs`: `has_console` (unix: `true`), `hide_window`,
    `hide_console_window`, `Exec`, `make_executable`, `which`, `is_executable` e
    `physical_memory_mb`, todos nos dois sistemas;
  - saem as cópias: `desktop.rs:237-246, 297-304, 363-377`, `legacy.rs:215-222`,
    `gpu.rs:104-108`, `update.rs:254-264`, `runner/mod.rs:1050-1067` (`which`/`is_executable`),
    `guest_memory.rs:30-48`;
  - `runner/mod.rs:200-205`: sai o `cfg` (`has_console` existe nos dois).

- [ ] **Step 1: Failing test** em `platform/unix.rs`:

```rust
#[cfg(unix)]
#[test]
fn adding_keeps_other_mode_bits() {
    // arquivo com modo 0o640; make_executable(p, Exec::Adding) → modo & 0o777 == 0o775
    // outro arquivo 0o640; Exec::Exactly → 0o755
}
```

  No Windows, `make_executable` é no-op que devolve `Ok(())`. Teste
  `make_executable_is_a_no_op_on_windows`.
- [ ] **Step 2: Run, expect FAIL:** `cargo test --lib make_executable` no Windows, e o teste unix
  no WSL (ver Global Constraints).
- [ ] **Step 3: Implement** e troque os callers. `desktop.rs` usa `Exec::Exactly` e `update.rs`
  usa `Exec::Adding`.
- [ ] **Step 4: Gate G**
- [ ] **Step 5: Commit** (`Refactor(Launcher): give both systems the same platform functions`)

### Task 37: `home(Lookup)`

**Files:**
- Modify:
  - `src/core/env.rs`: `pub type Lookup<'a> = &'a dyn Fn(&str) -> Option<String>;`;
  - `src/core/platform`: `pub fn home(env: Lookup) -> Option<PathBuf>`;
  - os callers `paths.rs:57-62`, `desktop.rs:166-170, 397`, `legacy.rs:44-51, 242`,
    `steam.rs:207-210` e `advanced.rs:484`.

- [ ] **Step 1: Failing test** em `platform/mod.rs`: `home_is_userprofile_or_home_never_empty`.
  No Windows, `home(&|k| (k=="USERPROFILE").then(|| "C:/u".into())) == Some("C:/u")`; nos
  demais, com `HOME`. Um valor vazio dá `None`. Em `paths.rs`, com HOME vazio, o fallback é `"."`.
- [ ] **Step 2: Run, expect FAIL:** `cargo test --lib home_is_userprofile`
- [ ] **Step 3: Implement**
- [ ] **Step 4: Gate G**
- [ ] **Step 5: Commit** (`Refactor(Launcher): find the home folder one way`)

### Task 38: o módulo `ipc`

**Files:**
- Create: `src/core/platform/ipc.rs`, com `trait Transport` e
  `connect(&Path) -> io::Result<Box<dyn Transport>>` nos dois sistemas; `set_pipe_nowait` e
  `pipe_bytes_available` ficam privados aqui.
- Modify: `src/runner/discord.rs`, cujos 10 gates `cfg` de produção passam a usar `ipc`. Os
  testes de transporte (:1442-1630) vão junto com o código.

- [ ] **Step 1: Failing test** em `ipc.rs`: `connecting_to_a_missing_path_is_not_found`. Nos dois
  sistemas, `connect` num caminho inexistente dá `ErrorKind::NotFound`.
- [ ] **Step 2: Run, expect FAIL:** `cargo test --lib connecting_to_a_missing_path`
- [ ] **Step 3: Implement**
- [ ] **Step 4: Gate G**
- [ ] **Step 5: Commit** (`Refactor(Launcher): open the Discord pipe through the platform`)

### Task 39: as leituras restantes de env recebem `Lookup`

**Files:**
- Modify: `src/core/gpu.rs:251, 257`, `src/core/desktop.rs:392-401` e
  `src/runner/pause_menu.rs:2009, 2194`. Elas recebem `Lookup`; os callers do runner passam o env
  do `Context` (Task 29) e os da UI, o env do `State`.

- [ ] **Step 1: Failing test** em `pause_menu.rs`: `the_label_reads_the_given_environment`. O
  rótulo usa o valor do `Lookup` passado, não o do processo.
- [ ] **Step 2: Run, expect FAIL:** `cargo test --lib the_label_reads_the_given_environment`
- [ ] **Step 3: Implement**
- [ ] **Step 4: Gate G** e depois o **dump real** (o `pause_menu` gera bytes).
- [ ] **Step 5: Commit** (`Refactor(Launcher): pass the environment to every reader`)

---

## Fechamento

### Task 40: relatório final

**Files:**
- Create (fora do repo): `<scratchpad>/deepening-report-<timestamp>.html`. Siga o estilo do
  relatório da revisão: Tailwind e Mermaid via CDN, render sequencial com ids fixos.

Conteúdo, por candidato:
- os commits;
- as interfaces que entraram;
- as linhas removidas e adicionadas (`git diff --stat 708a928..HEAD -- <arquivos>`);
- as mudanças ⚠ confirmadas, com o teste que fixa cada uma;
- o que ficou fora ou foi adiado, com o motivo.

Totais:
- o número de testes antes (gate da Task 1) e depois;
- o tempo do `cargo test` antes e depois;
- quantos testes sobem o exe real;
- os mapas de paridade verdes.

- [ ] **Step 1:** Colete os números (`cargo test 2>&1 | grep "test result"`, `git log --oneline
  708a928..HEAD` e `git diff --stat`).
- [ ] **Step 2:** Escreva o HTML, confira o render headless (Edge `--dump-dom`, sem
  `Syntax error in text`) e abra para o usuário.
- [ ] **Step 3:** Nada a commitar; o relatório fica fora do repo.
