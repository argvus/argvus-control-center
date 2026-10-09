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

* **Aparência** — temas, modos, acentos e wallpapers.
* **Taskbar** — posição, espaços, ícones, data e hora, reaproveitando as páginas de efeitos implementadas na Aparência.
* **Control Panel** — sessões, ordem e transparência dos cards, reaproveitando as páginas de efeitos implementadas na Aparência.
* **Widget Telemetry** — sessões e transparência do widget de telemetria, reaproveitando as páginas de efeitos implementadas na Aparência.
* **Aplicativos padrão** — selecione aplicativos instalados para funções compatíveis.
* **Fontes** — escolha os alvos de fonte e as configurações de renderização expostas pelo ARGVUS.
* **Localidade e região** — fuso horário, data e hora, localidade regional, locales do sistema e configuração do teclado.
* **Sistema** — host, usuários, grupos e páginas de administração disponíveis na instalação atual.
* **Hyprland** — atalhos de teclado, regras de janela e as páginas de efeitos de janela reaproveitadas da Aparência (espaços de janela, animações, blur, bordas), além da versão instalada do Hyprland.
* **Displays, rede, sessão e outros domínios** — disponíveis pela busca inicial e pelas rotas de domínio quando os recursos opcionais correspondentes estão instalados.

As páginas exatas dos providers podem depender dos pacotes instalados e das permissões. A ausência de um domínio opcional não significa necessariamente que o Control Center esteja quebrado.

Quando a grade tem mais itens do que cabem na tela, uma barra de rolagem aparece na borda direita para indicar que há mais conteúdo abaixo.

## Encontrando uma configuração

A tela inicial registra configurações por título, categoria, identificador e palavras-chave. A busca ignora acentos e aceita termos práticos como `theme`, `wallpaper`, `shortcut`, `mouse`, `monitor`, `network`, `user` e `DPI`. Correspondências exatas de título aparecem primeiro, seguidas por prefixo do título, por qualquer palavra do título ou categoria que contenha o texto digitado e, por fim, por palavras-chave — ou seja, digitar qualquer trecho de uma palavra (não só o início) ainda encontra o destino.

O menu visível reconhece as capacidades disponíveis. Uma rota pode não aparecer quando seu provider não está instalado, quando o hardware não está disponível ou quando a sessão atual não oferece a capacidade necessária. Por isso, a busca também é uma forma de descobrir o que esta instalação pode configurar.

A árvore atual de configurações inclui:

| Área | Destinos atuais |
| --- | --- |
| Aparência | Temas, modos de tema, cor de destaque, wallpapers, efeitos, espaços/bordas/posição |
| Taskbar | Posição, espaçamento, grupo utilitário, ícones, data e hora |
| Control Panel | Visibilidade/ordem dos cards, sessões e transparência |
| Widget Telemetry | Sessões e transparência |
| Entrada e teclado | Mouse e touchpad, layout/variante do teclado, mapa do console |
| Localidade e região | Idioma, fuso horário, data e hora, localidade regional, locales do sistema e teclado |
| Aplicativos | Aplicativos padrão e seletores por categoria, Projetos |
| Hyprland | Atalhos de teclado, Regras de janela e as páginas de efeitos de janela reaproveitadas da Aparência (Espaços de janela, Animações, Blur, Bordas), além da versão instalada do Hyprland |
| Sistema | Hostname, firewall, usuários, grupos e administração do sistema |
| Hardware | Resumo, CPU, GPU, memória, energia e dispositivos |
| Serviços e diagnósticos | Serviços, boot, pacotes, armazenamento e diagnósticos |
| Conectividade | Rede, áudio e Bluetooth |
| Sessão | Componentes, autostart, diagnósticos e logs |
| Displays | Resolução, taxa de atualização, escala, posição, orientação, display principal, VRR e HDR quando suportados |

Alguns itens são páginas completas e outros são subpáginas alcançadas pela busca ou por um resumo de domínio. A lista exata depende dos providers ARGVUS instalados.

## Para que serve cada área

### Aparência

Aparência é a principal área de personalização. Ela inclui temas e modos Sticky/Float, cores de destaque e wallpapers incluídos ou personalizados. Taskbar, Control Panel e Widget Telemetry são áreas próprias no nível principal (veja abaixo) que reaproveitam as páginas de efeitos compartilhadas da Aparência (transparência, blur) para suas próprias superfícies. Esses controles coordenam os componentes de aparência e sessão, em vez de alterar apenas a janela do Control Center.

