# Auditoria UX/UI do ARGVUS Control Center

Documento de trabalho do refactor de padronização de menus, ícones e ações
(skill `argvus-control-center-ux`). **Fase 0: auditoria, sem mudança de
comportamento.**

- Data: 2026-10-04
- Base auditada: `argvus-control-center` em `8d020c7`; `argvus-tui` em `d6f40c6`
  (resolvido por path: `../../../argvus-tui/crates/argvus-tui`, ou seja,
  `de/argvus-tui`, com fonte acessível e editável).
- Catálogo i18n: `de/argvus-i18n/locales/{en-US,pt-BR}/control-center.json`
  (1256 chaves em cada idioma).
- Fonte de ícones verificada: `/usr/share/fonts/TTF/SymbolsNerdFontMono-Regular.ttf`
  (nomes de glyph lidos da própria fonte com `fontTools`).

Legenda de tipos de linha usada abaixo: **Info**, **Action**, **Submenu**,
**Toggle**, **Choice**, **Value**, **Destructive** (ver seção 4).

Legenda de momento do efeito: **imediato** (job dispara ao ativar),
**rascunho** (só vale após `Apply`), **confirmado** (passa pela confirmação e
então dispara).

---

## Resumo dos achados

1. **Catálogo de ícones com codepoints errados.** 15 das 54 constantes de
   `argvus-tui::icons` não desenham o glyph que o comentário declara (ex.:
   `NETWORK` desenha `message_cog`, `VPN` desenha `walk`, `BOOT` desenha
   `assistant`, `FONTS` desenha `crop_free`). Detalhes na seção 1.1. Isto afeta
   todas as telas e precisa ser corrigido na Fase 1.
2. **Barra de botões em 11 crates** (`appearance`, `boot`, `packages`,
   `network`, `audio`, `bluetooth`, `hardware`, `services`, `displays`,
   `power`, `session`) e **uma variante própria** no `settings` (botões
   `argvus_tui::buttons` no fim da lista + linhas de texto `[ ... ]`). O
   diagnóstico inicial listava 7; `displays`, `power` e `session` também têm o
   trio `on_buttons`/`button_selected`/`button_from` (com enums
   `DisplayButton`, `PowerButton`, `SessionButton`).
3. **Confirmações: quatro mecanismos**, não três: (a) `ConfirmationState` +
   `draw_confirmation` compartilhados (9 crates); (b) páginas-lista
   `ThemeDeleteConfirm`/`ThemeImportConfirm` no `appearance`; (c) popup próprio
   `popup::draw_confirm` + `confirm_apply_selected` no `settings`; (d) contagem
   regressiva de reversão do `displays` (`RevertState`, teclas `y`/`n`). O
   componente compartilhado atual não tem `y`/`n` e começa com `Cancel`
   selecionado.
4. **Linhas Info recebem foco/realce** em várias páginas desenhadas com
   `page::list` (ex.: Network > DNS e Proxy, Boot > detalhes, Audio > Resumo,
   Bluetooth > Estado, Session > Diagnóstico). Ver seção 3.
5. **Ícone escolhido por índice** na Home (`home_icon_for_item(usize)`) e por
   comparação de texto traduzido nos cabeçalhos (`home_icon_for_header`).
6. **`q` e `?` globais interceptam digitação**: `event.rs:70-77` trata `q`/`?`
   antes de qualquer campo de texto (só o editor de administração e a busca da
   Home são protegidos). Digitar `q` na busca de pacotes, na busca do settings,
   em prompts do appearance/displays/boot encerra o app. Fora do escopo estrito
   de apresentação; listado em Pontos de decisão.
7. **Mouse só funciona na Home** (`event.rs:187`). As páginas de domínio não
   tratam clique; o critério "funciona com mouse" exige decisão (seção 6).
8. **Rodapé**: 43 chaves i18n de ajuda distintas, mais 4 hints montados com
   texto inglês fixo no `appearance` (`ui.rs:2527`, `2552`, `2769`, `2774`).

---

## 0. Inventário de botões e opções (anti-regressão)

Formato: `tela > botão/opção atual > ação executada (momento) > nova linha
equivalente (tipo) > atalho`. A coluna **Status** é marcada nas fases
seguintes (`pendente` → `migrado`). Nenhum item pode ficar sem equivalente.

Atalhos globais preservados em todas as telas: `↑↓/jk`, `PgUp/PgDn`,
`Home/End`, `Enter/→` ativar, `Esc/←` voltar, `?` ajuda, `q` sair,
`Ctrl+C` sair.

### 0.1 `argvus-control-center` (Home, Configuração, roteamento)

| Tela | Botão/opção atual | Ação (momento) | Nova linha equivalente | Atalho | Status |
| --- | --- | --- | --- | --- | --- |
| Home | Cartões com itens (22 ações, `action: usize`) | Abre a rota do domínio | Submenu por item, com `ItemId` estável no lugar do número | `Enter`, clique | pendente |
| Home | Navegação em grade | `Tab`/`BackTab` = próximo/anterior item; `←/→/h/l` = categoria anterior/seguinte | Mantida (Home é grade, não lista); ver decisão D7 | `Tab`, `←→hl` | pendente |
| Home | Busca global | `/` abre busca; Enter abre resultado | Mantida | `/` | pendente |
| Home | Atalho para Configuração | Abre Configuração | Mantido | `s` | pendente |
| Configuração | Ícones | Liga/desliga ícones (imediato) | Toggle | `Enter`/`Space` | pendente |
| Configuração | Transparência do Control Center | Ajuste ±, salva (imediato) | Value (`←/→`) | `←→` | pendente |
| Configuração | Blur do Control Center | Ajuste ±, salva (imediato) | Value (`←/→`) | `←→` | pendente |
| About | Abas | Troca de aba | Mantido (`Tab` legítimo para abas) | `Tab/BackTab/h/l/←→` | pendente |

### 0.2 `argvus-control-center-appearance`

Páginas com **rascunho** (`surface_draft`/`effect_draft`, aplicação só via
`Apply`): Taskbar, TaskbarIcons, TaskbarDate, TaskbarDateFormat, TaskbarTime,
TaskbarTimeFormat, WidgetTelemetry, ControlPanel, SurfaceSection{...} e as
páginas de editor de efeito (`effect_spec`: Blur global, TerminalTransparency,
TransparencySurface). Hoje `Esc` descarta o rascunho **sem perguntar**.

| Tela | Botão/opção atual | Ação (momento) | Nova linha equivalente | Atalho | Status |
| --- | --- | --- | --- | --- | --- |
| Home | Tema · família [modo] | Abre Temas | Submenu | `Enter` | pendente |
| Home | Cor de destaque · valor | Abre Destaques | Submenu | `Enter` | pendente |
| Home | Wallpaper · ativo | Abre Wallpapers | Submenu | `Enter` | pendente |
| Home | Espaços/Bordas/Posição | Abre página | Submenu | `Enter` | pendente |
| Home | Taskbar | Abre página | Submenu | `Enter` | pendente |
| Home | Efeitos | Abre página | Submenu | `Enter` | pendente |
| Home | Widget Telemetry | Abre página | Submenu | `Enter` | pendente |
| Home | Control Panel | Abre página | Submenu | `Enter` | pendente |
| Home | Terminal | Abre página | Submenu | `Enter` | pendente |
| Home | Launcher | Abre página | Submenu | `Enter` | pendente |
| Home | Modo de aparência · valor | Abre Modo | Submenu | `Enter` | pendente |
| Global | Recarregar | Recarrega estado | Atalho mantido | `r` | pendente |
| Temas | Oficiais › | Abre categorias | Submenu | `Enter` | pendente |
| Temas | Personalizados › | Abre lista | Submenu | `Enter` | pendente |
| Temas | Exportar | Prompt de nome → exporta (imediato) | Action (abre Value) | `Enter`, `e` | pendente |
| Temas | Importar | Abre lista de arquivos | Submenu | `Enter`, `i` | pendente |
| Temas oficiais | Categorias › | Abre famílias | Submenu | `Enter` | pendente |
| Famílias | Tema (· atual) | Aplica tema (imediato) | Choice (`●` no atual) | `Enter` | pendente |
| Temas personalizados | Tema (· atual) | Aplica tema (imediato) | Choice | `Enter` | pendente |
| Temas personalizados | Excluir tema | Abre `ThemeDeleteConfirm` | Destructive (componente único de confirmação), atalho na linha do item | `d` | pendente |
| ThemeDeleteConfirm | `Delete` / `Cancel` (linhas) | Exclui / volta | **Substituído** pelo componente de confirmação (Confirm/Cancel, `y`/`n`) | `Enter`/`Esc` | pendente |
| Importar | Arquivo `.zip` | Importa (imediato, ou pede substituição) | Action por arquivo | `Enter` | pendente |
| ThemeImportConfirm | `Replace` / `Cancel` (linhas) | Substitui / volta | **Substituído** pelo componente de confirmação | `Enter`/`Esc` | pendente |
| Prompts (exportar/importar, espaços, bordas) | Campo numérico/texto | Enter confirma (imediato), Esc volta | Value (edição inline/popup) | `0-9`, `Enter`, `Esc` | pendente |
| Modo | Sticky / Float (· atual) | Troca modo (imediato) | Choice | `Enter` | pendente |
| Wallpapers | Escolher imagem da home | Abre seletor de arquivo | Action | `Enter` | pendente |
| Wallpapers | Coleções › | Abre modos | Submenu | `Enter` | pendente |
| Modos de wallpaper | Modo › | Abre itens | Submenu | `Enter` | pendente |
| Itens de wallpaper | Arquivo (· atual) | Aplica wallpaper (imediato) | Choice | `Enter` | pendente |
| Destaques | Editar cor: `#hex` | Abre AccentEdit (hex; Enter aplica, imediato) | Value | `Enter` | pendente |
| Destaques | Restaurar padrão do tema | Reseta acento (imediato) | Action | `Enter` | pendente |
| Efeitos | `[x] Animações` | Alterna (imediato) | Toggle | `Enter`/`Space` | pendente |
| Efeitos | `[x] Blur` | Alterna (imediato) | Toggle | `Enter`/`Space` | pendente |
| Efeitos | Blur › N% | Abre editor de efeito | Submenu | `Enter` | pendente |
| Editor de efeito (Blur global, Terminal/Launcher transparência) | Valor N% | Ajusta rascunho ±5 / 0 / 100 | Value | `←→ h l + -`, `Home/End` | pendente |
| Editor de efeito | **`[ Apply ]`** (botão, `Tab`) | Aplica valor (rascunho → imediato) | Action `Apply` no fim da lista, desabilitada sem mudança | `Enter` na linha | pendente |
| Terminal | `[x] Transparência` | Alterna (imediato) | Toggle | `Enter`/`Space` | pendente |
| Terminal | Transparência › N% | Abre editor de efeito | Submenu | `Enter` | pendente |
| Launcher | `[x] Transparência` | Alterna (imediato) | Toggle | `Enter`/`Space` | pendente |
| Launcher | Transparência › N% | Abre editor de efeito | Submenu | `Enter` | pendente |
| Espaços/Bordas/Posição | Posição da taskbar / Espaços da taskbar / Espaços das janelas / Bordas gerais / Espessura | Abre página | Submenu ×5 | `Enter` | pendente |
| Posição da taskbar | Topo / Base (· atual) | Move taskbar (imediato) | Choice | `Enter` | pendente |
| Espaços da taskbar | Topo/Esquerda/Direita/Base · valor | Prompt (imediato ao confirmar) | Value ×4 | `Enter` | pendente |
| Espaços das janelas | Gap interno, gaps externos ×4 | Prompt (imediato) | Value ×5 | `Enter` | pendente |
| Bordas gerais | `[x] Arredondado` | Alterna (imediato) | Toggle | `Enter`/`Space` | pendente |
| Bordas gerais | Arredondamento · valor (· desativado) | Prompt (imediato) | Value; **desabilitada e pulada** quando Arredondado está desligado | `Enter` | pendente |
| Espessura | Espessura · valor | Prompt (imediato) | Value | `Enter` | pendente |
| Taskbar | Transparência ›, Ícones ›, Data ›, Hora › | Abre páginas | Submenu ×4 | `Enter` | pendente |
| Taskbar e subpáginas | **`[ Apply ]`** | Aplica rascunho da taskbar | Action `Apply` no fim da lista | `Enter` na linha | pendente |
| Ícones da taskbar | `[x]` player de áudio, launcher, widgets utilitários | Alterna no rascunho | Toggle (rascunho) | `Enter`/`Space` | pendente |
| Ícones da taskbar | Utilitários › | Abre agrupamento | Submenu | `Enter` | pendente |
| Data | Formato › | Abre formatos | Submenu | `Enter` | pendente |
| Formato de data/hora | Formato (· atual) | Seleciona no rascunho | Choice (rascunho) | `Enter` | pendente |
| Hora | `[x] Segundos` | Alterna no rascunho | Toggle (rascunho) | `Enter`/`Space` | pendente |
| Widget Telemetry | `[x] Ativar`, Sessões ›, Transparência › | Rascunho / abre páginas | Toggle + Submenu ×2 | `Enter` | pendente |
| Widget Telemetry | **`[ Apply ]`** | Aplica rascunho | Action `Apply` | `Enter` | pendente |
| Control Panel | `[x] Ativar`, Sessões ›, Transparência › | Rascunho / abre páginas | Toggle + Submenu ×2 | `Enter` | pendente |
| Control Panel | **`[ Apply ]`** | Aplica rascunho | Action `Apply` | `Enter` | pendente |
| Seção: ícones utilitários | Sempre expandido / Automático | Seleciona no rascunho | Choice (rascunho) | `Enter` | pendente |
| Seção: sessões | `[x]` blocos/cards | Alterna no rascunho | Toggle (rascunho) | `Enter`/`Space` | pendente |
| Seção: transparência/blur | `[x] Ativar` | Alterna no rascunho | Toggle (rascunho) | `Enter`/`Space` | pendente |
| Seção: transparência/blur | `Valor > N%` | Ajusta rascunho ±5 / 0 / 100 | Value | `+ -`, `Home/End` (ver D3) | pendente |
| Seção: * | **`[ Apply ]`** | Aplica rascunho | Action `Apply` | `Enter` | pendente |
| Páginas com rascunho | `Esc` | Descarta rascunho em silêncio | `Esc` pede confirmação se houver rascunho; linha `Cancel` só se D2 aprovar | `Esc` | pendente |

