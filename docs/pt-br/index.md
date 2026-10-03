---
title: Control Center
description: Configure as partes persistentes do seu desktop ARGVUS.
slug: pt/0.4.0/docs/user-guide/control-center
---

O Control Center é o aplicativo de configurações do ARGVUS, com navegação orientada ao teclado. Use-o quando quiser configurar o desktop, e não apenas executar uma ação pontual.

## Control Center e Control Panel

| Use… | Para… |
| --- | --- |
| **Control Center** | Temas, wallpapers, efeitos, layout, comportamento da taskbar, fontes, aplicativos padrão, teclado e entrada, idioma, região e configuração do sistema. |
| **Control Panel** | Status atual e ações frequentes, como volume, brilho, rede, notificações, energia e controles da sessão. |

O Control Panel pode oferecer controles rápidos para uma configuração permanente do Control Center. Por exemplo, use o painel para uma ação rápida de aparência ou display, e o Control Center quando quiser alterar a configuração do desktop por trás dela. Veja [Control Panel](/pt/docs/argvus-control-panel/) e [Onde configurar as coisas](/pt/docs/user-guide/where-to-configure/).

## Áreas principais

A tela inicial atual do Control Center oferece estas áreas:

* **Aparência** — temas, modos, acentos, wallpapers, efeitos, layout da taskbar e dos painéis, widgets de telemetria e cards do Control Panel.
* **Aplicativos padrão** — selecione aplicativos instalados para funções compatíveis.
* **Fontes** — escolha os alvos de fonte e as configurações de renderização expostas pelo ARGVUS.
* **Localidade e região** — fuso horário, data e hora, localidade regional, locales do sistema e configuração do teclado.
* **Sistema** — host, usuários, grupos e páginas de administração disponíveis na instalação atual.
* **Displays, rede, sessão e outros domínios** — disponíveis pela busca inicial e pelas rotas de domínio quando os recursos opcionais correspondentes estão instalados.

As páginas exatas dos providers podem depender dos pacotes instalados e das permissões. A ausência de um domínio opcional não significa necessariamente que o Control Center esteja quebrado.

## Encontrando uma configuração

A tela inicial registra configurações por título, categoria, identificador e palavras-chave. A busca ignora acentos e aceita termos práticos como `theme`, `wallpaper`, `shortcut`, `mouse`, `monitor`, `network`, `user` e `DPI`. Correspondências exatas de título aparecem primeiro, seguidas por prefixos e palavras-chave.

O menu visível reconhece as capacidades disponíveis. Uma rota pode não aparecer quando seu provider não está instalado, quando o hardware não está disponível ou quando a sessão atual não oferece a capacidade necessária. Por isso, a busca também é uma forma de descobrir o que esta instalação pode configurar.

A árvore atual de configurações inclui:

| Área | Destinos atuais |
| --- | --- |
| Aparência | Temas, modos de tema, cor de destaque, wallpapers, efeitos, espaços/bordas/posição, posição e espaços da taskbar, grupo utilitário da taskbar, telemetria e cards do Control Panel |
| Entrada e teclado | Mouse e touchpad, layout/variante do teclado, mapa do console, atalhos de teclado |
| Localidade e região | Idioma, fuso horário, data e hora, localidade regional, locales do sistema e teclado |
| Aplicativos | Aplicativos padrão e seletores por categoria |
| Sistema | Hostname, firewall, usuários, grupos e administração do sistema |
| Hardware | Resumo, CPU, GPU, memória, energia e dispositivos |
| Serviços e diagnósticos | Serviços, boot, pacotes, armazenamento e diagnósticos |
| Conectividade | Rede, áudio e Bluetooth |
| Sessão | Componentes, autostart, diagnósticos e logs |
| Displays | Resolução, taxa de atualização, escala, posição, orientação, display principal, VRR e HDR quando suportados |

Alguns itens são páginas completas e outros são subpáginas alcançadas pela busca ou por um resumo de domínio. A lista exata depende dos providers ARGVUS instalados.

## Para que serve cada área

### Aparência

Aparência é a principal área de personalização. Ela inclui temas e modos Sticky/Float, cores de destaque, wallpapers incluídos ou personalizados, efeitos compartilhados, espaçamento da taskbar e das janelas, bordas, espessura das bordas, blocos de telemetria e visibilidade/ordem dos cards do Control Panel. Esses controles coordenam os componentes de aparência e sessão, em vez de alterar apenas a janela do Control Center.

### Fontes

