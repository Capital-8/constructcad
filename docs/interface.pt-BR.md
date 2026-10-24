# A interface: ribbon, comandos e paleta

*English version: [interface.md](interface.md).*

A janela do ConstructCAD segue a disposição que usuários de CAD e BIM de desktop esperam. A **barra de acesso rápido** fica no canto superior esquerdo. Abaixo dela está o **ribbon**, uma fileira de abas com grupos de botões. Em seguida vem a **barra de opções** da ferramenta ativa. O **navegador de projeto** e as **propriedades** ficam encaixados à esquerda, as **vistas** ocupam as abas no centro e a **barra de status** fica embaixo.

## Comandos e suas dicas

Todo botão do ribbon e da barra de acesso rápido executa um *comando da interface*. O mesmo vale para cada atalho de duas letras e cada item da paleta de comandos. Cada comando define, num só lugar:

- seu **nome** e **ícone**;
- seu **atalho**: uma combinação com Ctrl ou duas letras digitadas com o foco numa vista;
- uma **descrição** do que faz;
- o **comando do motor** que faz a alteração. Scripts, a linha de comando `ccad` e o servidor MCP chamam esse mesmo comando (veja o [guia de automação](automacao.md));
- se ele **pode rodar agora** e, se não puder, por quê.

Passe o mouse sobre um botão para ver a dica:

```
Mover  [MV]
Move os elementos selecionados de um ponto base até um destino.
Indisponível: Selecione os elementos antes de usar a ferramenta
Comando do motor: elements_move
```

Botões que não podem rodar ficam acinzentados, e a dica explica o motivo. Motivos comuns são "nada para desfazer", "selecione os elementos primeiro", "abra uma folha para exportá-la", "selecione um texto" e "copie elementos primeiro". Um atalho digitado quando o comando não pode rodar mostra o mesmo motivo na barra de status.

## Ícones e selos

Os ícones têm dois tons: os traços na cor do texto e os preenchimentos suaves no tom de destaque do tema. Um pequeno selo no canto inferior direito indica o tipo de ação do comando:

| Selo | Significado | Exemplos |
|---|---|---|
| **+** verde | cria algo novo | nova planta de piso ou de forro, vista 3D, legenda, tabela, folha, novo componente |
| seta azul para baixo | traz dados de um arquivo | Importar IFC, Carregar componente |
| seta azul para cima | grava dados num arquivo | Exportar PDF, Exportar SVG |
| **−** vermelho | remove algo | Excluir |

A ferramenta selecionada aparece realçada na cor de destaque. Comandos indisponíveis ficam acinzentados, inclusive o selo.

## Recolher o ribbon

Para dar mais espaço às vistas, recolha o ribbon até os títulos das abas. Qualquer uma destas ações recolhe o ribbon ou o expande de novo:

- **Ctrl+F1**;
- um **clique duplo** em qualquer aba;
- a **seta** no fim da fileira de abas, à direita.

Com o ribbon recolhido, um clique numa aba mostra o painel dela por cima das vistas. O painel fecha quando um comando é executado, quando você aperta Esc ou clica em outro lugar, ou quando clica de novo na mesma aba. O ConstructCAD lembra entre as sessões se o ribbon estava recolhido.

## Janelas estreitas

Quando a janela é mais estreita que os grupos de uma aba, os grupos rolam de lado em vez de serem cortados. Aparecem setas nas duas pontas da fileira: clique nelas para rolar, ou use a roda do mouse. Cada seta fica acinzentada ao chegar à sua ponta.

## Paleta de comandos

**Ctrl+Shift+P** abre a paleta de comandos, assim como a lupa no fim da fileira de abas, à direita. Digite parte do que procura. A busca ignora maiúsculas e acentos ("niveis" encontra "Níveis"), e todas as palavras digitadas precisam aparecer. A paleta encontra:

- **comandos da interface**, pelo nome, pela descrição ou pelo atalho (digitar `WA` encontra *Parede*);
- **plantas a criar** para cada nível ("Planta de piso: Térreo") e **tabelas** para cada categoria;
- **vistas a abrir** ("Abrir vista: …");
- **comandos do motor** (`level_create`, `elements_list`, `type_set`…), em fonte monoespaçada.

Use ↑ e ↓ para escolher e Enter para executar, ou clique num item. Esc fecha a paleta. Um comando indisponível continua na lista, acinzentado, com o motivo no lugar da descrição.

### Executar comandos do motor

Escolher um comando do motor abre um formulário com sua descrição e seus argumentos em JSON, já com os campos obrigatórios:

```json
{
  "elevation": 9.0,
  "name": "Cobertura"
}
```

**Executar** (ou Ctrl+Enter) roda o comando no projeto aberto e mostra o resultado ou a mensagem de erro. Uma alteração é um passo de desfazer e é gravada enquanto uma macro está gravando, como qualquer outra. **Voltar** (ou Esc) retorna à busca. Os argumentos são os mesmos dos scripts e da [referência de comandos](commands.md). Comprimentos estão em metros, ângulos em graus, pontos são `[x, y]`, e níveis, tipos, vistas e materiais podem ser indicados pelo id ou pelo nome.