Páginas mortas (`#[allow(dead_code)]`, inalcançáveis pela navegação):
`Transparency`, `TransparencySurface` (exceto Launchers), `Blur` como página de
lista e `BlurSurface`. Ficam para a Fase 4 se confirmadas.

### 0.3 `argvus-control-center-settings`

O `settings` desenha a própria lista (`ui/list.rs`) e já tem
`row_selectable(index)`. Botões (`page_buttons`) ficam **depois** das linhas
no mesmo cursor (`selected - rows.len()`), desenhados com
`argvus_tui::buttons::draw`; `Tab` alterna entre lista e botões.

| Tela | Botão/opção atual | Ação (momento) | Nova linha equivalente | Atalho | Status |
| --- | --- | --- | --- | --- | --- |
| Aplicativos padrão, Fontes, seletores de app/fonte/ajuste | **`[ Reset Defaults ]`** | `PendingAction::Reset*` → confirmação → reset | Destructive `Restaurar padrões` (linha nova, D6) | `r` (mantido, D6), `Enter` na linha | pendente |
| Atalhos de teclado | **`[ Restore all shortcuts ]`** | `ResetKeybindings` → confirmação | Destructive `Restaurar todos os atalhos` | `r`, `Enter` | pendente |
| Aplicativos padrão | Categorias (11) | Abre seletor | Submenu | `Enter` | pendente |
| Seletor de app | App (atual) | Define app padrão (imediato) | Choice | `Enter`, `/` busca | pendente |
| Fontes | Alvos de fonte (7) + ajustes (4) | Abre seletor | Submenu | `Enter` | pendente |
| Fontes | Tamanho | Ajusta tamanho | Value | `+ = -` | pendente |
| Seletor de fonte/ajuste | Opção (atual) | Aplica (imediato) | Choice | `Enter`, `/` | pendente |
| Localidade e região | Fuso, Data/hora, Locale regional, Locales do sistema, Teclado | Abre páginas | Submenu ×5 | `Enter` | pendente |
| Data e hora | Data/hora local (só sem NTP) | Edita | Value (desabilitada com NTP ligado, como hoje) | `Enter` | pendente |
| Data e hora | Fuso horário | Info | Info | — | pendente |
| Data e hora | NTP automático | `SetNtp` → confirmação | Toggle confirmado | `Enter`/`Space` | pendente |
| Locales do sistema | Locales `[x]` | Marca; aplicar = `ApplySystemLocales` → confirmação | Toggle + Action `Apply` (já confirmada) | `Space`, `Enter` | pendente |
| Teclado | Layout, Variante, Mapa do console | Abre seletores | Submenu ×3 | `Enter` | pendente |
| Teclado | Modelo, Opções | Info (já não selecionáveis) | Info | — | pendente |
| Atalhos de teclado | Atalho (linhas com detalhe) | Abre edição | Submenu | `Enter`, `/` | pendente |
| Edição de atalho | **`[ Change shortcut ]`** | Inicia captura | Action `Alterar atalho` | `Enter`, `e` | pendente |
| Edição de atalho | **`[ Disable ]`** | Desativa atalho (imediato) | Action `Desativar` | `Enter` | pendente |
| Edição de atalho | **`[ Restore default ]`** | Restaura padrão (imediato) | Action `Restaurar padrão` | `Enter` | pendente |
| Captura de atalho | **`[ Apply ]` / `[ Try again ]`** | Aplica atalho capturado / recaptura | Action `Aplicar` (com tecla detectada) ou `Tentar de novo` | `Enter` | pendente |
| Captura de atalho | **`[ Cancel ]`** | Cancela captura | Action `Cancelar` (Esc equivalente) | `Enter`, `Esc` | pendente |
| Conflito de atalho | Popup | Substitui / cancela | Componente de confirmação | `r`, `Esc` | pendente |
| Mouse e touchpad | Velocidade, aceleração, rolagem, mão esquerda, toque, ratbag (dispositivo, perfil, DPI, polling) | Ciclo de valores (imediato) | Toggle / Value / Choice conforme a linha | `←→ h l`, `Space` | pendente |
| Idioma | Idiomas | Troca idioma (imediato) | Choice | `Enter` | pendente |
| Sistema | Hostname | Popup de edição → aplica | Value | `Enter` | pendente |
| Sistema | Usuários, Grupos | Abre páginas | Submenu | `Enter` | pendente |
| Sistema | Não perturbe | Alterna (imediato) | Toggle | `Enter`/`Space` | pendente |
| Sistema | Recarregar | `refresh_system` | Atalho mantido | `r` | pendente |
| Usuários / Grupos | Criar, Lista, Contas do sistema | Abre páginas | Submenu | `Enter` | pendente |
| Lista de usuários / contas do sistema | **`[ Reload ]`** | Recarrega | Action `Recarregar` | `Enter`, `r` (D6) | pendente |
| Usuário | Campos (nome, grupos, shell, grupo primário) | Edita no rascunho | Value / Submenu | `Enter` | pendente |
| Usuário | **`[ Save changes ]`** | `edit` → confirmação | Action `Salvar alterações` (confirmada) | `Enter` | pendente |
| Usuário | **`[ Change password ]`** | Abre página de senha | Submenu | `Enter` | pendente |
| Usuário | **`[ Lock password ]` / `[ Unlock password ]`** | `lock` → confirmação | Action ×2 (confirmadas) | `Enter` | pendente |
| Usuário | **`[ Require password change at login ]`** | `expire-password` → confirmação | Action (confirmada) | `Enter` | pendente |
| Usuário | **`[ Avatar image ]`** | Editor de caminho → confirmação | Value | `Enter` | pendente |
| Usuário | **`[ Remove avatar ]`** | `avatar ""` → confirmação | Destructive | `Enter` | pendente |
| Usuário | **`[ Delete user (keep home) ]`** | `delete` → confirmação | Destructive | `Enter` | pendente |
| Usuário | **`[ Delete user and home ]`** | `delete remove_home` → confirmação | Destructive | `Enter` | pendente |
| Criar usuário | Campos | Rascunho | Value | `Enter` | pendente |
| Criar usuário | **`[ Create account (locked / with password) ]`** | `create` → confirmação | Action (confirmada) | `Enter` | pendente |
| Senha do usuário | Campos + Salvar senha | `password` → confirmação | Value ×3 + Action | `Enter` | pendente |
| Criar grupo | **`[ Create group ]`** | `create-group` → confirmação | Action (confirmada) | `Enter` | pendente |
| Grupo | **`[ Save changes ]`** | `edit-group` → confirmação | Action (confirmada) | `Enter` | pendente |
| Grupo | **`[ Edit members ]`** | Abre membros | Submenu | `Enter` | pendente |
| Grupo | **`[ Delete group ]`** | `delete-group` → confirmação | Destructive | `Enter` | pendente |
| Firewall | Configuração (`y`/`n` → `[x]`) | Alterna no rascunho | Toggle (rascunho) | `Space` | pendente |
| Firewall | **`[ Save configuration ]`** | `save-config` → confirmação | Action `Salvar configuração` (confirmada) | `Enter` | pendente |
| Firewall | **`[ Add iptables rules ]`** | Editor de regras → confirmação | Action (abre editor) | `Enter` | pendente |
| Firewall | **`[ Apply saved rules ]`** | `restart` → confirmação | Action (confirmada) | `Enter` | pendente |
| Firewall | **`[ Cancel ]`** (Danger) | `admin.load(true)`: descarta rascunho recarregando | Action `Cancelar` (descarta e permanece; ver D2) | `Enter` | pendente |
| Editor de administração | Texto multilinha | Salva | Mantido | `Ctrl+S`, `Esc` | pendente |

