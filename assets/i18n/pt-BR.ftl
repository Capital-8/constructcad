# ConstructCAD — Português (Brasil)
# Formato: subconjunto de Fluent (chave = valor, placeholders { $nome }).

# Categorias
category-wall = Paredes
category-door = Portas
category-window = Janelas
category-floor = Pisos
category-ceiling = Forros
category-roof = Telhados

# Navegador de projeto
browser-title = Navegador de projeto
browser-floor-plans = Plantas de piso
browser-ceiling-plans = Plantas de forro
browser-3d-views = Vistas 3D
browser-elevations = Elevações
browser-sections = Cortes
browser-legends = Legendas
browser-schedules = Tabelas
browser-sheets = Folhas
browser-new = + Novo
browser-open = Abrir
browser-rename = Renomear
browser-duplicate = Duplicar
browser-delete = Excluir
browser-place-on-sheet = Inserir na folha ativa

# Títulos de abas
tab-floor-plan = Planta: { $name }
tab-ceiling-plan = Forro: { $name }
tab-elevation = Elevação: { $name }
tab-section = Corte: { $name }

# Faixa de opções
ribbon-file = Arquivo
ribbon-architecture = Arquitetura
ribbon-modify = Modificar
ribbon-view = Vista
ribbon-manage = Gerenciar

# Ferramentas
tool-select = Selecionar
tool-wall = Parede
tool-door = Porta
tool-window = Janela
tool-floor = Piso
tool-ceiling = Forro
tool-roof = Telhado
tool-section = Corte

# Ações
action-new = Novo
action-open = Abrir…
action-save = Salvar
action-save-as = Salvar como…
action-export-ifc = Exportar IFC…
action-undo = Desfazer
action-redo = Refazer
action-copy = Copiar
action-delete = Excluir
action-cancel = Cancelar
action-discard = Descartar
action-ok = OK
action-zoom-fit = Ajustar zoom

# Vista
view-new-floor-plan = Planta de piso
view-new-ceiling-plan = Planta de forro
view-new-3d = Vista 3D
view-elevations = Elevações
view-new-legend = Legenda
view-new-schedule = Tabela
view-new-sheet = Folha

# Dicas
hint-shortcut = Atalho
hint-copy = Duplica a seleção deslocada 1 m (ou Ctrl + arrastar)
hint-select = Clique para selecionar · arraste para mover (Ctrl copia) · janela: esquerda-direita seleciona contidos, direita-esquerda seleciona cruzados
hint-wall = Clique o início e os próximos pontos · digite o comprimento + Enter · Shift = ortogonal · Esc encerra
hint-hosted = Passe o cursor sobre uma parede e clique · Espaço inverte a abertura
hint-sketch = Clique os vértices · clique no primeiro ponto ou Enter para concluir · Backspace remove o último
hint-from-walls = Cria o contorno a partir das paredes fechadas deste nível

# Barra de opções
sketch-finish = Concluir
sketch-from-walls = Das paredes

# Barra de status
status-length = Comprimento: { $value }
status-selected = { $count } selecionado(s)
status-unsaved-file = (não salvo)

# Mensagens
msg-plan-only = Esta ferramenta funciona em plantas. Abra uma planta de piso ou de forro.
msg-open-plan = Abra uma planta de piso para usar esta ferramenta.
msg-no-type = Nenhum tipo disponível para esta categoria.
msg-deleted = { $count } elemento(s) excluído(s).
msg-wall-too-short = Parede curta demais
msg-sketch-invalid = O contorno precisa de pelo menos 3 pontos e área maior que zero.
msg-section-hint = Clique dois pontos em uma planta para definir a linha de corte.
msg-no-wall-loop = Não foi encontrado um contorno fechado de paredes neste nível.
msg-view-already-placed = Esta vista já está em uma folha.
msg-level-in-use = O nível possui elementos e não pode ser excluído.
msg-new-project = Novo projeto criado.
msg-opened = Aberto: { $path }
msg-saved = Salvo: { $path }
msg-open-failed = Não foi possível abrir o arquivo: { $error }
msg-save-failed = Não foi possível salvar o arquivo: { $error }
msg-exported = Exportado: { $path }
msg-export-failed = Não foi possível exportar o arquivo: { $error }
msg-no-gpu = A vista 3D requer o renderizador wgpu.
msg-no-views-open = Abra uma vista no Navegador de projeto.

# Rótulos de desenho
label-offset = Deslocamento:
label-move = Mover
label-copy = Copiar

# Propriedades
props-title = Propriedades
props-nothing = Nada selecionado.
props-multi = { $count } elementos selecionados
props-edit-type = Editar tipo…
props-viewport = Viewport
prop-name = Nome
prop-scale = Escala
prop-level = Nível
prop-category = Categoria
prop-base-offset = Deslocamento da base
prop-height = Altura
prop-height-offset = Altura em relação ao nível
prop-start-x = Início X
prop-start-y = Início Y
prop-end-x = Fim X
prop-end-y = Fim Y
prop-length = Comprimento
prop-thickness = Espessura
prop-volume = Volume
prop-host = Hospedeiro
prop-offset = Posição na parede
prop-sill = Peitoril
prop-flip-facing = Inverter face
prop-flip-hand = Inverter lado
prop-overhang = Beiral
prop-roof-shape = Forma
prop-slope = Inclinação
prop-direction = Direção
prop-material = Material
prop-cut-height = Altura de corte
prop-depth = Profundidade
prop-flip-section = Inverter direção
prop-sheet-number = Número da folha
prop-sheet-size = Formato
prop-sheet-views = Vistas na folha
prop-add-view = Adicionar vista
prop-no-views = Todas as vistas já estão em folhas
prop-remove-from-sheet = Remover da folha
prop-center-x = Centro X
prop-center-y = Centro Y
prop-legend-types = Tipos exibidos
roof-flat = Plano
roof-shed = Uma água

