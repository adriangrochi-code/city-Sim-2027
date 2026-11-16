extends CanvasLayer
## Interfaz con estética de plano colonial: papel envejecido y tinta sepia.

signal tool_selected(tool: int)
signal option_selected(index: int)
signal choice_made(index: int)
signal speed_selected(speed: int)

const PAPER := Color(0.91, 0.87, 0.77, 0.95)
const PAPER_DARK := Color(0.82, 0.76, 0.63, 0.97)
const INK := Color(0.24, 0.17, 0.10)
const BORDER := Color(0.54, 0.43, 0.30)

var _labels := {}
var _tool_buttons: Array[Button] = []
var _opt_buttons: Array[Button] = []
var _opt_row: HBoxContainer
var _opt_label: Label
var _speed_buttons: Array[Button] = []
var _hint: Label
var _info: Label
var _info_panel: PanelContainer
var _toast: Label
var _toast_tween: Tween
var _choice: PanelContainer
var _choice_box: VBoxContainer
var _root: Control


func setup(tool_defs: Array) -> void:
	var root := Control.new()
	root.set_anchors_preset(Control.PRESET_FULL_RECT)
	root.mouse_filter = Control.MOUSE_FILTER_IGNORE
	root.theme = _make_theme()
	add_child(root)
	_root = root

	# Barra superior.
	var top := PanelContainer.new()
	top.set_anchors_and_offsets_preset(Control.PRESET_TOP_WIDE)
	root.add_child(top)
	var hb := HBoxContainer.new()
	hb.add_theme_constant_override("separation", 16)
	top.add_child(hb)
	for key in ["date", "money", "pop", "pool", "era", "stage", "parcels", "wind"]:
		var l := Label.new()
		hb.add_child(l)
		_labels[key] = l
	var spacer := Control.new()
	spacer.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	spacer.mouse_filter = Control.MOUSE_FILTER_IGNORE
	hb.add_child(spacer)
	var clock := Label.new()
	hb.add_child(clock)
	_labels["clock"] = clock
	var speeds := ["II", ">", ">>", ">>>"]
	for i in speeds.size():
		var b := Button.new()
		b.text = speeds[i]
		b.toggle_mode = true
		b.focus_mode = Control.FOCUS_NONE
		b.custom_minimum_size = Vector2(40, 0)
		b.pressed.connect(func(): speed_selected.emit(i))
		hb.add_child(b)
		_speed_buttons.append(b)

	# Barra de herramientas inferior.
	var bottom := PanelContainer.new()
	bottom.set_anchors_and_offsets_preset(Control.PRESET_CENTER_BOTTOM)
	bottom.grow_horizontal = Control.GROW_DIRECTION_BOTH
	bottom.grow_vertical = Control.GROW_DIRECTION_BEGIN
	bottom.offset_top = -10
	bottom.offset_bottom = -10
	root.add_child(bottom)
	var vb := VBoxContainer.new()
	bottom.add_child(vb)
	var tools_row := HBoxContainer.new()
	vb.add_child(tools_row)
	for i in tool_defs.size():
		var b := Button.new()
		b.text = "%s  %s" % [tool_defs[i]["key"], tool_defs[i]["name"]]
		b.toggle_mode = true
		b.focus_mode = Control.FOCUS_NONE
		b.pressed.connect(func(): tool_selected.emit(i))
		tools_row.add_child(b)
		_tool_buttons.append(b)
	_opt_row = HBoxContainer.new()
	_opt_row.alignment = BoxContainer.ALIGNMENT_CENTER
	vb.add_child(_opt_row)
	_opt_label = Label.new()
	_opt_row.add_child(_opt_label)

	# Instrucciones de la herramienta (abajo a la izquierda).
	var hint_panel := PanelContainer.new()
	hint_panel.set_anchors_and_offsets_preset(Control.PRESET_BOTTOM_LEFT)
	hint_panel.grow_vertical = Control.GROW_DIRECTION_BEGIN
	hint_panel.offset_left = 12
	hint_panel.offset_bottom = -110
	hint_panel.offset_top = -110
	root.add_child(hint_panel)
	_hint = Label.new()
	_hint.custom_minimum_size = Vector2(380, 0)
	_hint.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	hint_panel.add_child(_hint)

	# Panel de información (derecha).
	_info_panel = PanelContainer.new()
	_info_panel.set_anchors_and_offsets_preset(Control.PRESET_TOP_RIGHT)
	_info_panel.grow_horizontal = Control.GROW_DIRECTION_BEGIN
	_info_panel.offset_top = 60
	_info_panel.offset_right = -12
	_info_panel.offset_left = -12
	_info_panel.visible = false
	root.add_child(_info_panel)
	_info = Label.new()
	_info.custom_minimum_size = Vector2(300, 0)
	_info.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_info_panel.add_child(_info)

	# Aviso flotante (gaceta).
	_toast = Label.new()
	_toast.set_anchors_and_offsets_preset(Control.PRESET_CENTER_TOP)
	_toast.grow_horizontal = Control.GROW_DIRECTION_BOTH
	_toast.offset_top = 64
	_toast.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	_toast.add_theme_font_size_override("font_size", 19)
	_toast.add_theme_color_override("font_color", Color(1.0, 0.95, 0.82))
	_toast.add_theme_color_override("font_outline_color", Color(0.15, 0.10, 0.05))
	_toast.add_theme_constant_override("outline_size", 6)
	_toast.modulate.a = 0.0
	root.add_child(_toast)

	var help := Label.new()
	help.text = "WASD mover · Q/E girar · rueda zoom · botón central orbitar · botón derecho arrastrar · Espacio pausa · Esc cancelar"
	help.set_anchors_and_offsets_preset(Control.PRESET_BOTTOM_RIGHT)
	help.grow_horizontal = Control.GROW_DIRECTION_BEGIN
	help.grow_vertical = Control.GROW_DIRECTION_BEGIN
	help.offset_right = -12
	help.offset_bottom = -110
	help.offset_top = -110
	help.add_theme_color_override("font_color", Color(1, 1, 1, 0.85))
	help.add_theme_color_override("font_outline_color", Color(0, 0, 0, 0.7))
	help.add_theme_constant_override("outline_size", 4)
	help.add_theme_font_size_override("font_size", 13)
	root.add_child(help)