### 0.4 `argvus-control-center-displays`

Detalhe do monitor edita `self.config` como **rascunho**; `Apply` aplica e abre
a contagem regressiva de reversão.

| Tela | Botão/opção atual | Ação (momento) | Nova linha equivalente | Atalho | Status |
| --- | --- | --- | --- | --- | --- |
| Home | Monitores conectados / configs antigas | Abre detalhe | Submenu | `Enter` | pendente |
| Home | Perfis | Abre perfis | Submenu | `Enter` | pendente |
| Home | **`[ Refresh ]`** | Recarrega | Action `Atualizar` | `Enter`, `r` | pendente |
| Home | **`[ Profiles ]`** | Abre perfis | Sem linha própria. **Equivalente: Enter na linha do item** `Perfis` (Submenu) (D5) | `Enter` | pendente |
| Detalhe (conectado) | Resolução, taxa, escala, posição, orientação, VRR, HDR, primário, ativo, espelho, profundidade, DPMS, brilho/saturação SDR, workspaces | Abre Picker/Prompt; altera rascunho | Choice/Value/Toggle por ajuste | `Enter` | pendente |
| Detalhe (conectado) | **`[ Apply ]`** | Aplica rascunho + reversão em 15 s | Action `Apply` no fim (desabilitada sem mudança) | `Enter` | pendente |
| Detalhe (conectado) | **`[ Default ]`** | `reset_button` (volta ao padrão) | Action `Restaurar padrão` | `Enter` | pendente |
| Detalhe (desconectado) | **`[ Remove config ]`** | Remove config persistida | Destructive | `Enter` | pendente |
| Perfis | **`[ New ]`** | Prompt de nome → salva perfil | Action (abre Value) | `Enter` | pendente |
| Perfis | **`[ Apply ]`** | Aplica perfil selecionado | Action na página do perfil (D4) | `Enter` | pendente |
| Perfis | **`[ Rename ]`** | Prompt de nome | Action (abre Value) | `Enter` | pendente |
| Perfis | **`[ Delete ]`** | `confirm_profile` → exclui | Destructive | `Enter` | pendente |
| Picker | Opção | Aplica ao rascunho / cancela | Choice | `Enter`; `r`/`q` cancelam (`q` hoje é capturado pelo sair global) | pendente |
| Reversão | "Manter configuração?" | Mantém / reverte | Componente de confirmação com prazo | `Enter`/`Space`/`y`, `Esc`/`n` | pendente |

### 0.5 `argvus-control-center-boot` (todas as ações passam por `ConfirmationState`)

| Tela | Botão/opção atual | Ação (momento) | Nova linha equivalente | Atalho | Status |
| --- | --- | --- | --- | --- | --- |
| Home | Resumo, Kernels, Bootloader, Initramfs, Plymouth | Abre páginas | Submenu ×5 (com valor à direita) | `Enter` | pendente |
| Global | Recarregar | `reload` | Atalho mantido | `r` | pendente |
| Kernels | Kernel | Abre detalhe | Submenu | `Enter` | pendente |
| Kernels / detalhe | **`[ Default ]`** | `SystemdDefault` (confirmado) | Action `Definir como padrão` na página do item | `Enter` | pendente |
| Bootloader | Entrada | Abre detalhe | Submenu | `Enter` | pendente |
| Bootloader / detalhe | **`[ Default ]`** | `SystemdDefault` (confirmado) | Action | `Enter` | pendente |
| Bootloader / detalhe | **`[ Timeout ]`** | Input 0–60 → confirmado | Value | `Enter` | pendente |
| Bootloader (GRUB) | **`[ Kernel command line ]`** | Input → `GrubCmdline` (confirmado) | Value | `Enter` | pendente |
| Bootloader (GRUB) | **`[ Regenerate ]`** | `GrubRegenerate` (confirmado) | Destructive `Regenerar GRUB` | `Enter` | pendente |
| Initramfs | Preset | Abre detalhe | Submenu | `Enter` | pendente |
| Initramfs | Última linha "Regenerar todas as imagens" | `Initramfs` (confirmado) | Destructive (já é linha) | `Enter` | pendente |
| Initramfs / detalhe | **`[ Regenerate ]`** | `Initramfs` (confirmado) | Sem linha própria. **Equivalente: Enter na linha do item** `Regenerar todas as imagens` (Destructive, mesma ação `Initramfs`) (D5) | `Enter` | pendente |
| Plymouth | Tema | Aplica (confirmado) | Choice confirmada | `Enter` | pendente |
| Plymouth | **`[ Apply theme ]`** | Igual ao Enter no tema | Sem linha própria. **Equivalente: Enter na linha do item** (Choice do tema) (D5) | `Enter` | pendente |
| Log da transação | Visualizador | Rolagem | Mantido | `PgUp/PgDn/Home/End` | pendente |

### 0.6 `argvus-control-center-packages`

| Tela | Botão/opção atual | Ação (momento) | Nova linha equivalente | Atalho | Status |
| --- | --- | --- | --- | --- | --- |
| Home | Instalar/Oficial, Instalar/AUR, Instalados, Órfãos, Atualizações, Cache, Histórico, Downgrade, Mirrors | Abre páginas | Submenu ×9 | `Enter` | pendente |
| Listas (busca, instalados, órfãos, atualizações, AUR) | Digitar | Filtra (`query`) | Mantido; ver D8 (letras como `r`,`j`,`k` viram texto) | letras, `/` | pendente |
| Listas | Pacote | Abre detalhes | Submenu | `Enter` | pendente |
| Instalados | **`[ Reinstall ]`** | `install_selected` (confirmado) | Action na página do pacote | `Enter` | pendente |
| Instalados / busca / AUR / detalhes | **`[ Remove ]`** | `remove_selected` (confirmado) | Destructive | `Enter` | pendente |
| Busca / AUR / detalhes | **`[ Install ]`** | `install_selected` (confirmado) | Action | `Enter` | pendente |
| Atualizações | Pacote | `UpgradePackage` (confirmado) | Action por item (como hoje) | `Enter` | pendente |
| Atualizações | **`[ Update ]`** | `UpgradePackage` do selecionado | Sem linha própria. **Equivalente: Enter na linha do item** (pacote) (D5) | `Enter` | pendente |
| Atualizações | **`[ Upgrade all ]`** | `begin_plan(Upgrade)` → confirmação | Action `Atualizar tudo` | `Enter` | pendente |
| Atualizações | **`[ Refresh database ]`** | `RefreshDatabase` (confirmado) | Action `Atualizar banco` | `Enter` | pendente |
| Órfãos | **`[ Select ]`** | `toggle_multi` (marca múltiplos) | Toggle por item | `Space` | pendente |
| Órfãos | **`[ Remove ]`** | Remove marcados (confirmado) | Destructive | `Enter` | pendente |
| Cache | **`[ Clean cache ]`** | `CleanCache keep-three` (confirmado) | Destructive | `Enter` | pendente |
| Cache | **`[ Keep one ]`** | `CleanCache keep-one` (confirmado) | Destructive | `Enter` | pendente |
| Cache | **`[ Uninstalled ]`** | `CleanCache uninstalled` (confirmado) | Destructive | `Enter` | pendente |
| Downgrade | **`[ Downgrade ]`** | `Downgrade(path)` (confirmado) | Destructive por item | `Enter` | pendente |
| Mirrors | Lista (readonly) | Enter abre editor reflector | Action `Configurar mirrors` | `Enter` | pendente |
| Editor de mirrors | País, opções | Ciclo de valores | Choice/Value | `←→`, `Space` | pendente |
| Histórico | Entrada | Abre detalhes | Submenu | `Enter` | pendente |
| Global | Recarregar | `reload_force` | Atalho mantido | `r` (fora das listas com digitação) | pendente |

### 0.7 `argvus-control-center-network`

| Tela | Botão/opção atual | Ação (momento) | Nova linha equivalente | Atalho | Status |
| --- | --- | --- | --- | --- | --- |
| Home | Status, Interfaces, Ethernet, Wi-Fi, VPN, DNS, Proxy, Firewall | Abre páginas | Submenu ×8 | `Enter` | pendente |
| Status | **`[ Enable Wi-Fi ]` / `[ Disable Wi-Fi ]`** | `toggle_wifi` (imediato) | Toggle `Wi-Fi` | `Enter`/`Space` | pendente |
| Status, Interfaces, Ethernet, VPN, Wi-Fi, DNS, Proxy, Detalhe | **`[ Refresh ]`** | `start_refresh` (Wi-Fi com rescan) | Action `Atualizar` | `r` | pendente |
| Interfaces / Ethernet | Interface | Abre detalhe | Submenu | `Enter`, `/` filtro | pendente |
| Interfaces / Ethernet / VPN / Wi-Fi | **`[ Connect ]`** | `connect_selected` (imediato) | Action na página do item | `Enter` | pendente |
| Interfaces / Ethernet / VPN / Wi-Fi | **`[ Disconnect ]`** | `disconnect_selected` (imediato) | Action | `Enter` | pendente |
| Wi-Fi | Rede | `connect_wifi` (senha se preciso) | Action (Enter mantém `connect_wifi`); ações secundárias (Disconnect, Forget) na página de detalhe da rede (D4) | `Enter` | pendente |
| Wi-Fi | **`[ Forget ]`** | `confirm_forget` → esquece | Destructive | `Enter` | pendente |
| DNS | **`[ Manual DNS ]`** | Input → aplica | Value | `Enter` | pendente |
| DNS | **`[ Automatic DNS ]`** | `apply_dns("")` (imediato) | Action | `Enter` | pendente |
| Firewall | Página do settings embutida | Igual a 0.3 Firewall | Igual a 0.3 | — | pendente |

