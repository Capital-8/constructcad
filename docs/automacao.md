# Automação: motor, linha de comando e MCP

[English](automation.md)

Tudo o que o ConstructCAD faz num modelo pode ser feito sem a interface. O **motor** (`cc_engine`) reúne as operações num catálogo de **comandos** nomeados, e três clientes usam esse mesmo catálogo:

```
                     ┌──────────────────────────────┐
  aplicativo     ──▶│                              │
  ccad (CLI)     ──▶│  cc_engine: Session +        │──▶ o modelo (.ccad)
  agentes (MCP)  ──▶│  catálogo de comandos        │
                     └──────────────────────────────┘
```

Uma parede desenhada com o mouse, um `ccad run wall_create …` e um agente de IA chamando a ferramenta `wall_create` executam o mesmo código, com os mesmos padrões, verificações e desfazer. A lista completa de comandos e parâmetros está na [referência de comandos](commands.md) (em inglês, gerada do próprio catálogo).

## Conceitos

**Sessão.** Um projeto aberto: o documento (com o histórico de desfazer) e a geometria e os desenhos derivados dele. A CLI abre uma por execução (ou script), o servidor MCP uma por conexão, o aplicativo uma por janela.

**Comandos.** Cada comando tem um nome em `snake_case`, um objeto JSON de parâmetros descrito por um esquema JSON e um resultado JSON. Tipos:

| Tipo | Faz | Exemplos |
|---|---|---|
| query | lê o modelo | `elements_list`, `element_get`, `schedule_table` |
| edit | altera o modelo: um passo de desfazer | `wall_create`, `elements_move`, `element_set` |
| file | trabalha com arquivos ou substitui o projeto | `project_open`, `ifc_import`, `sheet_export` |

**Unidades e coordenadas.** Comprimentos em metros, ângulos em graus, pontos `[x, y]` em planta. As cotas usam as coordenadas do desenho da sua vista (X/Y da planta nas plantas; posição horizontal e altura nas elevações e cortes). Áreas e volumes nos resultados vêm em m² e m³.

**Referências.** Níveis, tipos, vistas e materiais são indicados por **id ou nome** (`"level": 3` ou `"level": "Térreo"`; maiúsculas e minúsculas não importam; um nome ambíguo dá erro pedindo o id). Folhas também podem ser indicadas pelo número (`"A101"`). Elementos são indicados por id, que `elements_list` e os resultados dos comandos de criação informam. Sem nível, vale o nível mais baixo; sem tipo, o tipo padrão da categoria.

**Ler e alterar parâmetros.** `element_get`, `type_get` e `view_get` mostram os `params` de um objeto; `element_set`, `type_set` e `view_set` recebem os campos a alterar no mesmo formato e os mesclam:

```sh
ccad -p casa.ccad run element_get id=42
ccad -p casa.ccad --save run element_set id=42 'params={"height": 2.7}'
ccad -p casa.ccad --save run element_set id=57 'params={"shape": {"Gable": {"slope": 0.5, "ridge_deg": 0}}}'
```

Objetos se mesclam campo a campo; listas e valores simples substituem; `null` limpa um valor opcional. `material_set` e `units_set` (no formato de `material_get` e `units_get`) funcionam do mesmo jeito.

**Validação e desfazer.** Cada edição é um passo de desfazer (`undo`, `redo`). Uma edição que deixaria o modelo inválido — uma porta numa parede inexistente, uma medida negativa, uma referência a nada — é recusada sem deixar rastro. Elementos fixados não são movidos, aparados nem excluídos.

**Erros.** Um comando que falha devolve `{"error": {"code": …, "message": …}}`, com a mensagem no idioma da sessão (`--lang`, `language_set`).

| Código | Significado |
|---|---|
| `unknown_command` | nenhum comando tem esse nome |
| `invalid_arguments` | os parâmetros não seguem o esquema ou não fazem sentido (comprimento zero, tipo desconhecido…) |
| `not_found` | o elemento, tipo, nível, vista ou material indicado não existe |
| `rejected` | a edição deixaria o modelo inválido ou não é permitida (fixado, em uso…) |
| `io` | falha ao ler ou gravar um arquivo |
| `internal` | um defeito; o projeto fica como estava |

## Linha de comando: `ccad`

O `ccad` é um programa separado, sem dependências gráficas (roda em servidores e na CI). Ele vem nos pacotes de release; a partir do código-fonte: `cargo build --release -p cc_cli` (o binário é `target/release/ccad`).

```text
ccad [opções] commands             lista os comandos
ccad [opções] schema <comando>     o esquema JSON dos parâmetros de um comando
ccad reference                     a referência de comandos em Markdown
ccad [opções] run <comando> [args] executa um comando
ccad [opções] script <arquivo|->   executa um script de comandos
ccad [opções] mcp                  serve os comandos por MCP (stdio)

-p, --project <arquivo>   abre um projeto antes (.ccad, ou .ifc para importar)
--save                    salva o projeto no fim (no próprio arquivo)
--save-as <arquivo>       salva o projeto no fim em <arquivo>
--lang <en|pt-BR>         idioma dos nomes padrão e mensagens (também CCAD_LANG)
```

Os argumentos de `run` podem ser um objeto JSON, `@arquivo.json`, `-` (entrada padrão) ou pares `chave=valor`, cujos valores são lidos como JSON quando possível e como texto caso contrário:

```sh
ccad -p casa.ccad --save run wall_create start=[0,0] end=[5,0] height=2.8
ccad -p casa.ccad run elements_list category=door
ccad -p casa.ccad --save run view_set '{"view": "Corte 1", "scale": 100}'
ccad -p modelo.ifc --save-as modelo.ccad run project_info          # importa um IFC
ccad -p casa.ccad run sheet_export sheet=A101 path=a101.pdf
```

