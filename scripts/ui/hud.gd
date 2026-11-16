extends CanvasLayer
## Interfaz con estructura de constructor de ciudades moderno (barra de categorías con
## cajón de tarjetas, barra superior de fichas, paneles laterales) y estética de plano
## colonial: papel envejecido y tinta sepia.

signal tool_selected(tool: int)
signal card_selected(tool: int, option: int)
signal option_selected(index: int)
signal choice_made(index: int)
signal action_pressed(index: int)
signal speed_selected(speed: int)

const Icons = preload("res://scripts/ui/icons.gd")

const PAPER := Color(0.93, 0.89, 0.80, 0.96)
const PAPER_DARK := Color(0.84, 0.78, 0.65, 0.98)
const PAPER_LIGHT := Color(0.97, 0.94, 0.87, 0.98)
const INK := Color(0.24, 0.17, 0.10)
const INK_SOFT := Color(0.45, 0.36, 0.26)
const BORDER := Color(0.54, 0.43, 0.30)
const ACCENT := Color(0.55, 0.36, 0.20)
const STAT_ICONS := {"date": "scroll", "money": "coin", "pop": "people", "pool": "house", "era": "crown",
	"stage": "crown", "bricks": "brick", "stone": "rocks", "bread": "bread", "parcels": "parcels", "wind": "wind", "clock": "clock"}
const STAT_TIPS := {"date": "Fecha", "clock": "Hora y vecinos en la calle", "money": "Tesoro de la ciudad (pesos)",
	"pop": "Población", "pool": "Familias que esperan casa", "stage": "Etapa y época de la ciudad",
	"bricks": "Ladrillos en depósito", "stone": "Piedra de cantera", "bread": "Precio del pan (1,0 = normal)",
	"parcels": "Parcelas propias / permitidas", "wind": "Viento e incendios"}

var _labels := {}
var _cat_buttons: Array[Button] = []
var _categories: Array = []
var _cards := []            # [{"button", "tool", "option", "cat"}]
var _drawer: PanelContainer
var _drawer_box: HBoxContainer
var _drawer_title: Label
var _open_cat := -1
var _opt_buttons: Array[Button] = []
var _opt_panel: PanelContainer
var _opt_row: HBoxContainer
var _opt_label: Label
var _speed_buttons: Array[Button] = []
var _hint_panel: PanelContainer
var _hint: Label
var _info: Label
var _actions: VBoxContainer
var _actions_key := ""
var _actions_scroll: ScrollContainer
var _info_panel: PanelContainer
var _toast: Label
var _toast_tween: Tween
var _choice: PanelContainer
var _root: Control
var _disabled_tools := {}
var _current_tool := -1
var _current_option := -1


## `categories`: [{"name", "icon", "tool" (opcional, categoría directa), "items": [{"name", "icon", "tool", "option", "cost", "key"}]}]
func setup(categories: Array) -> void:
	_categories = categories
	var root := Control.new()
	root.set_anchors_preset(Control.PRESET_FULL_RECT)
	root.mouse_filter = Control.MOUSE_FILTER_IGNORE
	root.theme = _make_theme()
	add_child(root)
	_root = root
	_build_top_bar()
	_build_bottom_bar()
	_build_side_panels()


# --- Construcción -------------------------------------------------------------------

func _build_top_bar() -> void:
	var top := PanelContainer.new()
	top.set_anchors_and_offsets_preset(Control.PRESET_TOP_WIDE)
	top.add_theme_stylebox_override("panel", _sb(PAPER, 0, 0, 6))
	_root.add_child(top)
	var hb := HBoxContainer.new()
	hb.add_theme_constant_override("separation", 4)
	top.add_child(hb)
	# Izquierda: fecha, hora y velocidad.
	hb.add_child(_chip("date"))
	hb.add_child(_chip("clock"))
	var speeds := ["II", "▸", "▸▸", "▸▸▸"]
	for i in speeds.size():
		var b := Button.new()
		b.text = speeds[i]
		b.toggle_mode = true
		b.focus_mode = Control.FOCUS_NONE
		b.custom_minimum_size = Vector2(34, 0)
		b.tooltip_text = ["Pausa (Espacio)", "Velocidad normal", "Rápido", "Muy rápido"][i]
		b.pressed.connect(func(): speed_selected.emit(i))
		hb.add_child(b)
		_speed_buttons.append(b)
	hb.add_child(_spacer())
	# Centro: lo principal de la ciudad.
	for k in ["money", "pop", "pool"]:
		hb.add_child(_chip(k, true))
	hb.add_child(_spacer())
	# Derecha: recursos, época y entorno.
	for k in ["stage", "bricks", "stone", "bread", "parcels", "wind"]:
		hb.add_child(_chip(k))