Os comandos de arquivo do motor (`project_open`, `project_save`, `ifc_import`, `ifc_export`, `sheet_export`…) não aparecem aqui. Os comandos da interface correspondentes, na aba *Arquivo* e na paleta, cuidam dos arquivos pelos diálogos de sempre.

## Teclado

| Tecla | Ação |
|---|---|
| Ctrl+Shift+P | Paleta de comandos |
| Ctrl+F1 | Recolher / expandir o ribbon |
| Ctrl+N / Ctrl+O / Ctrl+S / Ctrl+Shift+S / Ctrl+W | Novo / Abrir / Salvar / Salvar como / Fechar projeto |
| Ctrl+Z / Ctrl+Y | Desfazer / Refazer |
| Ctrl+X / Ctrl+C / Ctrl+V | Recortar / Copiar / Colar |
| Delete | Excluir a seleção |
| Esc | Cancelar a operação, sair da ferramenta, limpar a seleção |
| Espaço | Girar ou inverter a seleção ou o elemento sendo posicionado |

As teclas de duas letras funcionam com o foco numa vista e nenhum campo de texto em edição. O README lista todas elas, e cada dica mostra também o seu atalho.

## Gráficos das vistas

*Vista → Gráficos* reúne **Visibilidade/gráficos** (VG), **Filtros** e **Modelos de vista** (também em *Gerenciar*).

- **Visibilidade/gráficos** edita a vista ativa (ou, a partir do gerenciador de modelos, um modelo). As abas *Categorias de modelo* e *Categorias de anotação* listam cada categoria com uma caixa de visibilidade e células para linhas em projeção, preenchimento de superfície, linhas de corte, preenchimento de corte, meio-tom e transparência; a célula mostra a sobreposição (— quando não há) e abre um editor de cor, espessura (mm de papel) e padrão. A aba *Filtros* lista os filtros aplicados à vista em ordem de prioridade (↑ ↓), com caixas de ativo e visível e as mesmas células; *Adicionar filtro…* aplica outro e *Editar filtros…* abre o editor de filtros. Quando o modelo da vista controla categorias ou filtros, a aba avisa e fica só para leitura. OK mantém as mudanças; Cancelar as desfaz.
- **Filtros** edita os filtros do projeto: nome, categorias (nenhuma: todas) e regras — parâmetro, operador e valor — combinadas com E ou OU, com o número de elementos que correspondem.
- **Modelos de vista** lista os modelos; *Criar a partir da vista* cria um da vista ativa. Cada propriedade tem a caixa *Incluir*; *Aplicar propriedades* copia o modelo para a vista ativa uma vez, *Atribuir à vista* faz a vista segui-lo. As propriedades da vista mostram seu modelo (*Nenhum* a libera) e um botão *Visibilidade/gráficos*; as propriedades controladas pelo modelo ficam só para leitura.
- Com elementos selecionados, **Sobrepor gráficos** muda seus gráficos só na vista ativa e **Ocultar na vista** (EH) os oculta nela (reexibidos em Visibilidade/gráficos).
- **Ocultar/isolar** (temporário): HH oculta a seleção, HI a isola, HC e IC ocultam ou isolam suas categorias, HR redefine. Enquanto ativo, a vista ganha uma borda colorida e um aviso com *Redefinir* e *Tornar permanente*. A ocultação temporária não é salva, não entra no desfazer e não é impressa.

## Fases de construção

- **Gerenciar → Fases** tem três abas. *Fases* lista as fases do projeto da mais antiga para a mais recente, com nome, descrição e quantos elementos são construídos e demolidos em cada uma; *Adicionar* acrescenta uma no fim, *Inserir antes* põe uma antes da selecionada, ↑ ↓ reordenam (uma mudança que poria uma demolição antes da construção é recusada), e excluir uma fase passa seus elementos e vistas para a fase escolhida em *Mesclar elementos e vistas em*. *Filtros de fase* define, para cada filtro, como aparecem os elementos existentes, demolidos, novos e temporários: *Não exibido*, *Por categoria* ou *Substituído*. *Gráficos de fase* define as sobreposições de cada estado (as mesmas células de Visibilidade/gráficos). OK mantém as mudanças; Cancelar as desfaz.
- As propriedades da vista (plantas, elevações, cortes, vistas 3D, tabelas) têm a seção *Fases* com sua *Fase* e *Filtro de fase*; o filtro fica só para leitura quando o modelo da vista o controla. Os elementos mostram *Fase de criação* e *Fase de demolição*. Elementos novos são construídos na fase da vista ativa.
- **Modificar → Demolir** (DE) demole os elementos clicados na fase da planta, elevação ou corte ativo; suas portas e janelas vão junto.

## Aviso de atualização

O ConstructCAD avisa quando uma versão mais nova é publicada; ele nunca baixa nem instala nada por conta própria.