func _make_theme() -> Theme:
	var t := Theme.new()
	t.default_font_size = 15
	var panel := _sb(PAPER)
	t.set_stylebox("panel", "PanelContainer", panel)
	t.set_stylebox("normal", "Button", _sb(PAPER_DARK, 1))
	t.set_stylebox("hover", "Button", _sb(PAPER_DARK.lightened(0.15), 1))
	t.set_stylebox("pressed", "Button", _sb(Color(0.55, 0.42, 0.28), 1))
	t.set_stylebox("hover_pressed", "Button", _sb(Color(0.60, 0.46, 0.31), 1))
	t.set_stylebox("disabled", "Button", _sb(PAPER_DARK.darkened(0.1), 1))
	t.set_stylebox("focus", "Button", StyleBoxEmpty.new())
	t.set_color("font_color", "Label", INK)
	t.set_color("font_color", "Button", INK)
	t.set_color("font_hover_color", "Button", INK)
	t.set_color("font_pressed_color", "Button", Color(0.98, 0.94, 0.85))
	t.set_color("font_hover_pressed_color", "Button", Color(0.98, 0.94, 0.85))
	t.set_color("font_disabled_color", "Button", Color(0.5, 0.45, 0.38))
	return t


func _sb(col: Color, border := 2) -> StyleBoxFlat:
	var sb := StyleBoxFlat.new()
	sb.bg_color = col
	sb.border_color = BORDER
	sb.set_border_width_all(border)
	sb.set_corner_radius_all(3)
	sb.content_margin_left = 10
	sb.content_margin_right = 10
	sb.content_margin_top = 6
	sb.content_margin_bottom = 6
	return sb


## Decisión importante en un panel central (por ejemplo, abrir la ciudad).
func show_choice(title: String, text: String, options: Array) -> void:
	if _choice != null:
		_choice.queue_free()
	_choice = PanelContainer.new()
	_choice.set_anchors_and_offsets_preset(Control.PRESET_CENTER)
	_choice.grow_horizontal = Control.GROW_DIRECTION_BOTH
	_choice.grow_vertical = Control.GROW_DIRECTION_BOTH
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


func select_tool(i: int) -> void:
	for k in _tool_buttons.size():
		_tool_buttons[k].set_pressed_no_signal(k == i)


## Segunda fila de la barra: opciones de la herramienta actual (tipo de calle, de pozo…).
func set_options(title: String, names: Array, selected: int) -> void:
	for b in _opt_buttons:
		b.queue_free()
	_opt_buttons.clear()
	_opt_label.text = title
	_opt_row.visible = not names.is_empty()
	for i in names.size():
		var b := Button.new()
		b.text = names[i]
		b.toggle_mode = true
		b.focus_mode = Control.FOCUS_NONE
		b.pressed.connect(func(): option_selected.emit(i))
		_opt_row.add_child(b)
		_opt_buttons.append(b)
	select_option(selected)


func select_option(i: int) -> void:
	for k in _opt_buttons.size():
		_opt_buttons[k].set_pressed_no_signal(k == i)


func select_speed(i: int) -> void:
	for k in _speed_buttons.size():
		_speed_buttons[k].set_pressed_no_signal(k == i)


func set_tool_enabled(i: int, on: bool) -> void:
	_tool_buttons[i].disabled = not on


func set_stats(values: Dictionary) -> void:
	for key in values:
		if _labels.has(key):
			(_labels[key] as Label).text = values[key]


func set_hint(text: String) -> void:
	_hint.text = text


func show_info(text: String) -> void:
	_info.text = text
	_info_panel.visible = text != ""


func toast(text: String) -> void:
	_toast.text = text
	_toast.modulate.a = 1.0
	if _toast_tween:
		_toast_tween.kill()
	_toast_tween = create_tween()
	_toast_tween.tween_interval(2.8)
	_toast_tween.tween_property(_toast, "modulate:a", 0.0, 1.2)


static func fmt(n: int) -> String:
	var s := str(absi(n))
	var out := ""
	while s.length() > 3:
		out = "." + s.substr(s.length() - 3) + out
		s = s.substr(0, s.length() - 3)
	return ("-" if n < 0 else "") + s + out