func _build_bottom_bar() -> void:
	var bar := PanelContainer.new()
	bar.set_anchors_and_offsets_preset(Control.PRESET_CENTER_BOTTOM)
	bar.grow_horizontal = Control.GROW_DIRECTION_BOTH
	bar.grow_vertical = Control.GROW_DIRECTION_BEGIN
	bar.offset_top = -8
	bar.offset_bottom = -8
	bar.add_theme_stylebox_override("panel", _sb(PAPER, 2, 10, 8))
	_root.add_child(bar)
	var hb := HBoxContainer.new()
	hb.add_theme_constant_override("separation", 4)
	bar.add_child(hb)
	for ci in _categories.size():
		var cat: Dictionary = _categories[ci]
		var b := Button.new()
		b.icon = Icons.texture(cat["icon"], 30)
		b.text = cat["name"]
		b.icon_alignment = HORIZONTAL_ALIGNMENT_CENTER
		b.vertical_icon_alignment = VERTICAL_ALIGNMENT_TOP
		b.toggle_mode = true
		b.focus_mode = Control.FOCUS_NONE
		b.custom_minimum_size = Vector2(94, 66)
		b.add_theme_font_size_override("font_size", 13)
		b.tooltip_text = cat.get("tooltip", cat["name"])
		b.pressed.connect(func(): _on_category(ci))
		hb.add_child(b)
		_cat_buttons.append(b)

	# Cajón de tarjetas (se abre encima de la barra).
	_drawer = PanelContainer.new()
	_drawer.set_anchors_and_offsets_preset(Control.PRESET_CENTER_BOTTOM)
	_drawer.grow_horizontal = Control.GROW_DIRECTION_BOTH
	_drawer.grow_vertical = Control.GROW_DIRECTION_BEGIN
	_drawer.offset_top = -98
	_drawer.offset_bottom = -98
	_drawer.add_theme_stylebox_override("panel", _sb(PAPER_LIGHT, 2, 10, 10))
	_drawer.visible = false
	_root.add_child(_drawer)
	var vb := VBoxContainer.new()
	_drawer.add_child(vb)
	_drawer_title = Label.new()
	_drawer_title.add_theme_font_size_override("font_size", 14)
	_drawer_title.add_theme_color_override("font_color", INK_SOFT)
	vb.add_child(_drawer_title)
	_drawer_box = HBoxContainer.new()
	_drawer_box.add_theme_constant_override("separation", 6)
	vb.add_child(_drawer_box)

	# Interruptores de la herramienta (tipo de calle, etc.), junto al cajón.
	_opt_panel = PanelContainer.new()
	_opt_panel.set_anchors_and_offsets_preset(Control.PRESET_CENTER_BOTTOM)
	_opt_panel.grow_horizontal = Control.GROW_DIRECTION_BOTH
	_opt_panel.grow_vertical = Control.GROW_DIRECTION_BEGIN
	_opt_panel.offset_top = -98
	_opt_panel.offset_bottom = -98
	_opt_panel.add_theme_stylebox_override("panel", _sb(PAPER_LIGHT, 2, 10, 6))
	_opt_panel.visible = false
	_root.add_child(_opt_panel)
	_opt_row = HBoxContainer.new()
	_opt_panel.add_child(_opt_row)
	_opt_label = Label.new()
	_opt_label.add_theme_color_override("font_color", INK_SOFT)
	_opt_row.add_child(_opt_label)