Os resultados saem como JSON na saída padrão. Erros vão para a saída de erro como JSON, com código de saída 1; erros de uso saem com 2. Sem `--save` ou `--save-as`, o arquivo do projeto não muda.

Os resultados se combinam com outras ferramentas; por exemplo, com `jq`:

```sh
parede=$(ccad -p casa.ccad --save run wall_create start=[0,0] end=[6,0] | jq .id)
ccad -p casa.ccad --save run door_create wall=$parede offset=1.5
```

### Scripts

Um script é uma lista de passos: um array JSON de objetos `{"command": …, "args": {…}}` ou um desses objetos por linha (linhas que começam com `//` são comentários). `ccad script` executa os passos em ordem numa mesma sessão e para no primeiro erro; cada passo é um passo de desfazer.

[`examples/small_house.ccmacro`](../examples/small_house.ccmacro) constrói uma casa pequena:

```text
// A small house built with engine commands only.
{"command": "wall_create", "args": {"start": [0, 0], "end": [8, 0], "height": 2.8}}
…
{"command": "door_create", "args": {"point": [2, -0.5]}}
{"command": "floor_create", "args": {"from_walls": true}}
{"command": "room_create", "args": {"point": [2, 3], "name": "Living room"}}
```

```sh
ccad --save-as casa.ccad script examples/small_house.ccmacro
```

Scripts não têm variáveis: um passo não usa o id criado por um passo anterior. Os comandos ajudam aceitando posições onde isso importa (`door_create` e `window_create` encontram a parede mais próxima de `point`; pisos, forros e telhados acompanham as paredes com `from_walls`). Para lógica entre passos, chame `ccad run` a partir de um script de shell ou use um agente via MCP.

## Macros no aplicativo

*Gerenciar → Macros → Gravar macro* grava as alterações que você fizer até apertar *Parar gravação*; a macro é salva como script (`.ccmacro`, o formato acima). *Executar macro* roda uma macro ou qualquer script no projeto aberto; `ccad script` roda os mesmos arquivos.

- Só edições são gravadas: abrir, salvar e exportar ficam de fora, então reproduzir nunca sobrescreve arquivos.
- Um valor arrastado nas propriedades é gravado uma vez, com o valor final; alterações feitas num diálogo cancelado não são gravadas.
- Os passos gravados se referem a elementos pelo id. No projeto em que foi gravada (ou numa cópia dele como estava no início da gravação), a macro reproduz as mesmas alterações; em outro projeto, passos que indicam elementos pelo id podem falhar ou atingir outros elementos.

## Servidor MCP

`ccad mcp` é um servidor [Model Context Protocol](https://modelcontextprotocol.io/) na entrada/saída padrão. Cada comando é uma ferramenta com o mesmo nome, descrição e esquema de entrada; as consultas são marcadas como somente leitura. O servidor mantém uma sessão, então um agente abre ou cria um projeto, trabalha nele ao longo de várias chamadas e o salva.

**Claude Code:**

```sh
claude mcp add constructcad -- ccad mcp
# ou já com um projeto aberto
claude mcp add constructcad -- ccad mcp -p /caminho/para/casa.ccad
```

**Claude Desktop e outros clientes** (`claude_desktop_config.json` ou as configurações MCP do cliente; use o caminho completo do `ccad`):

```json
{
  "mcpServers": {
    "constructcad": { "command": "/caminho/para/ccad", "args": ["mcp", "--lang", "pt-BR"] }
  }
}
```

Exemplos de pedidos que um agente resolve com essas ferramentas: *"abra casa.ccad, liste as portas sem marca e numere-as"*, *"importe modelo.ifc e diga o volume das paredes de concreto"*, *"crie um pavimento superior 3 m acima do térreo com as mesmas paredes externas"*, *"exporte a folha A101 em PDF"*.

Observações:

- Os caminhos são da máquina onde o `ccad` roda.
- Erros voltam como resultado da ferramenta com `isError` e o objeto de erro, para que o modelo corrija a chamada.
- O servidor fala as revisões 2025-06-18, 2025-03-26 e 2024-11-05 do protocolo e é implementado em `crates/cc_cli/src/mcp.rs` (JSON-RPC 2.0, uma mensagem por linha).

## Para desenvolvedores: criar um comando

Toda funcionalidade de usuário nasce como comando do motor; o aplicativo chama o comando em vez de alterar o documento.

1. Escreva a estrutura de parâmetros em `crates/cc_engine/src/commands/<área>.rs`: `#[derive(Debug, Deserialize, Serialize, JsonSchema)]`, `#[serde(deny_unknown_fields)]`, um comentário de documentação (a descrição do comando) e um em cada campo (a descrição do parâmetro; um teste falha sem eles). O código e os comentários são em inglês.
2. Implemente `Command` com `NAME` (`snake_case`), `KIND` e `run`. Resolva referências com `refs`, altere o modelo dentro de `commands::edit` (um passo de desfazer, recusado se o resultado precisar de reparo) e devolva JSON (`created(s, id)` para elementos). Use mensagens `t!` para erros que pessoas vão ler.
3. Registre-o no seu grupo em `registry.rs`.
4. Teste-o em `crates/cc_engine/tests/commands.rs` por `Session::execute`.
5. Gere a referência de novo: `cargo run -p cc_cli -- reference > docs/commands.md` (um teste falha enquanto ela estiver desatualizada).
6. No aplicativo, chame-o com `Session::run(Comando { … })`, `actions::run_command` ou os auxiliares de `engine_edit` (use `Session::run_merged` para valores arrastados). Isso também o torna gravável em macros.

Os detalhes de arquitetura estão em [AGENTS.md](../AGENTS.md).