### 0.8 `argvus-control-center-audio`

| Tela | Botão/opção atual | Ação (momento) | Nova linha equivalente | Atalho | Status |
| --- | --- | --- | --- | --- | --- |
| Home | Resumo, Saídas, Entradas, Dispositivos | Abre páginas | Submenu ×4 | `Enter` | pendente |
| Saídas / Entradas | Dispositivo | `request_default` (confirmado) | Choice confirmada (`●` no padrão) | `Enter` | pendente |
| Saídas / Entradas | **`[ Default ]`** | Igual ao Enter | Sem linha própria. **Equivalente: Enter na linha do item** (Choice) (D5) | `Enter` | pendente |
| Saídas / Entradas | **`[ Volume + ]` / `[ Volume - ]`** | `adjust_volume(±5)` (imediato) | Value `Volume` com `←/→` | `←→` | pendente |
| Saídas / Entradas | **`[ Mute ]`** | `toggle_mute` (imediato) | Toggle `Mudo` | `Space` | pendente |
| Saídas / Entradas | **`[ Value ]`** | Input numérico de volume | Edição da Value `Volume` | `Enter` | pendente |
| Global | Recarregar | `reload` | Atalho mantido | `r` | pendente |

### 0.9 `argvus-control-center-bluetooth`

| Tela | Botão/opção atual | Ação (momento) | Nova linha equivalente | Atalho | Status |
| --- | --- | --- | --- | --- | --- |
| Home | Estado, Dispositivos, Parear | Abre páginas | Submenu ×3 | `Enter` | pendente |
| Estado | **`[ Power ]`** | Liga/desliga adaptador (imediato) | Toggle `Ligado` | `Enter`/`Space` | pendente |
| Estado | **`[ Discoverable ]`** | Alterna visibilidade (imediato) | Toggle `Visível` | `Enter`/`Space` | pendente |
| Dispositivos | Dispositivo | Conecta/desconecta/pareia conforme estado | Action (Enter mantém conectar/desconectar/parear); ações secundárias (Trust, Remove) na página de detalhe do dispositivo (D4) | `Enter` | pendente |
| Dispositivos / Parear | **`[ Connect ]`**, **`[ Disconnect ]`** | `act_device` (imediato) | Action ×2 | `Enter` | pendente |
| Dispositivos / Parear | **`[ Trust ]`** | trust/untrust (imediato) | Toggle `Confiável` | `Space` | pendente |
| Dispositivos / Parear | **`[ Remove ]`** | `confirm_remove` → remove | Destructive | `Enter` | pendente |
| Parear | Dispositivo / **`[ Pair ]`** | `act_pair` (imediato, agente) | Action `Parear` | `Enter` | pendente |
| Global | Recarregar | `reload` | Atalho mantido | `r` | pendente |

### 0.10 `argvus-control-center-hardware`

| Tela | Botão/opção atual | Ação (momento) | Nova linha equivalente | Atalho | Status |
| --- | --- | --- | --- | --- | --- |
| Home | Resumo, CPU, GPU, Memória, Energia, Dispositivos | Abre páginas | Submenu ×6 | `Enter` | pendente |
| CPU | Governor (★ ativo) | `SetGovernor` (confirmado) | Choice confirmada | `Enter`, `g` | pendente |
| CPU | **`[ Apply Governor ]`** | Igual ao Enter | Sem linha própria. **Equivalente: Enter na linha do item** (Choice) (D5) | `Enter` | pendente |
| Energia | Perfil (★ ativo) | `SetProfile` (confirmado) | Choice confirmada | `Enter`, `e` | pendente |
| Energia | **`[ Apply Profile ]`** | Igual ao Enter | Sem linha própria. **Equivalente: Enter na linha do item** (Choice) (D5) | `Enter` | pendente |
| GPU / Dispositivos | Item | Abre detalhe | Submenu | `Enter` | pendente |
| Global | Atualizar | `refresh` | Atalho mantido | `r` | pendente |

### 0.11 `argvus-control-center-services`

| Tela | Botão/opção atual | Ação (momento) | Nova linha equivalente | Atalho | Status |
| --- | --- | --- | --- | --- | --- |
| Home | Sistema, Usuário, Falhos, Logs | Abre páginas | Submenu ×4 | `Enter` | pendente |
| Sistema / Usuário / Falhos | Unidade | Abre detalhe (que **já** lista ações como linhas) | Submenu | `Enter`, `/` busca | pendente |
| Sistema / Usuário / Falhos | **`[ Start ]`**, **`[ Stop ]`**, **`[ Restart ]`** | `request` (confirmado) | Action ×3 no detalhe (já existem lá) | `Enter` | pendente |
| Sistema / Usuário / Falhos | **`[ Filter: X ]`** | `cycle_filter` | Choice `Filtro` (cicla) no topo da lista | `Enter`/`←→` | pendente |
| Detalhe | Start, Stop, Restart, Enable, Disable, Enable now, Disable now, Logs | `request` (confirmado) / abre logs | Action ×7 + Submenu (já são linhas) | `Enter` | pendente |
| Logs | **`[ Boot: atual/anterior ]`** | Alterna boot | Toggle `Boot anterior` | `Space` | pendente |
| Logs | **`[ Service: X ]`** | `cycle_log_unit` | Choice `Serviço` | `Enter`/`←→` | pendente |
| Logs | **`[ Priority: X ]`** | `cycle_priority` | Choice `Prioridade` | `Enter`/`←→` | pendente |
| Logs | Entrada | Abre detalhe do log | Submenu | `Enter`, `/` | pendente |
| Global | Atualizar | `refresh` | Atalho mantido | `r` | pendente |

### 0.12 `argvus-control-center-power` (página única)

| Tela | Botão/opção atual | Ação (momento) | Nova linha equivalente | Atalho | Status |
| --- | --- | --- | --- | --- | --- |
| Energia | Tampa fechada (bateria/AC), botão de energia, tela desligada após (×2), bloquear após (×2), manter acordado | Abre picker → aplica (imediato) | Choice/Value (picker) ×7 (3 em desktop) | `Enter` | pendente |
| Energia | **`[ Suspend now ]`** | `Pending::Suspend` (confirmado) | Destructive `Suspender agora` | `Enter` | pendente |
| Energia | **`[ Hibernate now ]`** | `Pending::Hibernate` (confirmado) | Destructive `Hibernar agora` | `Enter` | pendente |
| Energia | **`[ Refresh ]`** | `refresh` | Action `Atualizar` | `r` | pendente |

### 0.13 `argvus-control-center-session`

| Tela | Botão/opção atual | Ação (momento) | Nova linha equivalente | Atalho | Status |
| --- | --- | --- | --- | --- | --- |
| Home | Componentes, Autostart, Diagnóstico, Logs | Abre páginas | Submenu ×4 | `Enter` | pendente |
| Componentes | Componente / **`[ Restart ]`** | `Pending::Restart` (confirmado) | Action por item (Enter, como hoje) | `Enter` | pendente |
| Autostart | Entrada / **`[ Enable ]`** / **`[ Disable ]`** | `ToggleAutostart` (confirmado) | Toggle confirmado por item | `Enter`/`Space` | pendente |
| Logs | **`[ Service: X ]`** | `toggle_log_filter` | Choice `Serviço` | `Enter`/`←→` | pendente |
| Logs | Entrada | Abre detalhe | Submenu | `Enter` | pendente |
| Componentes, Autostart, Logs | **`[ Refresh ]`** | `refresh` | Action `Atualizar` | `r` | pendente |

### 0.14 `storage`, `diagnostics`, `about`, `apps`

Sem botões. `storage` e `diagnostics`: Home com submenus, listas que abrem
detalhes, detalhes em `readonly`, `r` atualiza. `about`: abas + rolagem.
`apps`: sem UI própria (backend/CLI usado pelo settings).

---

## 1. Tabela de ícones

### 1.1 Catálogo `argvus-tui::icons`: codepoints que não batem com o nome

Verificado contra os nomes de glyph da Symbols Nerd Font Mono instalada.
Codepoint correto = glyph com o nome do comentário.

| Constante | Atual | Desenha hoje | Nome pretendido | Codepoint correto |
| --- | --- | --- | --- | --- |
| `NETWORK` | `f06f1` | message_cog | nf-md-network | `f06f3` |
| `WIFI` | `f0928` | wifi_strength_4 | nf-md-wifi | `f05a9` (ou manter, D9) |
| `VPN` | `f0583` | walk | nf-md-vpn | `f0582` |
| `DNS` | `f0155` | clock_start | nf-md-dns | `f01d6` |
| `GPU` | `f0fb2` | expansion_card_variant | nf-md-expansion-card | `f08ae` (ou manter) |
| `BOOT` | `f0064` | assistant | nf-md-boot (**não existe**) | proposta: `power_cycle` `f0901` |
| `PACKAGES` | `f03d7` | package_variant_closed | nf-md-package-variant | `f03d6` (ou manter) |
| `INSTALLED` | `f03d9` | palette_advanced | nf-md-package-check (**não existe**) | proposta: `package_variant_closed_plus` `f19d5` |
| `DIAGNOSTICS` | `f0151` | clock_end | nf-md-chart-box | `f154d` |
| `FONTS` | `f019f` | crop_free | nf-md-format-font | `f06d6` |
| `IMAGE` | `f02f9` | image_multiple | nf-md-image | `f02e9` |
| `USERS` | `f000d` | account_minus | nf-md-account-multiple | `f000e` |
| `BELL` | `f009c` | bell_outline | nf-md-bell | `f009a` |
| `BELL_OFF` | `f009e` | bell_ring | nf-md-bell-off | `f009b` |
| `PDF` | `f0e2d` | file_png_box | nf-md-file-pdf-box | `f0226` |

As outras 39 constantes conferem. Além disso, `Appearance > Modo > Float` usa o
emoji literal `"🪟"` (largura 2, fora do catálogo).

### 1.2 Novos glyphs propostos para o catálogo (todos conferidos na fonte)