### Taskbar

A Taskbar configura a posição do painel, o espaçamento ao redor dele e das janelas, o grupo de ícones utilitários e os blocos de data/hora, além da própria transparência e blur.

### Control Panel

O Control Panel configura a visibilidade e a ordem dos seus cards de ação rápida, além das próprias sessões, transparência e blur.

### Widget Telemetry

O Widget Telemetry configura as sessões, a transparência e o blur do widget de informações do sistema.

### Fontes

Fontes permite selecionar família e tamanho para taskbar, telemetria/informações do sistema, Control Panel, interface do sistema ARGVUS, aplicativos, terminal e navegador. Também expõe configurações de renderização como antialiasing, hinting, modo subpixel e DPI. A família principal padrão é IBM Plex Mono; cada alvo pode ser alterado independentemente e restaurado pela página Fontes.

### Aplicativos padrão

Aplicativos padrão seleciona programas instalados para funções como terminal, gerenciador de arquivos, editor de texto, editor do terminal, navegador, visualizador de imagens, visualizador de PDF, reprodutor de vídeo, reprodutor de áudio, ferramenta de arquivos compactados e launcher. A seleção é salva pelo ARGVUS e pode atualizar associações XDG padrão.

### Localidade e região

Esta área contém fuso horário, data e hora, localidade regional, locales do sistema e configurações do teclado. As configurações do teclado incluem layout, variante e mapa de teclado do console. A busca inicial também expõe atalhos de teclado e mouse/touchpad como rotas diretas de configuração.

### Hardware e displays

Quando os recursos correspondentes estão instalados, o Control Center pode mostrar informações de CPU, GPU, memória, energia e dispositivos. A página de entrada configura o comportamento do mouse e touchpad; a área de displays lida com resolução, taxa de atualização, escala, posição, orientação, display principal, VRR e HDR quando suportados pela sessão ativa. As mudanças de display valem na hora. Depois de uma mudança que pode deixar a tela inutilizável (resolução ou taxa, posição, espelhamento, desativar um monitor, profundidade de cor) ou depois de **Aplicar**, o Control Center pergunta se deve mantê-la, com o foco em **Reverter**: pressione `y` para manter; `n`, `Esc` ou `Enter` revertem, e sem resposta em 15 segundos a configuração anterior volta automaticamente. Remover a configuração salva de um monitor desconectado e excluir um perfil pedem confirmação; os perfis abrem uma página própria com **Aplicar perfil**, **Renomear** e **Excluir perfil**.

### Conectividade e áudio

A área de rede expõe o status e as páginas do provider de rede instalado, incluindo rotas de Wi-Fi, Ethernet, VPN, DNS, proxy e firewall quando disponíveis. Bluetooth depende do suporte Bluetooth. A área de áudio aparece quando o recurso opcional correspondente está habilitado e integra-se ao serviço de áudio instalado.

### Energia e sessão

Energia oferece os controles de energia do sistema e as páginas de política disponíveis na instalação, incluindo a opção Manter acordado. Quando ativada, Manter acordado impede que a política de ociosidade do ARGVUS inicie os temporizadores automáticos de bloqueio da tela e desligamento do display. Sessão expõe status da sessão ARGVUS, componentes, autostart, diagnósticos e logs. Essas páginas podem exigir permissões do sistema e devem ser diferenciadas das ações rápidas do Control Panel.

### Ferramentas do sistema

A seção de sistema pode expor informações de boot, pacotes, serviços, armazenamento, diagnósticos, administração de usuários/grupos e informações do sistema. São ferramentas administrativas ou de diagnóstico; uma página pode ser somente leitura ou exigir autorização dependendo da operação.

Nas listas de **Pacotes** que filtram ao digitar (Buscar, AUR, Instalados, Órfãos e Atualizações), as letras vão para o filtro; use as setas para navegar e `/` para digitar uma nova busca. `Enter` num pacote abre a página dele, com **Instalar** (ou **Reinstalar**) e, na **Zona de perigo**, **Remover**. Em **Órfãos**, `Space` marca pacotes (`[x]`) e **Remover marcados** remove todos juntos. **Atualizar tudo** e **Atualizar banco** são linhas da página Atualizações, as limpezas de cache ficam na Zona de perigo da página Cache, e **Mirrors → Configurar mirrors** define as opções do reflector antes de **Gerar preview**. Toda operação de pacotes mostra o plano e pede confirmação (`y` confirma, `n` ou `Esc` cancela) antes de rodar, e a saída aparece enquanto ela roda.