Fontes permite selecionar família e tamanho para taskbar, telemetria/informações do sistema, Control Panel, interface do sistema ARGVUS, aplicativos, terminal e navegador. Também expõe configurações de renderização como antialiasing, hinting, modo subpixel e DPI. A família principal padrão é IBM Plex Mono; cada alvo pode ser alterado independentemente e restaurado pela página Fontes.

### Aplicativos padrão

Aplicativos padrão seleciona programas instalados para funções como terminal, gerenciador de arquivos, editor de texto, editor do terminal, navegador, visualizador de imagens, visualizador de PDF, reprodutor de vídeo, reprodutor de áudio, ferramenta de arquivos compactados e launcher. A seleção é salva pelo ARGVUS e pode atualizar associações XDG padrão.

### Localidade e região

Esta área contém fuso horário, data e hora, localidade regional, locales do sistema e configurações do teclado. As configurações do teclado incluem layout, variante e mapa de teclado do console. A busca inicial também expõe atalhos de teclado e mouse/touchpad como rotas diretas de configuração.

### Hardware e displays

Quando os recursos correspondentes estão instalados, o Control Center pode mostrar informações de CPU, GPU, memória, energia e dispositivos. A página de entrada configura o comportamento do mouse e touchpad; a área de displays lida com resolução, taxa de atualização, escala, posição, orientação, display principal, VRR e HDR quando suportados pela sessão ativa.

### Conectividade e áudio

A área de rede expõe o status e as páginas do provider de rede instalado, incluindo rotas de Wi-Fi, Ethernet, VPN, DNS, proxy e firewall quando disponíveis. Bluetooth depende do suporte Bluetooth. A área de áudio aparece quando o recurso opcional correspondente está habilitado e integra-se ao serviço de áudio instalado.

### Energia e sessão

Energia oferece os controles de energia do sistema e as páginas de política disponíveis na instalação, incluindo a opção Manter acordado. Quando ativada, Manter acordado impede que a política de ociosidade do ARGVUS inicie os temporizadores automáticos de bloqueio da tela e desligamento do display. Sessão expõe status da sessão ARGVUS, componentes, autostart, diagnósticos e logs. Essas páginas podem exigir permissões do sistema e devem ser diferenciadas das ações rápidas do Control Panel.

### Ferramentas do sistema

A seção de sistema pode expor informações de boot, pacotes, serviços, armazenamento, diagnósticos, administração de usuários/grupos e informações do sistema. São ferramentas administrativas ou de diagnóstico; uma página pode ser somente leitura ou exigir autorização dependendo da operação.

### Usuários e grupos

Quando o provider de contas está disponível, abra **Configurações → Sistema → Usuários** ou busque por **usuários**. A interface pode listar contas normais e de sistema, criar usuários, editar metadados, gerenciar grupos suplementares e primários, alterar ou bloquear uma senha, desbloqueá-la, exigir troca de senha no próximo login e definir ou remover um avatar.

A criação pede username, nome completo, shell, grupos suplementares e uma senha opcional com confirmação. Uma senha vazia cria a conta com a senha bloqueada; não cria silenciosamente uma senha utilizável. Contas existentes oferecem ações destrutivas separadas para remover a conta mantendo o diretório home ou removendo também o home. Leia atentamente a confirmação antes de escolher a segunda opção. Operações de conta exigem privilégio e podem abrir o prompt de autorização do sistema. Avatares podem ser consumidos pelo greeter por meio da integração padrão de imagem da conta.

Os grupos possuem suas próprias páginas de lista e edição, incluindo gerenciamento de membros e exclusão de grupo. O Control Center não substitui as políticas de contas do sistema: contas protegidas e operações que exigem autorização continuam sujeitas às restrições do sistema operacional.

### Configuração do Control Center

A página **Configuração** atualmente controla os ícones decorativos do layout do Control Center. Ela altera a apresentação do aplicativo de configurações; não desativa serviços ARGVUS nem remove providers. O salvamento pode exigir autorização, enquanto a alteração visual é aplicada ao aplicativo atual.

## Busca e recursos opcionais

A tela inicial possui um registro pesquisável de rotas. Os termos incluem conceitos voltados ao usuário, como aparência, temas, wallpapers, fontes, atalhos de teclado, mouse, touchpad, displays, rede e sessão. Providers opcionais só aparecem quando o recurso foi compilado e sua capacidade de runtime foi detectada; por isso o menu visível é consciente das capacidades disponíveis.

Enquanto a busca da tela inicial estiver ativa, as teclas imprimíveis são inseridas na consulta, incluindo `j` e `k`; use as setas `↑` e `↓` para percorrer as rotas encontradas. Pressione `Enter` para abrir a rota selecionada, `Backspace` para editar a consulta e `Esc` para limpar a consulta ou sair do modo de busca.