func _build_side_panels() -> void:
	# Pista de la herramienta (abajo a la izquierda).
	_hint_panel = PanelContainer.new()
	_hint_panel.set_anchors_and_offsets_preset(Control.PRESET_TOP_LEFT)
	_hint_panel.offset_left = 12
	_hint_panel.offset_top = 52
	_hint_panel.add_theme_stylebox_override("panel", _sb(PAPER, 1, 8, 8))
	_root.add_child(_hint_panel)
	_hint = Label.new()
	_hint.custom_minimum_size = Vector2(300, 0)
	_hint.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_hint.add_theme_font_size_override("font_size", 13)
	_hint_panel.add_child(_hint)

	# Panel de información (derecha).
	_info_panel = PanelContainer.new()
	_info_panel.set_anchors_and_offsets_preset(Control.PRESET_TOP_RIGHT)
	_info_panel.grow_horizontal = Control.GROW_DIRECTION_BEGIN
	_info_panel.offset_top = 52
	_info_panel.offset_right = -12
	_info_panel.offset_left = -12
	_info_panel.add_theme_stylebox_override("panel", _sb(PAPER, 2, 8, 12))
	_info_panel.visible = false
	_root.add_child(_info_panel)
	var info_box := VBoxContainer.new()
	info_box.add_theme_constant_override("separation", 6)
	_info_panel.add_child(info_box)
	_info = Label.new()
	_info.custom_minimum_size = Vector2(360, 0)
	_info.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	info_box.add_child(_info)
	_actions_scroll = ScrollContainer.new()
	_actions_scroll.horizontal_scroll_mode = ScrollContainer.SCROLL_MODE_DISABLED
	info_box.add_child(_actions_scroll)
	_actions = VBoxContainer.new()
	_actions.add_theme_constant_override("separation", 6)
	_actions.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_actions_scroll.add_child(_actions)

	# Aviso (gaceta).
	_toast = Label.new()
	_toast.set_anchors_and_offsets_preset(Control.PRESET_CENTER_TOP)
	_toast.grow_horizontal = Control.GROW_DIRECTION_BOTH
	_toast.offset_top = 56
	_toast.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	_toast.add_theme_font_size_override("font_size", 18)
	_toast.add_theme_color_override("font_color", Color(1.0, 0.96, 0.85))
	_toast.add_theme_color_override("font_outline_color", Color(0.18, 0.12, 0.06))
	_toast.add_theme_constant_override("outline_size", 6)
	_toast.modulate.a = 0.0
	_root.add_child(_toast)

	var help := Label.new()
	help.text = "WASD mover · Q/E girar\nRueda: zoom · central: orbitar\nDerecho: arrastrar · Esc: cancelar"
	help.set_anchors_and_offsets_preset(Control.PRESET_BOTTOM_LEFT)
	help.grow_vertical = Control.GROW_DIRECTION_BEGIN
	help.offset_left = 12
	help.offset_bottom = -10
	help.offset_top = -10
	help.add_theme_color_override("font_color", Color(1, 1, 1, 0.8))
	help.add_theme_color_override("font_outline_color", Color(0, 0, 0, 0.7))
	help.add_theme_constant_override("outline_size", 4)
	help.add_theme_font_size_override("font_size", 12)
	_root.add_child(help)


func _chip(key: String, big := false) -> Control:
	var p := PanelContainer.new()
	p.tooltip_text = STAT_TIPS.get(key, "")
	p.add_theme_stylebox_override("panel", _sb(PAPER_LIGHT if big else PAPER_DARK.lightened(0.25), 1, 8, 4))
	var hb := HBoxContainer.new()
	hb.add_theme_constant_override("separation", 5)
	p.add_child(hb)
	var ic := TextureRect.new()
	ic.texture = Icons.texture(STAT_ICONS.get(key, "eye"), 18)
	ic.stretch_mode = TextureRect.STRETCH_KEEP_CENTERED
	hb.add_child(ic)
	var l := Label.new()
	l.add_theme_font_size_override("font_size", 15 if big else 13)
	hb.add_child(l)
	_labels[key] = l
	return p


func _spacer() -> Control:
	var s := Control.new()
	s.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	s.mouse_filter = Control.MOUSE_FILTER_IGNORE
	return s


# --- Categorías y tarjetas -----------------------------------------------------------

func _on_category(ci: int) -> void:
	var cat: Dictionary = _categories[ci]
	if cat.has("tool"):
		close_drawer()
		card_selected.emit(int(cat["tool"]), -1)
		return
	if _open_cat == ci:
		close_drawer()
		return
	_open_cat = ci
	for c in _drawer_box.get_children():
		c.queue_free()
	_cards = _cards.filter(func(x): return x["cat"] != ci and is_instance_valid(x["button"]))
	_drawer_title.text = cat["name"]
	for item in cat["items"]:
		var b := Button.new()
		b.icon = Icons.texture(item["icon"], 34)
		b.text = "%s\n%s" % [item["name"], item.get("cost", "")]
		b.icon_alignment = HORIZONTAL_ALIGNMENT_CENTER
		b.vertical_icon_alignment = VERTICAL_ALIGNMENT_TOP
		b.toggle_mode = true
		b.focus_mode = Control.FOCUS_NONE
		b.custom_minimum_size = Vector2(118, 92)
		b.add_theme_font_size_override("font_size", 12)
		b.tooltip_text = "%s%s" % [item["name"], (" (tecla %s)" % item["key"]) if item.get("key", "") != "" else ""]
		var tool: int = item["tool"]
		var opt: int = item.get("option", -1)
		b.disabled = _disabled_tools.has(tool)
		b.pressed.connect(func(): card_selected.emit(tool, opt))
		_drawer_box.add_child(b)
		_cards.append({"button": b, "tool": tool, "option": opt, "cat": ci})
	_drawer.visible = true
	_place_options()
	_refresh_highlight()