| Constante proposta | Glyph | Codepoint | Uso |
| --- | --- | --- | --- |
| `TASKBAR` | nf-md-dock_bottom | `f10a9` | Taskbar |
| `WALLPAPER` | nf-md-wallpaper | `f0e09` | Wallpaper |
| `TERMINAL` | nf-md-console | `f018d` | Terminal |
| `LAUNCHER` | nf-md-rocket_launch | `f14de` | Launcher |
| `TELEMETRY` | nf-md-gauge | `f029a` | Widget Telemetry |
| `CONTROL_PANEL` | nf-md-tune_variant | `f1542` | Control Panel |
| `LAYOUT` | nf-md-view_dashboard | `f056e` | Espaços/Bordas/Posição |
| `THEME_MODE` | nf-md-theme_light_dark | `f050e` | Modo de aparência |
| `WINDOW_FLOAT` | nf-md-window_restore | `f05b2` | Modo Float |
| `WINDOW_STICKY` | nf-md-view_split_vertical | `f0bcc` | Modo Sticky |
| `ACCENT` | nf-md-format_color_fill | `f0266` | Cor de destaque |
| `ARROW_UP` / `ARROW_DOWN` / `ARROW_LEFT` / `ARROW_RIGHT` | nf-md-arrow_*_bold | `f0737` / `f072e` / `f0731` / `f0734` | Posição e espaços por lado |
| `GAP` | nf-md-arrow_expand_horizontal | `f084e` | Gaps |
| `BORDER` | nf-md-border_style | `f00d0` | Bordas gerais |
| `ROUNDED` | nf-md-rounded_corner | `f0607` | Arredondado/arredondamento |
| `THICKNESS` | nf-md-format_line_weight | `f05c9` | Espessura |
| `OPACITY` | nf-md-opacity | `f05cc` | Transparência |
| `BLUR` | nf-md-blur | `f00b5` | Blur |
| `ANIMATION` | nf-md-animation_play | `f093a` | Animações |
| `CALENDAR` | nf-md-calendar | `f00ed` | Data |
| `TIMER` | nf-md-timer_outline | `f051b` | Tempo limite |
| `TRANSLATE` | nf-md-translate | `f05ca` | Idioma |
| `EARTH` | nf-md-earth | `f01e7` | Localidade/fuso/região |
| `APPLY` | nf-md-check_bold | `f0e1e` | Linha `Apply` |
| `CANCEL` | nf-md-cancel | `f073a` | Linha `Cancel` |
| `DELETE` | nf-md-delete | `f01b4` | Ações destrutivas de exclusão |
| `RESTORE` | nf-md-restore | `f099b` | Restaurar padrão |
| `IMPORT` / `EXPORT` | nf-md-import / nf-md-export | `f02fa` / `f0207` | Temas |
| `EDIT` | nf-md-pencil | `f03eb` | Editar/renomear |
| `ADD` | nf-md-plus | `f0415` | Novo/criar |
| `PLAY` / `STOP` / `RESTART` | nf-md-play / stop / restart | `f040a` / `f04db` / `f0709` | Serviços, componentes |
| `FILTER` | nf-md-filter_variant | `f0236` | Filtros de lista/log |
| `SLEEP` | nf-md-power_sleep | `f0904` | Suspender |
| `HIBERNATE` | nf-md-snowflake | `f0717` | Hibernar |
| `LINK_ON` / `LINK_OFF` | nf-md-lan_connect / lan_disconnect | `f0318` / `f0319` | Conectar/desconectar |
| `SHIELD` | nf-md-shield_outline | `f0499` | Firewall |
| `CLEAN` | nf-md-broom | `f00e2` | Limpar cache |
| `DOWNGRADE` | nf-md-package_down | `f03d4` | Downgrade |
| `DATABASE` | nf-md-database_refresh | `f05c2` | Atualizar banco |
| `CHIP_CPU` | já existe `CPU` | `f0ee0` | CPU |
| `DEVICES` | nf-md-devices | `f0fb0` | Dispositivos |
| `PROFILE` | nf-md-layers_outline | `f09fe` | Perfis |
| `AUTOSTART` | nf-md-launch | `f0327` | Autostart |
| `USB` | nf-md-usb | `f0553` | Dispositivos de hardware |
| `VOLUME_UP` / `VOLUME_DOWN` / `MUTE` | nf-md-volume_plus / volume_minus / volume_off | `f075d` / `f075e` / `f0581` | Áudio |
| `TRUST` | nf-md-shield_check | `f0565` | Bluetooth confiável |
| `VISIBLE` | nf-md-eye | `f0208` | Bluetooth visível |

### 1.3 Home (`argvus-control-center/src/ui.rs`)

Cabeçalhos de cartão (`home_icon_for_header`, por texto traduzido):

| Cartão | Ícone atual | Ícone proposto |
| --- | --- | --- |
| Idioma e região | `NETWORK` (desenha message_cog) | `EARTH` |
| Aparência | `PALETTE` | `PALETTE` |
| Aplicativos | `APPS` | `APPS` |
| Hardware | `MONITOR` | `HARDWARE` (chip) |
| Energia e sessão | `POWER` | `POWER` |
| Conectividade | `NETWORK` | `NETWORK` (corrigido) |
| Áudio | `AUDIO` | `AUDIO` |
| Sistema | `SETTINGS` | `SERVICES` (server) |
| Preferências | `SETTINGS` | `SETTINGS` |

Itens (`home_icon_for_item(action: usize)`, por índice):

| Item (índice) | Ícone atual | Problema | Ícone proposto |
| --- | --- | --- | --- |
| Aplicativos padrão (0) | `APPS` | — | `APPS` |
| Fontes (1) | `FONTS` | glyph errado (crop_free) | `FONTS` (corrigido) |
| Localidade e região (2) | `NETWORK` | sentido errado | `EARTH` |
| Idioma (3) | `KEYBOARD` | sentido errado | `TRANSLATE` |
| Hardware (4) | `MONITOR` | repete com Displays (irmãos) | `HARDWARE` |
| Rede (5) | `NETWORK` | glyph errado | `NETWORK` (corrigido) |
| Áudio (6) | `AUDIO` | — | `AUDIO` |
| Bluetooth (7) | `LINK` | genérico | `BLUETOOTH` |
| Boot (8) | `BOOT` | glyph errado (assistant) | `BOOT` (power_cycle) |
| Pacotes (9) | `PACKAGES` | — | `PACKAGES` |
| Serviços (10) | `SETTINGS` | repete com Sistema/Configuração | `SERVICES` |
| Sistema (11) | `SETTINGS` | repete com Serviços/Configuração | `SETTINGS` (passa a ser o único com cog) |
| Armazenamento (12) | `STORAGE` | — | `STORAGE` |
| Diagnóstico (13) | `DIAGNOSTICS` | glyph errado (clock_end) | `DIAGNOSTICS` (corrigido) |
| Sobre (14) | `INFO` | — (é informação) | `INFO` |
| Configuração (15) | `SETTINGS` | repete com Sistema | `CONTROL_PANEL` (tune) |
| Energia (16) | `POWER` | — | `POWER` |
| Sessão (17) | `REFRESH` | sentido errado | `USER` (sessão do usuário) |
| Displays (18) | `MONITOR` | repete com Hardware | `MONITOR` |
| Aparência (19) | `PALETTE` | — | `PALETTE` |
| Mouse e touchpad (20) | `MOUSE` | — | `MOUSE` |
| Atalhos de teclado (21) | `KEYBOARD` | repete com Idioma | `KEYBOARD` |

### 1.4 `appearance`

| Tela | Item | Ícone atual | Ícone proposto |
| --- | --- | --- | --- |
| Home | Tema | `PALETTE` | `PALETTE` |
| Home | Cor de destaque | `PALETTE` (repete Tema) | `ACCENT` |
| Home | Wallpaper | `IMAGE` | `WALLPAPER` |
| Home | Espaços/Bordas/Posição | `STORAGE` (HDD) | `LAYOUT` |
| Home | Taskbar | `STORAGE` (HDD) | `TASKBAR` |
| Home | Efeitos | `SUCCESS` (ok) | `EFFECT` |
| Home | Widget Telemetry | `WIDGET` (repete Control Panel) | `TELEMETRY` |
| Home | Control Panel | `WIDGET` | `CONTROL_PANEL` |
| Home | Terminal | `STORAGE` (HDD) | `TERMINAL` |
| Home | Launcher | `STORAGE` (HDD) | `LAUNCHER` |
| Home | Modo de aparência | `STORAGE` (HDD) | `THEME_MODE` |
| Temas | Oficiais ›, Personalizados › | sem ícone | `PALETTE`, `EDIT` |
| Temas | Exportar / Importar | `STORAGE` / `STORAGE` | `EXPORT` / `IMPORT` |
| Modo | Sticky / Float | `FOLDER` / emoji `🪟` | `WINDOW_STICKY` / `WINDOW_FLOAT` |
| Wallpapers | Escolher da home | `FOLDER` | `FOLDER` |
| Wallpapers | Coleções / Modos | `IMAGE` (todos iguais) | `IMAGE` (mesmo sentido: aceito) |
| Destaques | Editar / Restaurar | sem ícone | `EDIT` / `RESTORE` |
| Efeitos | Animações / Blur (toggle) / Blur % | `SUCCESS` / `SUCCESS` / `INFO` | `ANIMATION` / `BLUR` / `BLUR` (Value; mesmo sentido) |
| Terminal, Launcher | Transparência (toggle) / % | sem ícone | `OPACITY` |
| Espaços/Bordas/Posição | Posição da taskbar | `INFO` | `TASKBAR` |
| Espaços/Bordas/Posição | Espaços da taskbar / das janelas | `STORAGE` / `STORAGE` | `GAP` / `WINDOW_STICKY` (→ D9) |
| Espaços/Bordas/Posição | Bordas gerais / Espessura | `INFO` / `INFO` | `BORDER` / `THICKNESS` |
| Posição da taskbar | Topo / Base | `INFO` | `ARROW_UP` / `ARROW_DOWN` |
| Espaços da taskbar | Topo/Esquerda/Direita/Base | `INFO` | setas por lado |
| Espaços das janelas | Gap interno / externos ×4 | `INFO` | `GAP` / setas por lado |
| Bordas gerais | Arredondado / Arredondamento | `INFO` | `ROUNDED` |
| Espessura | Espessura | `INFO` | `THICKNESS` |
| Taskbar | Transparência / Ícones / Data / Hora | sem ícone | `OPACITY` / `APPS` / `CALENDAR` / `CLOCK` |
| Todas as páginas com rascunho | `Apply` (botão) | — | `APPLY` |

### 1.5 Demais crates