- **Quando**: ao iniciar, no máximo uma vez por dia, em segundo plano (a janela nunca espera por isso). *Arquivo → Verificar atualizações* (também no diálogo *Sobre*) verifica na hora e informa "está atualizado", a nova versão ou que não foi possível verificar.
- **O que é contatado**: uma requisição HTTPS à API do GitHub, `https://api.github.com/repos/francisreboucas/constructcad/releases/latest`, que devolve a última versão publicada. A requisição envia os dados usuais de conexão (seu endereço IP) e o agente `ConstructCAD/<versão>`; nenhum dado de projeto, identificador ou informação de uso. Vale para ela a declaração de privacidade do GitHub.
- **O que aparece**: quando a versão publicada é mais nova que a em uso (pré-lançamentos são ignorados), um aviso aparece na barra de status e na tela inicial: *Baixar* abre a página da versão no navegador, *Ignorar esta versão* esconde o aviso até sair uma versão posterior, e × o esconde até a próxima verificação. Qualquer falha (sem conexão, tempo esgotado, proxy, limite de requisições) é silenciosa.
- **Como desligar**: desmarque *Verificar atualizações automaticamente* em *Arquivo → Sobre*. A variável de ambiente `CONSTRUCTCAD_NO_UPDATE_CHECK=1` desliga toda verificação, automática ou pedida (útil em CI, quiosques e instalações sem rede). O ConstructCAD guarda a hora da última verificação, a versão ignorada e a opção junto com as outras preferências.

## Acessibilidade

Os botões do ribbon, os títulos das abas, as setas de rolagem, os itens da paleta, as linhas do navegador, os cartões da tela inicial, os botões de fechar dos diálogos, as seções de propriedades e o botão de início do cubo de vista são desenhados pelo próprio ConstructCAD. Eles informam à interface de acessibilidade do sistema seu papel (botão, item selecionável, cabeçalho recolhível), seu nome e seu estado (habilitado, selecionado, expandido), para que leitores de tela possam anunciá-los.

## Para desenvolvedores

### Adicionar um comando à interface

1. Primeiro, faça da alteração um comando do motor (veja *Para desenvolvedores* no [guia de automação](automacao.md)).
2. Acrescente uma variante a `UiCommand` em `crates/cc_app/src/commands.rs`. Depois preencha `icon`, `label`, `description` (uma chave i18n `cmd-desc-…` em todos os arquivos `.ftl`), `shortcut` (se houver), `engine` (o nome do comando do motor), `badge`, `unavailable` (um motivo traduzido para cada estado em que não pode rodar), `selected` (para ferramentas) e `run`. Inclua-o em `UiCommand::all`.
3. Coloque-o num grupo do ribbon com `command_button` (grande) ou numa família de botões pequenos com `stacks` (`crates/cc_app/src/ribbon.rs`).

A dica, a habilitação, o item da paleta e um atalho de duas letras saem todos do comando. Os testes verificam que cada nome do motor existe no catálogo, que os atalhos não se repetem e que todo comando aparece no ribbon com nome acessível.

### Ícones

Os ícones são arquivos SVG em `assets/icons/ui`, numa grade de 24 unidades. Eles usam traços brancos (`stroke-width` 1.75, pontas e junções arredondadas) e preenchimentos suaves escritos como `fill="#fff" fill-opacity=".2"`. O `icons.rs` separa cada ícone em duas camadas por esses atributos: os traços e as marcas sólidas formam a tinta, e os preenchimentos suaves formam o tom. Cada camada é rasterizada pelo resvg no tamanho exato em pixels e pintada nas cores do tema (`icons::Tones`). Acrescente o novo ícone a `Icon`, `Icon::ALL` e `Icon::source`. `ICON_SHEET=folha.png cargo test -p cc_app icon_sheet` grava todos os ícones em vários tamanhos para revisão.

### Tema

As cores são tokens de `theme::Palette`: `PALETTE` (claro) e `DARK`, com um teste de contraste. Pinte com `theme::palette(ctx)` para que um tema escuro possa ser ligado depois.

### Testes de interface

O `crates/cc_app/src/ui_tests.rs` roda o ribbon e a paleta sem janela, com o [egui_kittest](https://crates.io/crates/egui_kittest). Os testes clicam e digitam pela árvore de acessibilidade, então widgets próprios precisam chamar `Response::widget_info`. As capturas de referência ficam em `crates/cc_app/tests/snapshots`. Elas cobrem cada aba do ribbon, o ribbon estreito e o recolhido, uma dica completa, a paleta e uma galeria de ícones nos dois temas. Placas de vídeo renderizam com pequenas diferenças, então a comparação das capturas roda sob demanda:

```sh
CC_UI_SNAPSHOTS=1 cargo test -p cc_app snapshots     # compara com as referências
UPDATE_SNAPSHOTS=1 cargo test -p cc_app snapshots    # regrava após uma mudança intencional
```

Revise as imagens novas antes do commit. Quando há diferença, o kittest grava `*.new.png` e `*.diff.png` ao lado das referências; o Git ignora esses arquivos.