### Usuários e grupos

Quando o provider de contas está disponível, abra **Configurações → Sistema → Usuários** ou busque por **usuários**. A interface pode listar contas normais e de sistema, criar usuários, editar metadados, gerenciar grupos suplementares e primários, alterar ou bloquear uma senha, desbloqueá-la, exigir troca de senha no próximo login, definir ou remover um avatar, e ativar o login automático.

A criação pede username, nome completo, shell, grupos suplementares e uma senha opcional com confirmação. Uma senha vazia cria a conta com a senha bloqueada; não cria silenciosamente uma senha utilizável. A página de um usuário agrupa os campos da conta (nome completo, shell, grupo principal e grupos suplementares, seguidos de **Salvar alterações**), o alternador de **Login automático**, as ações de **Senha** (alterar, bloquear, desbloquear, exigir troca no próximo login) e as de **Avatar**. Username, UID / GID e diretório home aparecem só como informação. A **Zona de perigo** no fim reúne as ações destrutivas separadas para remover a conta mantendo o diretório home ou removendo também o home. Leia atentamente a confirmação antes de escolher a segunda opção. Operações de conta exigem privilégio e podem abrir o prompt de autorização do sistema. Avatares podem ser consumidos pelo greeter por meio da integração padrão de imagem da conta.

O **Login automático** inicia a sessão da conta selecionada no próximo boot sem mostrar a tela de login, a mesma convenção usada por GNOME, LightDM e SDDM para esse recurso: nenhuma senha é armazenada em lugar nenhum. Ativá-lo para uma conta desativa automaticamente qualquer conta configurada anteriormente (apenas uma conta pode ter login automático por vez). Ele vale apenas para o próximo boot; depois de um logout manual, a tela de login normal volta a aparecer pelo resto daquela sessão. O alternador é aplicado imediatamente, com uma confirmação, e está disponível apenas para administradores.

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
* **Widgets de telemetria** — ative a superfície de telemetria, selecione seus blocos disponíveis e reordene-os (`Shift+↑`/`Shift+↓` com um bloco focado).
* **Control Panel** — ative, desative e reordene os cards do painel (`Shift+↑`/`Shift+↓` com um card focado, aplicado na hora através do mesmo mecanismo que o arrastar do painel real usa).

Essas ações atualizam o estado lógico do ARGVUS e aplicam a configuração de runtime afetada. Veja [Aparência](/pt/docs/user-guide/appearance/), [Temas](/pt/docs/argvus-themes/) e [Janelas e layout](/pt/docs/argvus-hyprland/windows-and-layout/).

## Uso pelo teclado

O aplicativo de configurações suporta navegação pelo teclado. Cada página é uma lista única: use as setas ou `j`/`k` para mover, `Enter` para abrir ou executar a linha selecionada, `Space` para alternar, `Esc` para voltar, `/` para buscar listas e `?` para ajuda; o rodapé mostra as teclas que valem para a linha selecionada. As ações são linhas da lista, não botões, e `Tab` não leva até elas. Linhas que só mostram informação são puladas. Enquanto um campo de texto, uma busca ou um filtro de lista estiver ativo, `q` e `?` são digitados como texto em vez de sair ou abrir a ajuda. **Restaurar padrões** e **Restaurar todos os atalhos** são linhas de uma **Zona de perigo** no fim das suas páginas; `r` pede a mesma restauração, e na lista de atalhos `r` restaura o atalho selecionado depois de uma confirmação. Em **Teclado → Layout**, `Enter` define o layout padrão e `Space` adiciona ou remove um layout. Em **Locales do sistema**, `Enter` ou `Space` marcam um locale e a linha **Aplicar** no fim grava a seleção.

As páginas que juntam alterações antes de salvar (um usuário, um grupo, a criação de conta, a troca de senha, a configuração do firewall e Locales do sistema) terminam esse bloco com a linha **Salvar** ou **Aplicar**, que fica apagada até haver alguma alteração. Se voltar descartaria alterações não salvas, o Control Center pergunta antes. Toda confirmação mostra **Confirmar** e **Cancelar** como duas linhas, com o foco em **Cancelar**: `Enter` executa a linha em foco, `y` confirma e `n` ou `Esc` cancelam.