| Crate > tela | Item | Ícone atual | Ícone proposto |
| --- | --- | --- | --- |
| audio > Home | Resumo / Saídas / Entradas / Dispositivos | `MONITOR` / `AUDIO` / `MICROPHONE` / `SPEAKER` | `INFO` / `SPEAKER` / `MICROPHONE` / `DEVICES` |
| audio > Resumo | cabeçalhos de seção | `MONITOR`, `AUDIO`, `SPEAKER` | sem ícone (Info) |
| boot > Home | Resumo / Kernels / Bootloader / Initramfs / Plymouth | `MONITOR` / `MEMORY` / `STORAGE` / `PACKAGES` / `PALETTE` | `INFO` / `CPU` / `BOOT` / `PACKAGES` / `IMAGE` |
| boot > detalhes | cabeçalhos | `MEMORY`, `STORAGE`, `PACKAGES` | sem ícone (Info) |
| diagnostics > Home | Resumo, Serviços, Kernel e boot, Gráficos, Rede, Áudio, Bluetooth, Armazenamento, Pacotes, ARGVUS | `MONITOR`, `SETTINGS`, `MEMORY`, `GPU`, `NETWORK`, `AUDIO`, `LINK`, `STORAGE`, `PACKAGES`, `SUCCESS` | `INFO`, `SERVICES`, `BOOT`, `GPU`, `NETWORK`, `AUDIO`, `BLUETOOTH`, `STORAGE`, `PACKAGES`, `PALETTE`/logo (D9) |
| diagnostics | `category_icon` (mapa duplicado do anterior) | idem | unificar com o item |
| displays > Home | Monitor / desconectado / Perfis | `MONITOR` / `ETHERNET` / `APPS` | `MONITOR` / `LINK_OFF` / `PROFILE` |
| displays > detalhe | "Monitor desconectado" | `ETHERNET` | `WARNING` |
| hardware > Home | Resumo / CPU / GPU / Memória / Energia / Dispositivos | `MONITOR` / `MEMORY` / `GPU` / `POWER` / `BATTERY` / `ETHERNET` | `INFO` / `CPU` / `GPU` / `MEMORY` / `BATTERY` / `USB` |
| network > Home | Status / Interfaces / Ethernet / Wi-Fi / VPN / DNS / Proxy / Firewall | `NETWORK` / `ETHERNET` / `NETWORK` / `WIFI` / `LOCK` / `SEARCH` / `LOCK` / `WARNING` | `INFO` / `NETWORK` / `ETHERNET` / `WIFI` / `VPN` / `DNS` / `LINK` / `SHIELD` |
| network > páginas | cabeçalhos de seção | `NETWORK`, `WIFI`, `ETHERNET`, `SEARCH`, `LOCK` | sem ícone (Info) |
| packages > Home | Instalar oficial / AUR / Instalados / Órfãos / Atualizações / Cache / Histórico / Downgrade / Mirrors | `SEARCH` / `SUCCESS` / `PACKAGES` / `ERROR` / `REFRESH` / `STORAGE` / `LOGS` / `UPDATE` / `NETWORK` | `SEARCH` / `ADD` / `INSTALLED` / `CLEAN` / `UPDATE` / `DATABASE` / `HISTORY` / `DOWNGRADE` / `NETWORK` |
| packages > detalhes | cabeçalhos | `PACKAGES`, `LINK`, `SETTINGS` | sem ícone (Info) |
| power | Tampa (bateria) / Tampa (AC) / Botão / Tela desligada ×2 / Bloquear ×2 / Manter acordado | `MONITOR` / `ETHERNET` / `POWER` / `MONITOR` ×2 / `LOCK` ×2 / `MONITOR` | `BATTERY` / `power_plug` `f06a5` (novo) / `POWER` / `MONITOR` / `LOCK` / `SLEEP` (mesmo ícone em bateria e AC: mesmo sentido) |
| services > Home | Sistema / Usuário / Falhos / Logs | `SETTINGS` / `USER` / `WARNING` / `LOGS` | `SERVICES` / `USER` / `WARNING` / `LOGS` |
| session > Home | Componentes / Autostart / Diagnóstico / Logs | `APPS` / `BOOT` / `DIAGNOSTICS` / `LOGS` | `APPS` / `AUTOSTART` / `DIAGNOSTICS` / `LOGS` |
| settings > Localidade | Fuso / Data e hora / Locale regional / Locales do sistema / Teclado | `NETWORK` / `HISTORY` / `NETWORK` / `APPS` / `KEYBOARD` | `CLOCK` / `CALENDAR` / `EARTH` / `TRANSLATE` / `KEYBOARD` |
| settings > Data e hora | Local / Fuso / NTP / RTC | `HISTORY` / `NETWORK` / `SATELLITE` / `BATTERY` | `CALENDAR` / `CLOCK` / `SATELLITE` / sem ícone (Info) |
| settings > Teclado | Layout / Variante / Modelo / Opções / Console | `KEYBOARD` / `FONTS` / `MOUSE` / `SETTINGS` / `MONITOR` | `KEYBOARD` / `EDIT` / sem ícone / sem ícone / `TERMINAL` |
| settings > Sistema | Hostname / Usuários / Grupos / Não perturbe | `MONITOR` / `USER` / `USERS` / `BELL(_OFF)` | `MONITOR` / `USER` / `USERS` / `BELL`/`BELL_OFF` (codepoints corrigidos) |
| settings > Apps padrão | Navegador / Launcher / Terminal | `NETWORK` / `BOOT` / `MONITOR` | `EARTH` / `LAUNCHER` / `TERMINAL` |
| settings > Fontes | Taskbar / Sysinfo / Control Panel / Sistema / Apps / Terminal / Navegador | `MONITOR` / `DIAGNOSTICS` / `SETTINGS` / `SERVICES` / `PACKAGES` / `KEYBOARD` / `NETWORK` | `TASKBAR` / `DIAGNOSTICS` / `CONTROL_PANEL` / `SETTINGS` / `APPS` / `TERMINAL` / `EARTH` |
| settings > Fontes | Antialiasing / Hinting / Subpixel / DPI | `SUCCESS` / `SEARCH` / `PALETTE` / `STORAGE` | `EFFECT` / `RULER` / `PALETTE` / `MONITOR` (→ D9) |
| storage > Home | Resumo / Discos / Partições / Sistemas de arquivos / Pontos de montagem / SMART / Uso | `STORAGE` ×5, `LOCK`, `STORAGE` | `INFO` / `STORAGE` / `LAYOUT` / `FOLDER` / `LINK` / `DIAGNOSTICS` / `DIAGNOSTICS` (SMART e Uso repetem: → D9) |
| storage | `disk_icon`, `partition_icon` (SWAP = `REFRESH`, cifrado = `LOCK`) | — | SWAP = `MEMORY`; manter `LOCK` |

Itens marcados com "→ D9" têm mais de uma proposta razoável; a escolha final
fica para a Fase 1 com revisão visual.

---

## 2. Idiomas por crate

| Crate | Navegação | Botões | Confirmação | Marcadores | Rodapé |
| --- | --- | --- | --- | --- | --- |
| `argvus-control-center` (Home) | Grade de cartões; `Tab` = próximo item; mouse só aqui | — | — | `>` | chave i18n por estado |
| `config_app` | Lista; `←→` ajustam valor | — | — | `>`, `[x]` | 1 chave |
| `about` | Abas (`Tab`), rolagem | — | — | — | 2 chaves |
| `appearance` | `Vec<String>` + `match self.selected`; `Tab` para botão | `Button::new(Apply)` em 9 tipos de página | Páginas-lista Delete/Cancel e Replace/Cancel | `[x]`, ` · atual`, ` · valor`, `›`, ` >`, ` > N%` | 4 chaves + 4 hints em inglês fixo |
| `settings` | Lista própria (`ui/list.rs`), `row_selectable`, botões depois das linhas, `Tab` cicla | `argvus_tui::buttons` + linhas `[ ... ]` | Popup próprio `draw_confirm`; popup de conflito (`r`/`Esc`) | `[x]`, `●`/current, detalhe à direita | chaves por página |
| `boot` | `Selection` + `page::list`; `Tab` | `ActionButton` ×5 | `ConfirmationState` | `>`, ` · ` | chaves por página |
| `packages` | `Selection`; digitação filtra | `ActionButton` ×10 | `ConfirmationState` | `>`, `[x]` (multi) | chaves por página |
| `network` | `Selection`; `/` filtro | `ActionButton` ×7 | `ConfirmationState` (`confirm_forget`) | `>`, `●` conectado | chaves por página |
| `audio` | `Selection` | `ActionButton` ×5 | `ConfirmationState` | `>`, `●` padrão | chaves por página |
| `bluetooth` | `Selection` | `ActionButton` ×7 | `ConfirmationState` (`confirm_remove`) | `>` | chaves por página |
| `hardware` | `Selection` | `ActionButton` ×2 | `ConfirmationState` | `>`, `★ ativo` | chaves com `g`/`e` |
| `services` | índice `usize` próprio; `/` busca | `ActionButton` ×7 (rótulos dinâmicos `X: valor`) | `ConfirmationState` | `>` | chaves por página |
| `displays` | índice próprio; Picker e Prompt | `DisplayButton` ×9 | `ConfirmationState` (perfil) + reversão com prazo (`y`/`n`) | `>`, `●` | chaves por página |
| `power` | índice próprio; picker | `PowerButton` ×3 | `ConfirmationState` | `>` | chaves por página |
| `session` | índice próprio | `SessionButton` ×5 | `ConfirmationState` | `>` | chaves por página |
| `storage` | `Selection` + `readonly` | — | — | `>` | 2 chaves |
| `diagnostics` | `Selection` + `readonly` | — | — | `>`, ok/aviso/erro | 2 chaves |

Inconsistências de teclado encontradas:

- `r` = **atualizar** em quase todo lugar, mas = **restaurar padrões**
  (`reset_current`, com confirmação) no `settings` e = **cancelar** no Picker do
  `displays`.
- `←/→`: voltar/abrir em listas; ajustar valor no `config_app`, editores de
  efeito do `appearance` e Mouse e touchpad; navegar botões quando o foco está
  na barra.
- `Home/End`: navegar, exceto na linha de valor de SurfaceSection (define 0/100).
- `Space`: toggle no settings/appearance; ativar botão nos demais.
- Atalhos de página únicos: `e`/`i`/`d` (temas), `g` (governor), `e`
  (perfil de energia e captura de atalho), `+`/`-`/`=` (fontes e efeitos).

---

## 3. Linhas Info hoje selecionáveis

Linhas que recebem o marcador `>`/realce sem ter ação (ou que o cursor
alcança sem efeito). "Realce sem cursor" = `row_count` 0 mas `page::list`
desenha a linha 0 selecionada.

