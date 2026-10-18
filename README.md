# ConstructCAD

Software livre de **Modelagem de Informações da Construção (BIM)**, inspirado no Revit, escrito em Rust.

Um único modelo paramétrico é a fonte da verdade. Plantas, forros, cortes, elevações, vistas 3D, legendas, tabelas e folhas são *consultas* sobre esse modelo e se atualizam automaticamente a cada alteração.

## Executar

```sh
cargo run --release -p cc_app                                    # projeto novo
cargo run --release -p cc_app -- examples/casa_simples.ccad      # abrir um projeto
cargo run --release -p cc_app -- examples/casa_simples.ccad --view "Elevação: Sul"
cargo run --release -p cc_app -- --lang en                       # interface em inglês
cargo run -p cc_core --example casa_simples                      # regenerar o exemplo
```

## Idiomas

A interface está disponível em **português (Brasil)** e **inglês**, com troca em *Arquivo → Idioma*. A escolha fica salva entre sessões. Na primeira execução vale o idioma do sistema, e `--lang pt-BR|en` força um idioma.

Projetos novos recebem os nomes de níveis, materiais, tipos e vistas no idioma ativo. Projetos existentes mantêm seus nomes, pois eles fazem parte dos dados. O separador decimal acompanha o idioma (vírgula ou ponto).

As traduções ficam em `assets/i18n/*.ftl`. Um teste garante que todos os idiomas têm as mesmas chaves e os mesmos placeholders. Para adicionar um idioma, crie o `.ftl` e registre-o em `Language` (`crates/cc_app/src/i18n.rs`).

## Funcionalidades (MVP)

- **Navegador de projeto**: Plantas de piso, Plantas de forro, Vistas 3D, Elevações (N/S/L/O), Cortes, Legendas, Tabelas e Folhas. Pelo menu de contexto é possível abrir, renomear, duplicar, excluir e inserir na folha ativa.
- **Modelagem em planta**:
  - Paredes em cadeia, com junções automáticas em L (meia-esquadria) e T.
  - Portas e janelas hospedadas na parede, com abertura recortada automaticamente.
  - Pisos, forros e telhados (plano ou de uma água, com beiral) por esboço ou "Das paredes".
- **Snapping**: extremidades, pontos médios, sobre a parede, ângulos de 15°, Shift para ortogonal, comprimento digitado + Enter.
- **Edição**: seleção por clique ou janela (contida ou cruzada), mover arrastando, copiar com Ctrl+arrastar, excluir. Desfazer e refazer cobrem todas as edições.
- **Propriedades**: parâmetros de instância e da vista, troca de tipo e editor de tipos (camadas, materiais, dimensões, duplicar).
- **Vista 3D** (wgpu, MSAA): orbitar, pan, zoom, ajustar ao modelo e seleção por clique.
- **Elevações e cortes**: remoção de linhas ocultas, hachura de corte e níveis anotados.
- **Tabelas**:
  - Portas e janelas.
  - Paredes, com comprimento, área e volume.
  - Pisos, forros e telhados, com área, perímetro e volume.
- **Folhas** A0–A4: carimbo com dados do projeto e viewports arrastáveis em escala.
- **Gerenciar**: informações do projeto, níveis (cada nível novo cria suas plantas) e materiais.
- **Arquivo `.ccad`**: RON versionado e legível; o salvamento é atômico.

## Atalhos

| Tecla | Ação | Tecla | Ação |
|---|---|---|---|
| WA | Parede | SB | Piso |
| DR | Porta | FO | Forro |
| WN | Janela | TE | Telhado |
| CT | Corte | MD | Selecionar |
| ZE | Ajustar zoom | Esc | Cancelar / sair da ferramenta |
| Ctrl+Z / Ctrl+Y | Desfazer / Refazer | Delete | Excluir |
| Ctrl+S / Ctrl+Shift+S | Salvar / Salvar como | Espaço | Inverter porta |

Navegação 2D: roda do mouse dá zoom e o botão do meio faz pan. Navegação 3D: arrastar com o botão esquerdo orbita, botão direito ou do meio faz pan e a roda dá zoom.

## Arquitetura

```
crates/
  cc_core    modelo BIM (elementos, tipos, níveis, vistas), transações undo/redo, persistência .ccad
  cc_geom    geometria derivada: paredes com junções e aberturas, lajes, telhados, cortes, picking
  cc_views   geração de desenhos 2D (planta, forro, elevação, corte, axonometria, legenda) e tabelas
  cc_render  renderizador 3D offscreen em wgpu
  cc_app     aplicativo eframe/egui: ribbon, navegador, propriedades, ferramentas, folhas
assets/i18n  traduções (pt-BR, en); textos de interface nunca ficam fixos no código
```

Toda alteração passa por `Document::edit`, que registra uma `Transaction` reversível. As edições contínuas (arrastar valores) são agrupadas em um único passo de desfazer.

## Desenvolvimento

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

## Próximos passos

- Exportação e importação IFC.
- Exportação das folhas em PDF/SVG.
- Cotas e anotações.
- Famílias paramétricas do usuário.
- Escadas e guarda-corpos.
- Eixos (grids).
- Telhados de duas e quatro águas.
- Paredes amarradas ao nível superior.
- Texturas de materiais.

## Licença

GPL-3.0-or-later © Francis Rebouças <francisreboucas@gmail.com>.