# Colunas de tabelas
col-mark = Marca
col-type = Tipo
col-width = Largura
col-height = Altura
col-sill = Peitoril
col-level = Nível
col-host = Parede
col-comments = Comentários
col-length = Comprimento (m)
col-thickness = Espessura (m)
col-area = Área (m²)
col-perimeter = Perímetro (m)
col-volume = Volume (m³)
schedule-total = Total: { $count }
schedule-hint = Clique em uma linha para selecionar o elemento. As tabelas são atualizadas automaticamente.

# Carimbo
title-project = PROJETO
title-sheet = FOLHA
title-client = CLIENTE
title-number = NÚMERO
title-project-number = Proj. nº
title-author = Autor:

# Diálogos
dlg-project-info = Informações do projeto
dlg-levels = Níveis
dlg-materials = Materiais
dlg-type = Propriedades do tipo
dlg-unsaved = Alterações não salvas
dlg-unsaved-text = O projeto possui alterações não salvas. Deseja salvá-las?
dlg-error = Erro
dlg-about = Sobre
info-name = Nome do projeto
info-number = Número
info-client = Cliente
info-address = Endereço
info-author = Autor
level-elevation = Elevação
level-add = + Adicionar nível
material-add = + Adicionar material
type-layers = Camadas (exterior - interior)
type-total-thickness = Espessura total
type-add-layer = + Camada
type-duplicate = Duplicar tipo
about-version = Versão
about-text = Software livre de Modelagem de Informações da Construção (BIM).
about-license = Licença GPL-3.0-or-later.
file-filter = Projeto ConstructCAD
file-filter-ifc = Arquivo IFC

# Nomes padrão
default-section-name = Corte { $n }
default-3d-name = 3D { $n }
default-legend-name = Legenda { $n }
default-schedule-name = Tabela de { $category }
default-sheet-name = Nova folha
default-level-name = Nível { $n }
default-material-name = Material { $n }
copy-of = { $name } (cópia)

# Transações (histórico de desfazer)
tx-create-wall = Criar parede
tx-create-door = Inserir porta
tx-create-window = Inserir janela
tx-create-floor = Criar piso
tx-create-ceiling = Criar forro
tx-create-roof = Criar telhado
tx-create-section = Criar corte
tx-delete = Excluir
tx-move = Mover
tx-copy = Copiar
tx-modify = Modificar
tx-change-type = Alterar tipo
tx-modify-type = Modificar tipo
tx-duplicate-type = Duplicar tipo
tx-modify-view = Modificar vista
tx-new-view = Nova vista
tx-new-sheet = Nova folha
tx-duplicate-view = Duplicar vista
tx-delete-view = Excluir vista
tx-rename-view = Renomear vista
tx-place-view = Inserir vista na folha
tx-move-viewport = Mover viewport
tx-remove-viewport = Remover viewport
tx-add-level = Adicionar nível
tx-delete-level = Excluir nível
tx-modify-level = Modificar nível
tx-modify-material = Modificar material
tx-project-info = Informações do projeto

# Idioma
settings-language = Idioma
msg-language-changed = Idioma alterado. Novos projetos usarão nomes neste idioma.

# Erros de arquivo
err-io = erro de leitura/gravação ({ $detail })
err-parse = o arquivo não é um projeto ConstructCAD válido ({ $detail })
err-serialize = falha ao gerar o arquivo ({ $detail })
err-version = o arquivo usa a versão de formato { $version }; esta versão do programa suporta até { $max }

# Modelo de projeto novo (nomes de níveis, materiais, tipos e vistas)
tpl-project-name = Novo projeto
tpl-level-ground = Térreo
tpl-level-first = Pavimento 1
tpl-mat-masonry = Alvenaria
tpl-mat-plaster = Reboco
tpl-mat-concrete = Concreto
tpl-mat-wood = Madeira
tpl-mat-glass = Vidro
tpl-mat-gypsum = Gesso
tpl-mat-roof-tile = Telha cerâmica
tpl-mat-screed = Contrapiso
tpl-type-masonry-15 = Alvenaria 15 cm
tpl-type-masonry-25 = Alvenaria 25 cm
tpl-type-drywall-10 = Divisória gesso 10 cm
tpl-type-door-80 = Porta 80x210
tpl-type-door-90 = Porta 90x210
tpl-type-window-120 = Janela 120x120
tpl-type-window-150 = Janela 150x100
tpl-type-slab-12 = Laje 12 cm
tpl-type-ceiling-gypsum = Forro de gesso
tpl-type-roof-tile = Telhado cerâmico
tpl-elev-north = Norte
tpl-elev-south = Sul
tpl-elev-east = Leste
tpl-elev-west = Oeste
tpl-legend = Legenda de tipos
tpl-schedule-doors = Tabela de portas
tpl-schedule-windows = Tabela de janelas
tpl-schedule-walls = Tabela de paredes
tpl-sheet-plans = Plantas

# Bússola 3D
axis-east = L
axis-north = N