func close_drawer() -> void:
	_open_cat = -1
	_drawer.visible = false
	_place_options()
	_refresh_highlight()


func _place_options() -> void:
	# Los interruptores van encima del cajón si está abierto.
	var y := -236 if _drawer.visible else -98
	_opt_panel.offset_top = y
	_opt_panel.offset_bottom = y


func select_tool(t: int) -> void:
	_current_tool = t
	_refresh_highlight()


func select_card(t: int, option: int) -> void:
	_current_tool = t
	_current_option = option
	_refresh_highlight()


func _refresh_highlight() -> void:
	for ci in _cat_buttons.size():
		var cat: Dictionary = _categories[ci]
		var has := int(cat.get("tool", -99)) == _current_tool
		for item in cat.get("items", []):
			if int(item["tool"]) == _current_tool:
				has = true
		_cat_buttons[ci].set_pressed_no_signal(has or ci == _open_cat)
	for c in _cards:
		if is_instance_valid(c["button"]):
			var on: bool = c["tool"] == _current_tool and (c["option"] == -1 or c["option"] == _current_option)
			(c["button"] as Button).set_pressed_no_signal(on)


func set_tool_enabled(t: int, on: bool) -> void:
	if on:
		_disabled_tools.erase(t)
	else:
		_disabled_tools[t] = true
	for c in _cards:
		if is_instance_valid(c["button"]) and c["tool"] == t:
			(c["button"] as Button).disabled = not on


## Interruptores de la herramienta actual (tipo de calle, etc.).
func set_options(title: String, names: Array, selected: int) -> void:
	for b in _opt_buttons:
		b.queue_free()
	_opt_buttons.clear()
	_opt_label.text = title + " "
	_opt_panel.visible = not names.is_empty() and title.begins_with("Tipo")
	for i in names.size():
		var b := Button.new()
		b.text = names[i]
		b.toggle_mode = true
		b.focus_mode = Control.FOCUS_NONE
		b.pressed.connect(func(): option_selected.emit(i))
		_opt_row.add_child(b)
		_opt_buttons.append(b)
	select_option(selected)
	_place_options()


func select_option(i: int) -> void:
	_current_option = i
	for k in _opt_buttons.size():
		_opt_buttons[k].set_pressed_no_signal(k == i)
	_refresh_highlight()


func select_speed(i: int) -> void:
	for k in _speed_buttons.size():
		_speed_buttons[k].set_pressed_no_signal(k == i)


# --- Contenido ---------------------------------------------------------------------

func set_stats(values: Dictionary) -> void:
	for key in values:
		if _labels.has(key):
			(_labels[key] as Label).text = values[key]


func set_hint(text: String) -> void:
	_hint.text = text
	_hint_panel.visible = text != ""


func show_info(text: String) -> void:
	_info.text = text
	_info_panel.visible = text != ""
	if text == "":
		set_actions([])


## Botones de acción del elemento inspeccionado: [[texto, habilitado], ...].
func set_actions(actions: Array) -> void:
	var key := str(actions)
	if key == _actions_key:
		return
	_actions_key = key
	for c in _actions.get_children():
		c.queue_free()
	for i in actions.size():
		# "Título · descripción": el botón lleva el título y debajo va la descripción.
		var parts: PackedStringArray = str(actions[i][0]).split(" · ", true, 1)
		var b := Button.new()
		b.text = parts[0]
		b.disabled = not actions[i][1]
		b.focus_mode = Control.FOCUS_NONE
		b.alignment = HORIZONTAL_ALIGNMENT_LEFT
		b.clip_text = true
		b.custom_minimum_size = Vector2(340, 0)
		b.add_theme_font_size_override("font_size", 13)
		b.pressed.connect(func(): action_pressed.emit(i))
		_actions.add_child(b)
		if parts.size() > 1:
			b.tooltip_text = parts[1]
			var d := Label.new()
			d.text = parts[1]
			d.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
			d.custom_minimum_size = Vector2(340, 0)
			d.add_theme_font_size_override("font_size", 12)
			d.add_theme_color_override("font_color", INK_SOFT)
			_actions.add_child(d)
	# Con muchas acciones, el panel se desplaza en lugar de salirse de la pantalla.
	var long := actions.size() > 4
	_actions_scroll.custom_minimum_size = Vector2(350, 330 if long else 0)
	_actions_scroll.vertical_scroll_mode = ScrollContainer.SCROLL_MODE_AUTO if long else ScrollContainer.SCROLL_MODE_DISABLED