| Crate > tela | Linhas | Sintoma |
| --- | --- | --- |
| network > DNS | cabeçalho "DNS CONFIGURATION" e valores | realce sem cursor na linha 0 |
| network > Proxy | cabeçalho "PROXY CONFIGURATION" e valores | realce sem cursor |
| boot > Kernel/Bootloader/Initramfs detalhe | todas (cabeçalho + campos) | `selection_len` = 1, linha 0 realçada |
| audio > Resumo | cabeçalho "AUDIO SYSTEM" + campos | `row_count` = 1, linha 0 realçada |
| audio > Dispositivos | lista de dispositivos | navegável, Enter sem efeito |
| bluetooth > Estado | 2 linhas (estado do adaptador) | navegáveis, Enter sem efeito (ações só nos botões) |
| hardware > GPU / Dispositivos | itens | ok (abrem detalhe) |
| services > LogDetail | conteúdo | `selection_len` = 1 |
| session > Diagnóstico | itens de diagnóstico | navegáveis, Enter sem efeito |
| session > LogDetail | conteúdo | `selection_len` = 1 |
| packages > Mirrors | lista (readonly) | ok (readonly); Enter abre editor sem linha de ação visível |
| displays > Home | configs antigas (stale) | navegáveis; abrem detalhe com `Remove config` (ok) |
| diagnostics > Resumo / detalhe | readonly | ok |
| appearance > Bordas gerais | Arredondamento com Arredondado desligado | selecionável e editável sem efeito visual; deve ficar desabilitada |
| appearance > Home | Tema/Destaque/Wallpaper mostram valor | ok (são Submenu com valor) |
| settings > Data e hora | Fuso (Info) e RTC | já não selecionáveis (ok) |
| settings > Teclado | Modelo, Opções | já não selecionáveis (ok) |
| settings > Usuário / Grupo | cabeçalhos "Account", "Identity", "Information", "Group", "Members" | já não selecionáveis (ok) |
| settings > Firewall | "Service", "Active/Stopped", "Start at boot" | selecionáveis (`row_selectable` retorna `true`) |
| settings > Captura de atalho | "Pressione o novo atalho" | já não selecionável (ok) |

---

## 4. Proposta de `RowKind` e API compartilhada (`argvus-tui`)

Local: novo módulo `argvus-tui/src/menu.rs` (lista + linha), `confirm.rs`
(substitui `components::ConfirmationState`), `hints.rs` (rodapé). `buttons.rs`
fica marcado para remoção na Fase 4, quando nenhum crate o usar.

```rust
/// Identidade estável de um item; substitui índices numéricos.
pub trait RowId: Copy + Eq {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowKind {
  /// Rótulo + valor, nunca recebe foco.
  Info,
  /// Enter executa.
  Action,
  /// Enter abre outra página (marcador `›`).
  Submenu,
  /// Enter/Space alterna; o momento do efeito é decidido pela página.
  Toggle { on: bool },
  /// Opção exclusiva; `current` desenha `●`.
  Choice { current: bool },
  /// Número ou texto; Enter edita, `←/→` ajusta quando `step` existe.
  Value { text: String, step: Option<Step> },
  /// Ação que sempre passa pelo componente de confirmação.
  Destructive,
  /// Linha separadora visual (não selecionável).
  Separator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step { pub delta: i32, pub min: i32, pub max: i32 }

#[derive(Debug, Clone)]
pub struct Row<Id> {
  pub id: Id,
  pub kind: RowKind,
  /// Glyph do catálogo (`icons::*`); resolvido por `AppConfig::icon` ao desenhar.
  pub icon: Option<&'static str>,
  pub label: String,
  /// Valor à direita (Info/Submenu/Value).
  pub detail: Option<String>,
  /// Desabilitada: desenhada em `muted` e pulada pelo cursor.
  pub enabled: bool,
  /// Ênfase de tema (Primary para `Apply`, Danger para destrutivas).
  pub emphasis: Emphasis,
}

/// Cursor que só para em linhas selecionáveis.
#[derive(Debug, Clone, Copy, Default)]
pub struct MenuState { selected: Option<usize>, offset: usize }

impl MenuState {
  pub fn selectable(row: &Row<impl Copy>) -> bool; // !Info && !Separator && enabled
  pub fn normalize<Id>(&mut self, rows: &[Row<Id>]);  // primeira selecionável ou None
  pub fn handle<Id>(&mut self, key: KeyCode, rows: &[Row<Id>], page: usize) -> MenuEvent<Id>;
  pub fn selected_id<Id: Copy>(&self, rows: &[Row<Id>]) -> Option<Id>;
}

/// O que a página deve fazer; a página mantém a regra de negócio.
pub enum MenuEvent<Id> {
  None,
  Moved,
  Activate(Id),        // Enter/→ em Action, Submenu, Choice, Value (abrir edição)
  Toggle(Id),          // Space/Enter em Toggle
  Adjust(Id, i32),     // ←/→ em Value com step
  Confirm(Id),         // Enter em Destructive → página abre ConfirmState
  Back,                // Esc/←
}

pub fn draw_menu<Id>(frame: &mut Frame, area: Rect, theme: &Theme, rows: &[Row<Id>], state: &MenuState);

/// Componente único de confirmação.
pub struct ConfirmState { confirm_selected: bool }
pub enum ConfirmOutcome { Pending, Confirmed, Cancelled }
impl ConfirmState {
  pub fn handle(&mut self, key: KeyCode) -> ConfirmOutcome; // ↑↓/Tab/←→ alternam; Enter; Esc; y/n
}
pub struct ConfirmDialog<'a> { pub title: &'a str, pub message: &'a str, pub confirm: &'a str, pub cancel: &'a str, pub danger: bool, pub deadline: Option<Duration> }
pub fn draw_confirm(frame: &mut Frame, area: Rect, theme: &Theme, dialog: ConfirmDialog<'_>, state: &ConfirmState);

/// Rodapé derivado do tipo da linha selecionada + extras da página.
pub struct HintContext<'a> { pub row: Option<&'a RowKind>, pub can_go_back: bool, pub search: bool, pub refresh: bool, pub extra: &'a [(&'a str, &'a str)] }
pub fn hints(lang: Lang, ctx: HintContext<'_>) -> String;
```

Notas de projeto:

- **Ícone pertence ao item**: cada crate expõe `fn icon(id) -> &'static str`
  (ou preenche `Row::icon`) a partir de um enum de itens; `home_icon_for_item`
  e `home_icon_for_header` passam a usar `HomeItem`/`HomeCategory`.
- **Marcadores**: coluna própria antes do ícone: `●` (Choice atual),
  `[x]`/`[ ]` (Toggle; ver D1), `›` à direita (Submenu). Remove ` · atual`,
  `★ ativo`, ` >`, ` > N%`.
- **Apply**: helper `draft_actions(dirty: bool, lang) -> Vec<Row<Id>>` gera
  `Separator` + `Apply` (desabilitada se `!dirty`) e, quando aprovado, `Cancel`.
- **Estado no `settings`**: `row_selectable` e `page_buttons` são substituídos
  pelo `RowKind`; `Row { label, detail, current }` do settings migra para
  `argvus_tui::menu::Row`.
- **Testes da Fase 1** (no `argvus-tui`): pular Info/Separator/desabilitadas em
  `↑↓ jk Home End PgUp PgDn`; lista vazia; página só com Info (sem cursor,
  rolagem); seleção inicial; `Adjust` só em Value com step; `y`/`n` na
  confirmação; rodapé por tipo de linha.

---

## 5. Progresso

| Fase | Item | Status |
| --- | --- | --- |
| 0 | Auditoria (`docs/ux-audit.md`) | concluída (2026-10-04); decisões em 6.1 |
| 1 | Catálogo: corrigir codepoints (1.1) e adicionar glyphs (1.2) | concluída (`argvus-tui` `29808a3`) |
| 1 | `RowKind`, `Row`, `MenuState`, `draw_menu` em `argvus-tui` | concluída (`argvus-tui` `e2fd513`) |
| 1 | Componente único de confirmação (`y`/`n`) | concluída (`argvus-tui` `1d3459d`) |
| 1 | Rodapé contextual (`hints`) | concluída (`argvus-tui` `bee7e32`, `argvus-i18n` `0c2d823`) |
| 1 | Testes de cursor e confirmação | concluída (junto com cada componente) |
| 1 | Campo de texto ativo captura `q`/`?` (D10) | concluída (`argvus-control-center` `5b346ca`) |
| 2 | `appearance` | pendente |
| 3 | `settings` | pendente |
| 3 | `displays` | pendente |
| 3 | `boot` | pendente |
| 3 | `packages` | pendente |
| 3 | `network` | pendente |
| 3 | `services` | pendente |
| 3 | `audio` | pendente |
| 3 | `bluetooth` | pendente |
| 3 | `power` | pendente |
| 3 | `storage` | pendente |
| 3 | `hardware` | pendente |
| 3 | `diagnostics` | pendente |
| 3 | `session` | pendente |
| 3 | `about` | pendente |
| 3 | Home (`argvus-control-center`) | pendente |
| 4 | Chaves i18n órfãs (en-US e pt-BR) | pendente |
| 4 | Código morto (páginas `Transparency`/`Blur*`, `argvus_tui::buttons`) | pendente |
| 4 | README "Keyboard controls" e entrada no CHANGELOG | pendente |

### 5.1 Notas da Fase 1 (2026-10-04)

API entregue em `argvus-tui` (nada no Control Center usa ainda; as telas
migram nas Fases 2 e 3):

- `menu::{RowKind, Row, Emphasis, draft_actions, MenuState, MenuEvent,
  MenuStyle, draw_menu}`. `Row::value(id, label, text, step)` guarda o valor
  em `detail`; `MenuState::select(rows, &id)` restaura a seleção por id.
- `confirm::{ConfirmState, ConfirmOutcome, ConfirmDialog, draw_confirm}`. O
  `components::ConfirmationState` antigo continua até a migração dos crates.
- `hints::{HintContext, hint_keys, hints, confirm_hint_keys, confirm_hints}`,
  com chaves `control_center.hint.*` e `control_center.confirm` (en-US e
  pt-BR) no `argvus-i18n`.
- Catálogo `icons`: das 15 constantes da seção 1.1, 12 tiveram o codepoint
  corrigido; `WIFI`, `GPU` e `PACKAGES` mantiveram o glyph (variantes de mesmo
  sentido) e o comentário passou a nomear o glyph real. 56 constantes novas
  (seção 1.2 mais `POWER_PLUG`), todas conferidas na fonte instalada.

