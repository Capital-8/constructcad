# ConstructCAD

Software livre de **Modelagem de Informações da Construção (BIM)**, escrito em Rust e independente de qualquer fornecedor comercial.

[English](README.md) · [Como contribuir](CONTRIBUTING.md) · [Código de conduta](CODE_OF_CONDUCT.md) · [Segurança](SECURITY.md)

Um único modelo paramétrico é a fonte da verdade. Plantas, forros, cortes, elevações, vistas 3D, legendas, tabelas e folhas são *consultas* sobre esse modelo e se atualizam automaticamente a cada alteração.

## Familiar por design

Os menus, atalhos, painéis e ferramentas que suas mãos já conhecem. Produtivo desde o primeiro dia, sem reaprender.

O ConstructCAD segue as convenções comuns às ferramentas CAD e BIM de desktop: barra de acesso rápido no canto superior esquerdo, ribbon com abas, paleta de propriedades e navegador de projeto encaixados na lateral, e atalhos de teclado padrão e de duas letras. São convenções gerais; o ConstructCAD não copia ícones, arte ou textos de nenhum produto (veja [Independência](#independência)).

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
- **Estrutura e circulação**: pilares (retangulares ou circulares, topo preso a um nível), vigas, escadas retas (espelhos a partir do espelho máximo do tipo) e guarda-corpos; aba *Estrutura*; exportados como `IfcColumn`, `IfcBeam`, `IfcStair`, `IfcRailing`.
- **Telhados**: plano, uma água, duas águas e quatro águas (a menor das águas que sobem de cada beiral), com cumeeira e espigões na planta; telhados inclinados saem no IFC como B-rep de faces.
- **Ambientes**: clique dentro de paredes que delimitam ambientes para criar o ambiente; contorno, área e perímetro acompanham as paredes. Etiquetas na planta, tabela de ambientes e exportação como `IfcSpace`.
- **Eixos e anotação**: eixos com bolhas em todas as plantas; cotas alinhadas em qualquer vista 2D (aba *Anotar*); marcas de portas e janelas etiquetadas automaticamente.
- **Snapping**: extremidades, pontos médios, sobre a parede, ângulos de 15°, Shift para ortogonal, comprimento digitado + Enter.
- **Edição**: seleção por clique ou janela (contida ou cruzada), mover arrastando, copiar com Ctrl+arrastar, excluir. Desfazer e refazer cobrem todas as edições.
- **Componentes paramétricos**: mobiliário, louças e marcenaria feitos de caixas, cilindros e perfis extrudados cujas medidas são expressões sobre parâmetros nomeados (do tipo ou da instância). Editor de componentes com verificação imediata e pré-visualização em planta e vista frontal, biblioteca inicial (mesa, cadeira, cama, sofá, bacia sanitária, lavatório, armário), ferramenta *Componente* (CM), tabelas e legendas, arquivos `.ccomp` para compartilhar e exportação/importação IFC como `IfcFurniture`, `IfcSanitaryTerminal`, `IfcLightFixture` ou `IfcBuildingElementProxy`.
- **Propriedades**: parâmetros de instância e da vista em seções recolhíveis; paredes com restrição de base e de topo (altura desconectada ou até um nível), linha de localização, delimitação de ambientes e estrutural.
- **Tipos**: diálogo de propriedades do tipo com duplicar e renomear, editor de camadas com função (núcleo, acabamentos), função da parede e dados de identidade (marca de tipo, fabricante, modelo, classificação, resistência ao fogo, custo…).
- **Vista 3D** (wgpu, MSAA): orbitar, pan, zoom, ajustar ao modelo e seleção por clique.
- **Elevações e cortes**: remoção de linhas ocultas, hachura de corte e níveis anotados.
- **Tabelas** de todas as categorias do modelo e de ambientes: escolha e ordem dos campos (da instância, do tipo e calculados, como comprimento, área e volume), ordenação, filtro, totais e exportação para CSV.
- **Folhas** A0–A4: carimbo com dados do projeto e viewports arrastáveis em escala; exportação vetorial para PDF e SVG (*Arquivo → Exportar PDF/SVG…*), idêntica à tela.
- **Gerenciar**: informações do projeto, níveis (cada nível novo cria suas plantas) e materiais.
- **Arquivo `.ccad`**: RON versionado e legível; o salvamento é atômico.
- **Exportação IFC4** (*Arquivo → Exportar IFC…*): pavimentos, paredes com aberturas, portas, janelas, pisos, forros, telhados, estrutura e componentes como sólidos extrudados, com tipos, camadas de materiais, cores e quantitativos. Gerada pelo escritor STEP do próprio ConstructCAD, sem bibliotecas IFC de terceiros.
- **Importação IFC** (*Arquivo → Importar IFC…*, IFC2X3 e IFC4): pavimentos viram níveis; paredes, portas, janelas, lajes, forros, telhados, ambientes, pilares, vigas, mobiliário e louças viram elementos nativos com tipos (existentes ou novos), materiais e cores. Lida pelo leitor STEP do próprio ConstructCAD; produtos não suportados são contados e ignorados.

## Vindo do Revit?

O ConstructCAD não lê arquivos `.rvt`. Para trazer um modelo do Revit, exporte-o para IFC no próprio Revit (*File → Export → IFC*, IFC4 ou IFC2x3) e abra o arquivo em *Arquivo → Importar IFC…*. Pavimentos, paredes, portas, janelas, lajes, forros, telhados, ambientes, pilares e vigas chegam como elementos nativos e editáveis, com seus tipos e materiais; a mensagem da importação informa o que foi ignorado.

Quem não tem o Revit pode usar ferramentas independentes de terceiros que convertem `.rvt` para IFC; verifique por conta própria a cobertura e a licença delas. O ConstructCAD não as inclui nem as endossa.

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
| Ctrl+N / Ctrl+O | Novo / Abrir | Ctrl+W | Fechar projeto |
| RM | Ambiente | GR | Eixo |
| DI | Cota alinhada | CL | Pilar |
| BM | Viga | ST | Escada |
| RA | Guarda-corpo | CM | Componente |

Navegação 2D: roda do mouse dá zoom e o botão do meio faz pan. Navegação 3D: arrastar com o botão esquerdo orbita, botão direito ou do meio faz pan e a roda dá zoom.

## Arquitetura

```
crates/
  cc_core    modelo BIM (elementos, tipos, níveis, vistas), transações undo/redo, persistência .ccad
  cc_geom    geometria derivada: paredes com junções e aberturas, lajes, telhados, cortes, picking
  cc_views   geração de desenhos 2D (planta, forro, elevação, corte, axonometria, legenda), tabelas, montagem das folhas e exportação PDF/SVG
  cc_render  renderizador 3D offscreen em wgpu
  cc_ifc     exportação IFC4 e importação IFC2X3/IFC4: escritor e leitor STEP, mapeamento do modelo
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

Os arquivos IFC exportados são validados na CI com o [IfcOpenShell](https://ifcopenshell.org/), uma referência independente e de código aberto usada apenas nos testes:

```sh
pip install -r scripts/requirements-ifc.txt
cargo run -p cc_ifc --example export_ifc -- examples/casa_simples.ccad casa_simples.ifc
python scripts/validate_ifc.py casa_simples.ifc
```

## Próximos passos

- Componentes em série, aninhados e hospedados (em paredes e forros).
- Importação IFC de escadas, guarda-corpos, eixos e geometria livre.
- Arquitetura em que tudo é um comando (scripts e macros).
- Texturas de materiais.

## Independência

O ConstructCAD é um projeto independente. O comportamento do programa foi projetado com base em documentação pública, convenções gerais de CAD e retorno dos usuários. Os testes de referência usam valores analíticos e ferramentas de código aberto. O ConstructCAD não tem afiliação com a Autodesk, nem é patrocinado ou endossado por ela. Autodesk e Revit são marcas registradas da Autodesk, Inc.

> ConstructCAD is independent. Behaviour is designed from public documentation, general CAD conventions and user feedback. Reference tests use analytic values and open-source tools. ConstructCAD is not affiliated with, sponsored by or endorsed by Autodesk. Autodesk and Revit are trademarks of Autodesk, Inc.

## Licença

Copyright © Francis Rebouças <francisreboucas@gmail.com>.

O ConstructCAD é distribuído sob a [GNU General Public License v3.0 ou posterior](LICENSE) (GPL-3.0-or-later).

O ícone do aplicativo é de [Magnific – Flaticon](https://www.flaticon.com/) (veja `assets/icons/FLATICON.txt`). A fonte [Inter](https://rsms.me/inter/) incluída é © The Inter Project Authors e distribuída sob a [SIL Open Font License 1.1](assets/fonts/OFL-Inter.txt).