func toast(text: String) -> void:
	_toast.text = text
	_toast.modulate.a = 1.0
	if _toast_tween:
		_toast_tween.kill()
	_toast_tween = create_tween()
	_toast_tween.tween_interval(2.8)
	_toast_tween.tween_property(_toast, "modulate:a", 0.0, 1.2)


## Decisión importante en un panel central (por ejemplo, abrir la ciudad).
func show_choice(title: String, text: String, options: Array) -> void:
	if _choice != null:
		_choice.queue_free()
	_choice = PanelContainer.new()
	_choice.set_anchors_and_offsets_preset(Control.PRESET_CENTER)
	_choice.grow_horizontal = Control.GROW_DIRECTION_BOTH
	_choice.grow_vertical = Control.GROW_DIRECTION_BOTH
	_choice.add_theme_stylebox_override("panel", _sb(PAPER_LIGHT, 2, 10, 18))
	_root.add_child(_choice)
	var vb := VBoxContainer.new()
	vb.add_theme_constant_override("separation", 10)
	_choice.add_child(vb)
	var t := Label.new()
	t.text = title
	t.add_theme_font_size_override("font_size", 22)
	t.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	vb.add_child(t)
	var body := Label.new()
	body.text = text
	body.custom_minimum_size = Vector2(560, 0)
	body.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	vb.add_child(body)
	for i in options.size():
		var b := Button.new()
		b.text = options[i]
		b.focus_mode = Control.FOCUS_NONE
		b.pressed.connect(func():
			_choice.queue_free()
			_choice = null
			choice_made.emit(i))
		vb.add_child(b)


func choice_open() -> bool:
	return _choice != null


# --- Tema --------------------------------------------------------------------------

func _make_theme() -> Theme:
	var t := Theme.new()
	t.default_font_size = 15
	t.set_stylebox("panel", "PanelContainer", _sb(PAPER, 2, 6, 8))
	t.set_stylebox("normal", "Button", _sb(PAPER_DARK, 1, 6, 6))
	t.set_stylebox("hover", "Button", _sb(PAPER_LIGHT, 1, 6, 6))
	t.set_stylebox("pressed", "Button", _sb(Color(0.80, 0.66, 0.46), 2, 6, 6, ACCENT))
	t.set_stylebox("hover_pressed", "Button", _sb(Color(0.84, 0.70, 0.50), 2, 6, 6, ACCENT))
	t.set_stylebox("disabled", "Button", _sb(PAPER_DARK.darkened(0.08), 1, 6, 6))
	t.set_stylebox("focus", "Button", StyleBoxEmpty.new())
	t.set_color("font_color", "Label", INK)
	t.set_color("font_color", "Button", INK)
	t.set_color("font_hover_color", "Button", INK)
	t.set_color("font_pressed_color", "Button", INK)
	t.set_color("font_hover_pressed_color", "Button", INK)
	t.set_color("font_disabled_color", "Button", Color(0.55, 0.50, 0.42))
	t.set_stylebox("panel", "TooltipPanel", _sb(PAPER_LIGHT, 1, 4, 6))
	t.set_color("font_color", "TooltipLabel", INK)
	return t


func _sb(col: Color, border := 1, radius := 6, margin := 6, border_col := BORDER) -> StyleBoxFlat:
	var sb := StyleBoxFlat.new()
	sb.bg_color = col
	sb.border_color = border_col
	sb.set_border_width_all(border)
	sb.set_corner_radius_all(radius)
	sb.content_margin_left = margin + 2
	sb.content_margin_right = margin + 2
	sb.content_margin_top = margin
	sb.content_margin_bottom = margin
	sb.shadow_color = Color(0, 0, 0, 0.18)
	sb.shadow_size = 3 if border > 0 else 0
	return sb


static func fmt(n: int) -> String:
	var s := str(absi(n))
	var out := ""
	while s.length() > 3:
		out = "." + s.substr(s.length() - 3) + out
		s = s.substr(0, s.length() - 3)
	return ("-" if n < 0 else "") + s + out