Telas que mudam de comportamento com o D10 (`q`/`?` passam a ser texto):

| Crate | Tela / estado | Antes |
| --- | --- | --- |
| `packages` | Instalar (oficial), AUR, Instalados, Órfãos, Atualizações: listas que filtram ao digitar | `q` saía, `?` abria a ajuda |
| `packages` | Prompt de entrada (`input`) | idem |
| `settings` | Busca ativa (`/`) em qualquer página | idem |
| `settings` | Edição de hostname | idem |
| `settings` | Captura de atalho de teclado | idem (`Super+Q` também saía) |
| `network` | Senha Wi-Fi, DNS manual, filtro de lista (`/`) | idem |
| `network` | Firewall embutido: editor de administração | idem (só a rota Settings era protegida) |
| `services` | Busca de unidades e de logs | idem |
| `audio` | Campo de valor do volume | idem |
| `boot` | Campo de timeout / linha de comando do kernel | idem |
| `displays` | Prompts (nome de perfil, valores) | idem |
| `appearance` | Prompts (nome de tema, caminho, espaços, bordas) e editor HEX do destaque | idem |

Sem mudança: Home (a busca global já era protegida), editor de
administração na rota Settings (já protegido), e todas as listas que não
filtram ao digitar. Os prompts de texto/número foram incluídos como "campo de
texto ativo" segundo o registro do D10 em 6.1; confirmar se o ajuste "só
filtro ou busca" pretendia excluí-los.

Achados fora do escopo, sem alteração:

- `Ctrl+C` não sai do app enquanto a busca global da Home está ativa (o ramo
  da busca retorna antes); comportamento anterior à Fase 1.
- `boot`: a edição de "Kernel command line" (GRUB) reutiliza `timeout_input`,
  cujo tratamento de teclas só aceita dígitos até 2 caracteres; com o valor
  atual carregado, só é possível apagar, não digitar. Tratar na migração do
  `boot` (Fase 3).
- O cspell não está instalado nesta máquina; `docs/ux-audit.md` já está no
  `ignorePaths`.

---

## 6. Pontos de decisão

### 6.1 Decisões registradas (2026-10-04)

| Ponto | Decisão | Status |
| --- | --- | --- |
| D1 | Toggle como `[x]`/`[ ]` em todo o app. | decidido |
| D2 | Aceito. Sem linha `Cancel` nas páginas com rascunho do `appearance`; `Esc` com rascunho pendente pede confirmação (componente único). | decidido |
| D3 | Em linhas Value com step, `←/→` ajustam; `Home/End` continuam navegando; `Esc` volta; o rodapé mostra a dica de ajuste nessas linhas. | decidido |
| D4 | Enter mantém o que faz hoje na ação principal do item. Ações secundárias (ex.: Forget, Remove, Trust) vão para a página de detalhes do item, mantendo os atalhos de uma tecla existentes. | decidido |
| D5 | Aceito, desde que cada botão sem linha própria fique marcado no inventário como "Equivalente: Enter na linha do item" (feito nas seções 0.4, 0.5, 0.6, 0.8 e 0.10). | decidido |
| D6 | Manter `r` = restaurar padrões no `settings`, passando pela confirmação única, e criar também a linha `Restore defaults` / `Restaurar padrões`. | decidido |
| D7 | Recomendação aceita: manter `Tab`/`BackTab` na grade da Home. | decidido (provisório, ver nota) |
| D8 | Recomendação aceita: listas com filtro por digitação mantêm o comportamento atual (letras viram texto da busca). | decidido (provisório, ver nota) |
| D9 | Recomendação aceita: ícones com mais de uma opção são decididos com captura de tela, na migração de cada crate. | decidido (provisório, ver nota) |
| D10 | Incluir. Campo de texto ativo captura as teclas; `q` e `?` globais não fecham o app nem abrem a ajuda durante a digitação. | decidido (Fase 1) |
| D11 | Fora do escopo. Mouse nas páginas de domínio fica para outra tarefa; o critério "com e sem mouse" vale para a Home (que já trata clique) e não é exigido das páginas de domínio neste refactor. | decidido |
| D12 | Recomendação aceita: hints em inglês fixo resolvidos pelo rodapé contextual; chave i18n nova para `Hostname`. | decidido (provisório, ver nota) |
| D13 | `docs/ux-audit.md` entra no `ignorePaths` do `cspell.json`. Verificado: o site só publica `docs/en/` e `docs/pt-br/`; arquivos na raiz de `docs/` são ignorados (`web/argvus-website/src/lib/documentation/loader.ts`, filtro de locale `en`/`pt-br`). | decidido |

Nota sobre D7, D8, D9 e D12: a resposta veio como o modelo
`[aceito as recomendações / minhas respostas]`, sem escolha explícita. Foram
registradas como recomendação aceita; confirmar antes da fase que depende de
cada uma (D7: Home; D8: `packages`; D9: migração de cada crate; D12:
`appearance` e `settings`).

Ajustes aprovados junto com o plano da Fase 1:

- **Foco inicial da confirmação.** Verificado: todos os usos de
  `ConfirmationState` (`audio`, `bluetooth`, `boot`, `displays`, `hardware`,
  `network`, `packages`, `power`, `services`, `session`) criam o estado com
  `::default()`, ou seja, foco em **Cancel**. O componente único mantém Cancel
  como foco inicial; com `danger` ligado o foco também é Cancel, e
  confirmações comuns seguem o mesmo padrão já existente.
- **Cursor nas pontas.** Verificado: nenhuma lista dá a volta hoje
  (`argvus_tui::page::Selection`, `settings::App::move_selection`,
  `App::move_home` e as listas com índice próprio usam
  `saturating_sub`/`clamp`). Só as barras de botões davam a volta
  (`rem_euclid`), e elas serão removidas. A lista única para nas pontas.
- **D10, escopo.** `q` deixa de sair do app só onde a lista já trata
  caracteres digitáveis como filtro ou onde um campo de busca/texto está
  ativo. As telas afetadas estão listadas na seção 5 (Progresso, D10).

### 6.2 Propostas originais

- **D1. Visual do Toggle.** Proposta: manter `[x]`/`[ ]` (ASCII, já usado em
  todo o app e funciona sem Nerd Font). Alternativa: glyph de switch
  (`toggle_switch` `f0521`/`toggle_switch_off_outline` `f0a19`).
- **D2. Linha `Cancel` nas páginas com rascunho do `appearance`.** Hoje não
  existe botão Cancel nelas (só `Apply`; `Esc` descarta em silêncio). Proposta:
  não criar linha `Cancel` (Esc já cumpre o papel) e adicionar a confirmação no
  `Esc` com rascunho pendente. No Firewall do settings, o `Cancel` existente
  vira linha `Cancelar alterações` (recarrega e permanece na página, como hoje).
- **D3. `←/→` em Value.** A regra manda `←/→` ajustar valores, mas hoje `←` é
  "voltar" na linha de valor de SurfaceSection, e `Home/End` definem 0/100 ali.
  Proposta: em linhas Value, `←/→` ajustam e `Home/End` continuam navegando;
  0/100 passam a `Shift+←/→`? Ou manter `Home/End` só durante a edição inline.
- **D4. Ações dependentes do item** (Wi-Fi, Bluetooth, pacotes, perfis de
  display, kernels). Proposta: Enter mantém o efeito atual do item e uma
  página de detalhe do item (já existe em pacotes, kernels, serviços) lista as
  ações como linhas. Para Wi-Fi e Bluetooth isto cria uma página de detalhe
  nova; a alternativa é um submenu de ações aberto com `Enter` e o efeito
  atual de Enter movido para uma linha. Preciso da sua escolha, porque muda o
  que Enter faz em Wi-Fi/Bluetooth.
- **D5. Botões duplicados de ações já disponíveis por Enter** (`[ Apply theme ]`
  no Plymouth, `[ Default ]` no áudio, `[ Apply Governor ]`, `[ Apply Profile ]`,
  `[ Update ]` em Atualizações, `[ Profiles ]` na Home do displays,
  `[ Regenerate ]` no detalhe do initramfs). Proposta: não criar linha extra,
  porque a mesma ação continua na linha do item. Confirmar se aceita.
- **D6. Atalho `r`.** No `settings`, `r` restaura padrões (com confirmação); no
  Picker do `displays`, cancela. Proposta: manter `r` = restaurar no settings
  (preservação de atalho), mas com a confirmação única; no Picker, manter `r` e
  documentar. Alternativa: unificar `r` = atualizar e mover "restaurar" para
  outra tecla (quebra atalho existente).
- **D7. `Tab` na Home.** Hoje `Tab` avança item na grade. A regra diz `Tab`
  só para abas/painéis. Proposta: manter (a grade de cartões é o caso de
  "painéis"), ou remover e deixar só `↑↓←→`.
- **D8. Digitação vs atalhos em listas com filtro** (`packages`): letras viram
  texto da busca, então `r`, `j`, `k` não funcionam nessas páginas. Proposta:
  manter como está nesta refatoração (comportamento existente).
- **D9. Ícones com mais de uma opção** (marcados "→ D9" na seção 1). Proponho
  decidir na Fase 1 com captura de tela e ícones ligados/desligados.
- **D10. `q`/`?` globais engolindo digitação** (achado 6). É um bug funcional
  fora do escopo estrito de apresentação. Corrigir junto (guardar teclas
  quando um campo de texto estiver ativo) ou em tarefa separada?
- **D11. Mouse nas páginas de domínio.** Hoje só a Home trata clique. O
  critério de aceite pede "com e sem mouse". Proposta: a lista única
  (`draw_menu`) devolve a área de cada linha e o roteador passa cliques para a
  página ativa (clique = Enter na linha; Info ignorada). Isto adiciona suporte
  a mouse onde não existia; confirmar se entra no escopo.
- **D12. Hints com texto fixo em inglês** no `appearance` (4 ocorrências) e o
  rótulo literal `"Hostname"` no settings. Proposta: resolver com o rodapé
  contextual (Fase 1/2) e chave i18n nova para Hostname.
- **D13. Local deste arquivo.** `docs/` publica no site (`en/`, `pt-br/` com
  front matter `slug`). Este arquivo não tem front matter e fica fora de
  `en/`/`pt-br/`, mas confirme se a sincronização do site ignora arquivos na
  raiz de `docs/`. O `cspell.json` usa idioma `en`; se o cspell rodar em `docs/`,
  este arquivo em português vai acusar palavras.