## Fluxo de aparência

Abra **Aparência** para ver os controles visuais integrados. Suas páginas são:

* **Temas** — escolha uma família incluída e seu modo Sticky ou Float, ou gerencie perfis de tema personalizados.
* **Acentos** — escolha o acento ou informe uma cor RGB válida de seis dígitos.
* **Wallpapers** — escolha um wallpaper incluído ou selecione um arquivo personalizado.
* **Espaços, bordas e posição** — posição da taskbar, espaço da taskbar/shell, gaps das janelas, bordas e espessura das bordas.
* **Efeitos** — ative ou desative o estado compartilhado de efeitos visuais.
* **Widgets de telemetria** — ative a superfície de telemetria e selecione seus cards disponíveis.
* **Control Panel** — ative, desative e reordene os cards do painel.

Essas ações atualizam o estado lógico do ARGVUS e aplicam a configuração de runtime afetada. Veja [Aparência](/pt/docs/user-guide/appearance/), [Temas](/pt/docs/argvus-themes/) e [Janelas e layout](/pt/docs/argvus-hyprland/windows-and-layout/).

## Uso pelo teclado

O aplicativo de configurações suporta navegação pelo teclado. Use as setas ou `j`/`k` para mover, `Enter` para abrir ou aplicar, `Esc` para voltar, `/` para buscar listas, `Tab` para mover entre campos ou ações e `?` para ajuda contextual. Uma página pode mostrar a ação **Restaurar padrões** ou **Restaurar todos os atalhos** quando ela oferecer esse recurso.

## Persistência e restauração

As páginas aplicam alterações pelo provider responsável e salvam o estado de usuário suportado. As mudanças de aparência ficam na configuração canônica, e o `argvus-config` é o único componente que escreve os arquivos de consumo derivados em `~/.config/argvus/data/generated/`; esses arquivos gerados não são o local para uma edição permanente. As páginas de entrada e atalhos possuem estado persistente e ações de restauração próprias. Outras alterações do sistema podem exigir permissões ou recarga de serviço.

Não existe uma restauração global para todas as configurações do ARGVUS. Restaure uma alteração pela página responsável ou use o procedimento de recuperação documentado para o recurso correspondente.

O suporte à restauração é específico de cada domínio:

* **Aparência** oferece restauração da cor de destaque para o padrão do tema ativo. Não existe uma restauração global de aparência; restaure os demais valores pelos próprios controles.
* **Fontes** pode restaurar todas as fontes, um alvo ou uma configuração individual.
* **Aplicativos padrão** pode restaurar todos os padrões, uma categoria ou um seletor individual.
* **Atalhos de teclado** pode restaurar um atalho ou todos e então recarregar os atalhos gerados da sessão.
* **Outras páginas do sistema** exibem ações de restauração, aplicação, exclusão ou recuperação somente quando o provider responsável oferece esse recurso.

Remover uma preferência do usuário pode fazer um provider retornar ao estado padrão, mas apagar arquivos manualmente não é um procedimento geral de recuperação. Prefira a ação de restauração da página ou o comando documentado para o recurso.

## Entradas pela CLI

O binário instalado é `argvus-control-center`. Ele pode abrir áreas específicas diretamente, por exemplo:

```sh
argvus-control-center appearance themes
argvus-control-center appearance wallpapers
argvus-control-center input
argvus-control-center keybindings
```

Execute `argvus-control-center --help` para ver as rotas disponíveis na versão instalada. A rota pela linha de comando é um atalho para o mesmo aplicativo, não um segundo sistema de configuração.

Rotas diretas úteis incluem `apps`, `fonts`, `locale`, `input`, `keybindings`, `language`, `config`, `system`, `hardware`, `services`, `network`, `audio`, `bluetooth`, `boot`, `packages`, `storage`, `diagnostics`, `power`, `session`, `displays` e `appearance`. Appearance também aceita rotas específicas como `themes`, `wallpapers`, `accents`, `effects`, `spaces`, `taskbar` e `widget-telemetry`; displays aceita `resolution`, `refresh`, `scale`, `position`, `orientation`, `primary`, `vrr` e `hdr`. Use a saída `--help` instalada ao criar scripts, pois a disponibilidade ainda depende do build instalado.

## Relacionados

* [Primeira configuração](/pt/docs/getting-started/)
* [Control Panel](/pt/docs/argvus-control-panel/)
* [Aparência](/pt/docs/user-guide/appearance/)
* [Atalhos de teclado](/pt/docs/argvus-hyprland/keyboard-shortcuts/)
* [Mouse e touchpad](/pt/docs/argvus-hyprland/input/)