Em **Aparência**, cada página é uma lista única: o rodapé mostra só as teclas que valem para a linha selecionada, e `Space` também abre ou seleciona a linha. As páginas que juntam alterações antes de aplicar (Taskbar, Widget Telemetry, Control Panel e os controles de efeito) terminam com a linha **Aplicar**, que fica apagada até haver alguma alteração. Nas linhas de porcentagem, `←`/`→` (ou `+`/`-`) mudam o valor de 5 em 5 e `Enter` permite digitá-lo; use `Esc` para voltar. Se voltar descartaria alterações não aplicadas, o Control Center pergunta antes. Excluir um tema personalizado (`d`) ou substituir um tema importado pede confirmação; o foco começa em **Cancelar**, `y` confirma e `n` ou `Esc` cancelam.

## Persistência e restauração

As páginas aplicam alterações pelo provider responsável e salvam o estado de usuário suportado. As mudanças de aparência ficam na configuração canônica, e o `argvus-config` é o único componente que escreve os arquivos de consumo derivados em `~/.config/argvus/data/generated/`; esses arquivos gerados não são o local para uma edição permanente. As páginas de entrada e atalhos possuem estado persistente e ações de restauração próprias. Outras alterações do sistema podem exigir permissões ou recarga de serviço.

Não existe uma restauração global para todas as configurações do ARGVUS. Restaure uma alteração pela página responsável ou use o procedimento de recuperação documentado para o recurso correspondente.

O suporte à restauração é específico de cada domínio:

* **Aparência** oferece restauração da cor de destaque para o padrão do tema ativo. Não existe uma restauração global de aparência; restaure os demais valores pelos próprios controles.
* **Fontes** pode restaurar todas as fontes, um alvo ou uma configuração individual.
* **Aplicativos padrão** pode restaurar todos os padrões, uma categoria ou um seletor individual.
* **Atalhos de teclado** pode restaurar um atalho ou todos e então recarregar os atalhos gerados da sessão.
* **Regras de janela** ficam em **Hyprland → Configurações → Regras de janela** (busque por `regras de janela` ou execute `argvus-control-center window-rules`). Cada regra tem a linha **Workspace** (`Enter` ou `←/→` percorrem de 1 a 10) e a linha **Classes de janela**, que abre um campo de texto com expressões regulares separadas por vírgula. **Adicionar regra** cria `rule-N` no workspace 1, sem classes. **Remover regra** de cada regra fica na **Zona de perigo** e pede confirmação. Cada alteração é gravada em `hyprland.window_rules` no `argvus-config` e recarrega a configuração na hora; padrões de classe inválidos são recusados e mostrados como erro.

* **Projetos** ficam em **Aplicativos → Projetos** (busque por `projetos` ou execute `argvus-control-center projects`). **Adicionar pasta de projeto** e **Adicionar pasta raiz** ficam no topo da página e abrem um campo de texto com o caminho; o launcher confere se é um diretório e mostra o erro caso não seja, inclusive quando a pasta levaria o total efetivo de projetos além de 9 (o `SUPER + ALT + 1..9` não consegue endereçar mais que isso). Cada pasta configurada, marcada como raiz (seus subdiretórios são projetos) ou projeto único, ganha seu próprio bloco abaixo do formulário com uma linha **Remover** que pede confirmação. A página chama o `argvus-projects`, então as alterações são gravadas pelo launcher e valem na próxima vez que `SUPER + O` ou `SUPER + ALT + 1..9` forem usados. Se o `argvus-projects` não estiver instalado, a página informa isso.
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

Rotas diretas úteis incluem `apps`, `window-rules`, `projects`, `fonts`, `locale`, `input`, `keybindings`, `language`, `config`, `system`, `hardware`, `services`, `network`, `audio`, `bluetooth`, `boot`, `packages`, `storage`, `diagnostics`, `power`, `session`, `displays` e `appearance`. Appearance também aceita rotas específicas como `themes`, `wallpapers`, `accents`, `effects`, `spaces`, `taskbar` e `widget-telemetry`; displays aceita `resolution`, `refresh`, `scale`, `position`, `orientation`, `primary`, `vrr` e `hdr`. Use a saída `--help` instalada ao criar scripts, pois a disponibilidade ainda depende do build instalado.

## Relacionados

* [Primeira configuração](/pt/docs/getting-started/)
* [Control Panel](/pt/docs/argvus-control-panel/)
* [Aparência](/pt/docs/user-guide/appearance/)
* [Atalhos de teclado](/pt/docs/argvus-hyprland/keyboard-shortcuts/)
* [Mouse e touchpad](/pt/docs/argvus-hyprland/input/)
