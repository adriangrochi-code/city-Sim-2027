extends Node3D
## Prototipo 1, época colonial: fundar la ciudad, trazar calles curvas y en damero,
## zonificar, ver crecer ranchos y casas, comprar parcelas y vivir el ciclo día/noche.

const TerrainGen = preload("res://scripts/world/terrain_gen.gd")
const Terrain = preload("res://scripts/world/terrain.gd")
const Parcels = preload("res://scripts/world/parcels.gd")
const Trees = preload("res://scripts/world/trees.gd")
const DayNight = preload("res://scripts/world/day_night.gd")
const RoadNetwork = preload("res://scripts/roads/road_network.gd")
const RoadRenderer = preload("res://scripts/roads/road_renderer.gd")
const Lots = preload("res://scripts/city/lots.gd")
const BuildingGen = preload("res://scripts/city/building_gen.gd")
const CameraRig = preload("res://scripts/camera_rig.gd")
const Agents = preload("res://scripts/agents/agents.gd")
const Services = preload("res://scripts/city/services.gd")
const Fires = preload("res://scripts/city/fires.gd")
const Hud = preload("res://scripts/ui/hud.gd")
const ROAD_SHADER = preload("res://shaders/road.gdshader")

enum Tool { INSPECT, PLAZA, STRAIGHT, CURVE, GRID, ZONE, DEZONE, BULLDOZE, PARCELS, WATER }
const TOOL_DEFS := [
	{"key": "1", "name": "Inspeccionar", "hint": "Haz clic en un vecino, una carreta, un solar, una calle o el terreno para ver su información. F sigue al vecino elegido."},
	{"key": "2", "name": "Plaza mayor", "hint": "Elige el sitio de la plaza mayor (terreno firme y llano). R gira la plaza. Se crean la iglesia matriz y las calles que la rodean."},
	{"key": "3", "name": "Calle recta", "hint": "Clic en el inicio y clic en el final. Sigue encadenando tramos; Esc para terminar."},
	{"key": "4", "name": "Calle curva", "hint": "Clic en el inicio, clic en el final y mueve el ratón para doblarla. Un tercer clic la fija."},
	{"key": "5", "name": "Damero", "hint": "Clic en una esquina, clic en la otra punta del primer lado y mueve el ratón para extender las manzanas (cuadras de 84 m). Tercer clic para trazar."},
	{"key": "6", "name": "Zona mixta", "hint": "Pinta con el ratón los solares junto a las calles. Ahí se instalan los pobladores. [ y ] cambian el tamaño del pincel."},
	{"key": "7", "name": "Quitar zona", "hint": "Pinta para quitar la zona (derriba lo construido)."},
	{"key": "8", "name": "Demoler", "hint": "Haz clic sobre un tramo de calle o un pozo, aljibe o puesto para demolerlo."},
	{"key": "9", "name": "Parcelas", "hint": "Compra parcelas contiguas a las tuyas (mercedes de tierras). Se desbloquean más con la población."},
	{"key": "0", "name": "Agua", "hint": "Construye pozos, aljibes o un puesto de aguadores junto a una calle (R gira). Las casas de azotea necesitan agua y las de altos, buena agua. Con agua cerca, los vecinos apagan los incendios. Azul: buena agua · celeste: regular · rojo: sin agua."},
]
const WINDS := [["pampero", Vector2(0.7, -0.7)], ["sudestada", Vector2(-0.7, -0.7)], ["viento norte", Vector2(0.0, 1.0)], ["viento del este", Vector2(-1.0, 0.0)], ["brisa del río", Vector2(0.0, -1.0)]]
const SPEEDS := [0.0, 1.0, 2.0, 4.0]
const MONTH_SECONDS := 10.0
const MONTHS := ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre"]
const START_YEAR := 1726
const BLOCK := 84.0
const PLAZA_SIZE := 76.0
const STAGES := [[0, "Fundación"], [200, "Villa"], [1000, "Ciudad"], [4000, "Plaza fuerte"]]
const PARCEL_STEPS := [150, 500, 1000, 2000, 3000, 4000]

var gen: TerrainGen
var parcels: Parcels
var terrain: Terrain
var day: DayNight
var net: RoadNetwork
var roads: RoadRenderer
var lots: Lots
var trees: Trees
var agents: Agents
var services: Services
var fires: Fires
var service_type := Services.Type.WELL
var service_angle := 0.0
var rig: CameraRig
var hud: Hud

var tool := Tool.PLAZA
var road_type := RoadNetwork.Type.DIRT
var speed := 1
var money := 20000
var pool := 0
var month := 2
var plaza_founded := false
var plaza_angle := 0.0
var brush := 30.0
var rng := RandomNumberGenerator.new()

var _hover := Vector2.ZERO
var _has_hover := false
var _stage := 0
var _p0 := Vector2.ZERO
var _p1 := Vector2.ZERO
var _painting := false
var _month_acc := 0.0
var _growth_acc := 0.0
var _hud_acc := 0.0
var _stage_name := "Fundación"
var _im := ImmediateMesh.new()
var _tris := PackedVector3Array()
var _cols := PackedColorArray()
var _dyn_hint := ""
var _follow := false


func _ready() -> void:
	rng.seed = 1726
	gen = TerrainGen.new()
	parcels = Parcels.new()
	day = DayNight.new()
	add_child(day)
	day.setup(self)
	day.night_materials.append(BuildingGen.material())
	terrain = Terrain.new()
	add_child(terrain)
	terrain.setup(gen, parcels)
	net = RoadNetwork.new()
	roads = RoadRenderer.new()
	add_child(roads)
	roads.setup(net, gen)
	lots = Lots.new()
	add_child(lots)
	lots.setup(net, gen, parcels)
	trees = Trees.new()
	add_child(trees)
	trees.setup(gen, _tree_blocked)
	net.changed.connect(trees.mark_dirty)
	lots.lots_changed.connect(trees.mark_dirty)
	agents = Agents.new()
	add_child(agents)
	agents.setup(net, gen, lots, day)
	services = Services.new()
	add_child(services)
	services.setup(gen, parcels, net, lots)
	agents.services = services
	fires = Fires.new()
	add_child(fires)
	fires.setup(lots, services, gen, _send_bucket_brigade)
	fires.event.connect(hud_toast)
	rig = CameraRig.new()
	add_child(rig)
	rig.gen = gen
	rig.look_at_point(Vector3(13500, gen.height(13500, 16300), 16300), 1500.0)
	var pm := StandardMaterial3D.new()
	pm.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	pm.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	pm.vertex_color_use_as_albedo = true
	pm.no_depth_test = true
	pm.cull_mode = BaseMaterial3D.CULL_DISABLED
	var pmi := MeshInstance3D.new()
	pmi.mesh = _im
	pmi.material_override = pm
	pmi.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	add_child(pmi)
	hud = Hud.new()
	add_child(hud)
	hud.setup(TOOL_DEFS)
	hud.tool_selected.connect(_select_tool)
	hud.option_selected.connect(_on_option)
	hud.speed_selected.connect(_select_speed)
	parcels.changed.connect(terrain.refresh)
	_select_road_type(RoadNetwork.Type.DIRT)
	_select_speed(1)
	_select_tool(Tool.PLAZA)
	_refresh_tool_lock()
	hud.toast("Funda la ciudad: elige el sitio de la plaza mayor en la península")
	_handle_args()


# --- Bucle ---------------------------------------------------------------------

func _process(delta: float) -> void:
	var hit = _pick(get_viewport().get_mouse_position())
	_has_hover = hit != null
	if _has_hover:
		_hover = Vector2(hit.x, hit.z)
	if _painting and _has_hover:
		lots.paint(_hover, brush, 1 if tool == Tool.ZONE else 0)
	_update_preview()
	var mult: float = SPEEDS[speed]
	day.speed = mult
	agents.speed_mult = mult
	if _follow:
		var ap = agents.position_of(agents.selected_id)
		if ap == null:
			_follow = false
		else:
			rig.position = rig.position.lerp(ap, clampf(delta * 6.0, 0.0, 1.0))
	if plaza_founded:
		_month_acc += delta * mult
		while _month_acc >= MONTH_SECONDS:
			_month_acc -= MONTH_SECONDS
			_month_tick()
		_growth_acc += delta * mult
		while _growth_acc >= 0.5:
			_growth_acc -= 0.5
			_growth_tick()
	_hud_acc += delta
	if _hud_acc > 0.2:
		_hud_acc = 0.0
		_refresh_hud()


func _month_tick() -> void:
	month += 1
	var pop := lots.population()
	var arrivals := 4 + int(pop * 0.03)
	if month % 4 == 0:
		var ship := 20 + rng.randi_range(0, 25) + int(pop * 0.02)
		arrivals += ship
		hud.toast("Llega un barco con %d pobladores" % ship)
	pool = mini(pool + arrivals, 600)
	money += int(pop * 1.2) - int(net.total_length() * 0.01) - services.upkeep()
	services.update_water()
	lots.refresh_overlay()
	if rng.randf() < 0.35:
		var w: Array = WINDS[rng.randi() % WINDS.size()]
		fires.set_wind(w[1], w[0])
	lots.age_lots()
	var unlocked := 1
	for s in PARCEL_STEPS:
		if pop >= s:
			unlocked += 1
	if unlocked > parcels.max_owned:
		parcels.max_owned = unlocked
		hud.toast("La Corona concede una nueva merced de tierras (herramienta Parcelas)")
	var stage_name: String = STAGES[0][1]
	for st in STAGES:
		if pop >= int(st[0]):
			stage_name = st[1]
	if stage_name != _stage_name:
		_stage_name = stage_name
		hud.toast("¡La fundación ya es una %s!" % stage_name.to_lower() if stage_name != "Plaza fuerte" else "¡La ciudad es ya una plaza fuerte!")


func _growth_tick() -> void:
	var used := lots.grow_step(pool, rng)
	pool -= used
	fires.step()


func hud_toast(text: String) -> void:
	hud.toast(text)


## Los vecinos salen de la fuente de agua con baldes hacia el incendio.
func _send_bucket_brigade(from: Vector2, to: Vector2, n: int) -> void:
	for k in n:
		agents.spawn(Agents.Kind.WALK, from, to, -1, "lleva baldes de agua al incendio", rng.randi() % 100000, Agents.TEMPERS[2])


func _refresh_hud() -> void:
	var pop := lots.population()
	hud.set_stats({
		"date": "%s de %d" % [MONTHS[month % 12].capitalize(), START_YEAR + month / 12],
		"money": "%s pesos" % Hud.fmt(money),
		"pop": "Población: %s" % Hud.fmt(pop),
		"pool": "Pobladores esperando: %d" % pool,
		"stage": "Etapa: %s" % _stage_name,
		"parcels": "Parcelas: %d/%d" % [parcels.owned.size(), parcels.max_owned],
		"wind": "Viento: %s%s" % [fires.wind_name, (" · %d incendios" % fires.burning.size()) if fires.burning.size() > 0 else ""],
		"clock": "%s %s · %d en las calles" % [DayNight.WEEKDAYS[day.weekday()].capitalize(), day.clock_text(), agents.active_count()],
	})
	if agents.selected_id != -1:
		hud.show_info(agents.describe(agents.selected_id))
	var base: String = TOOL_DEFS[tool]["hint"]
	hud.set_hint(base + ("\n" + _dyn_hint if _dyn_hint != "" else ""))


# --- Herramientas ------------------------------------------------------------------

func _select_tool(t: int) -> void:
	if not plaza_founded and t != Tool.PLAZA and t != Tool.INSPECT:
		hud.toast("Primero funda la plaza mayor")
		t = Tool.PLAZA
	tool = t
	_stage = 0
	_painting = false
	hud.select_tool(t)
	lots.set_show_candidates(t == Tool.ZONE or t == Tool.DEZONE)
	lots.set_overlay_mode("water" if t == Tool.WATER else "zone")
	terrain.set_overlay(t == Tool.PARCELS)
	if t == Tool.STRAIGHT or t == Tool.CURVE or t == Tool.GRID:
		hud.set_options("Tipo de calle (Tab):", ["Camino de tierra", "Calle empedrada"], road_type)
	elif t == Tool.WATER:
		var names := []
		for k in Services.INFO:
			names.append("%s (%s)" % [Services.INFO[k]["name"], Hud.fmt(Services.INFO[k]["cost"])])
		hud.set_options("Construir (Tab):", names, service_type)
	else:
		hud.set_options("", [], -1)


func _select_road_type(t: int) -> void:
	road_type = t
	hud.select_option(t)


func _on_option(i: int) -> void:
	if tool == Tool.WATER:
		service_type = i
		hud.select_option(i)
	else:
		_select_road_type(i)


func _select_speed(s: int) -> void:
	speed = s
	hud.select_speed(s)


func _refresh_tool_lock() -> void:
	for i in TOOL_DEFS.size():
		hud.set_tool_enabled(i, plaza_founded or i == Tool.PLAZA or i == Tool.INSPECT)


func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventMouseButton:
		var mb := event as InputEventMouseButton
		if mb.button_index == MOUSE_BUTTON_LEFT:
			if mb.pressed:
				_on_click()
			else:
				_painting = false
		elif mb.button_index == MOUSE_BUTTON_RIGHT and mb.pressed and _stage > 0:
			_stage = 0
	elif event is InputEventKey and event.pressed and not event.echo:
		var k := (event as InputEventKey).keycode
		if k >= KEY_1 and k <= KEY_9:
			_select_tool(k - KEY_1)
		elif k == KEY_0:
			_select_tool(Tool.WATER)
		elif k == KEY_ESCAPE:
			if _stage > 0:
				_stage = 0
			else:
				_select_tool(Tool.INSPECT)
		elif k == KEY_F and agents.selected_id != -1:
			_follow = not _follow
		elif k == KEY_TAB:
			if tool == Tool.WATER:
				_on_option((service_type + 1) % Services.INFO.size())
			else:
				_select_road_type(1 - road_type)
		elif k == KEY_SPACE:
			_select_speed(0 if speed > 0 else 1)
		elif k == KEY_R:
			plaza_angle += deg_to_rad(15.0)
			service_angle += deg_to_rad(15.0)
		elif k == KEY_BRACKETLEFT:
			brush = maxf(10.0, brush - 10.0)
		elif k == KEY_BRACKETRIGHT:
			brush = minf(150.0, brush + 10.0)


func _on_click() -> void:
	if not _has_hover:
		return
	match tool:
		Tool.INSPECT:
			var aid := agents.pick(_hover, 7.0)
			agents.selected_id = aid
			_follow = false
			if aid != -1:
				hud.show_info(agents.describe(aid))
			else:
				hud.show_info(_describe(_hover))
		Tool.PLAZA:
			found_plaza(_hover, plaza_angle)
		Tool.STRAIGHT:
			if _stage == 0:
				_p0 = _snap(_hover)
				_stage = 1
			else:
				var end := _snap(_hover)
				if build_road(RoadNetwork.line(_p0, end)):
					_p0 = end
		Tool.CURVE:
			if _stage == 0:
				_p0 = _snap(_hover)
				_stage = 1
			elif _stage == 1:
				_p1 = _snap(_hover)
				if _p1.distance_to(_p0) > 5.0:
					_stage = 2
			else:
				if build_road(_curve_points()):
					_p0 = _p1
					_stage = 1
		Tool.GRID:
			if _stage == 0:
				_p0 = _snap(_hover)
				_stage = 1
			elif _stage == 1:
				_p1 = _hover
				if _p1.distance_to(_p0) > 40.0:
					_stage = 2
			else:
				if build_grid(_grid_lines(_p0, _p1, _hover)):
					_stage = 0
		Tool.ZONE, Tool.DEZONE:
			_painting = true
		Tool.WATER:
			place_service(service_type, _hover, service_angle)
		Tool.BULLDOZE:
			var sid := services.at(_hover)
			if sid != -1:
				services.remove(sid)
				services.update_water()
				lots.refresh_overlay()
				return
			var e := net.nearest_edge(_hover, 10.0)
			if e["edge"] != -1:
				net.remove_edge(e["edge"])
				lots.on_roads_changed([])
				money -= 20
		Tool.PARCELS:
			_buy_parcel(parcels.tile_of(_hover))


func _snap(p: Vector2) -> Vector2:
	return net.snap(p)["pos"]


func _curve_points() -> PackedVector2Array:
	var ctrl := 2.0 * _hover - (_p0 + _p1) * 0.5
	return RoadNetwork.bezier(_p0, ctrl, _snap(_p1))


# --- Construcción ------------------------------------------------------------------

func road_problem(pts: PackedVector2Array, type: int) -> String:
	if pts.size() < 2 or RoadNetwork.polyline_length(pts) < 6.0:
		return "Demasiado corta"
	var r := RoadNetwork.min_radius(pts)
	var min_r: float = RoadNetwork.MIN_RADIUS[type]
	if r < min_r:
		return "Curva demasiado cerrada: radio %d m (mínimo %d m)" % [int(r), int(min_r)]
	for i in pts.size():
		var p := pts[i]
		if not parcels.is_owned_pos(p):
			return "Fuera de tus parcelas"
		var h := gen.height(p.x, p.y)
		if h < 0.5:
			return "No se puede construir sobre el agua (aún no hay puentes)"
		if i > 0:
			var dist := pts[i - 1].distance_to(p)
			var hp := gen.height(pts[i - 1].x, pts[i - 1].y)
			if dist > 0.5 and absf(h - hp) / dist > 0.2:
				return "Demasiado empinado"
	return ""


func road_cost(pts: PackedVector2Array, type: int) -> int:
	return int(RoadNetwork.polyline_length(pts) * float(RoadNetwork.COST_PER_M[type]))


func build_road(pts: PackedVector2Array, free := false) -> bool:
	var err := road_problem(pts, road_type)
	if err != "":
		hud.toast(err)
		return false
	var cost := 0 if free else road_cost(pts, road_type)
	if cost > money:
		hud.toast("Fondos insuficientes: hacen falta %s pesos" % Hud.fmt(cost))
		return false
	money -= cost
	var before := net.edges.keys()
	net.add_road(pts, road_type)
	lots.on_roads_changed(_new_edges(before))
	return true


func build_grid(lines: Array) -> bool:
	if lines.is_empty():
		return false
	var cost := 0
	for l in lines:
		var err := road_problem(l, road_type)
		if err != "":
			hud.toast("Damero: " + err)
			return false
		cost += road_cost(l, road_type)
	if cost > money:
		hud.toast("Fondos insuficientes: hacen falta %s pesos" % Hud.fmt(cost))
		return false
	money -= cost
	var before := net.edges.keys()
	for l in lines:
		net.add_road(l, road_type)
	lots.on_roads_changed(_new_edges(before))
	return true


func _new_edges(before: Array) -> Array:
	var old := {}
	for id in before:
		old[id] = true
	var out := []
	for id in net.edges:
		if not old.has(id):
			out.append(id)
	return out


func _grid_lines(a: Vector2, b: Vector2, cursor: Vector2) -> Array:
	var u := b - a
	var lu := u.length()
	if lu < 40.0:
		return []
	u /= lu
	var v := Vector2(-u.y, u.x)
	var ext := (cursor - a).dot(v)
	if ext < 0.0:
		v = -v
		ext = -ext
	var nu := maxi(1, int(round(lu / BLOCK)))
	var nv := maxi(1, int(round(ext / BLOCK)))
	var lines := []
	for j in nv + 1:
		var s := a + v * BLOCK * j
		lines.append(RoadNetwork.line(s, s + u * BLOCK * nu))
	for i in nu + 1:
		var s := a + u * BLOCK * i
		lines.append(RoadNetwork.line(s, s + v * BLOCK * nv))
	return lines


func _plaza_frame(angle: float) -> Array:
	var f := Vector2(sin(angle), cos(angle))
	return [f, Lots.side_of(f)]


func plaza_problem(c: Vector2, angle: float) -> String:
	var fr := _plaza_frame(angle)
	var corners := Lots.corners_of(c, fr[0], BLOCK + 10.0, BLOCK + 10.0)
	var hmin := INF
	var hmax := -INF
	for p in Array(corners) + [c]:
		if not parcels.is_owned_pos(p):
			return "Fuera de tus parcelas"
		var h := gen.height(p.x, p.y)
		if h < 1.0:
			return "Demasiado cerca del agua"
		hmin = minf(hmin, h)
		hmax = maxf(hmax, h)
	if hmax - hmin > 7.0:
		return "El terreno es demasiado irregular"
	return ""


func found_plaza(c: Vector2, angle: float) -> bool:
	if plaza_founded:
		hud.toast("La plaza mayor ya está fundada")
		return false
	var err := plaza_problem(c, angle)
	if err != "":
		hud.toast(err)
		return false
	var fr := _plaza_frame(angle)
	var f: Vector2 = fr[0]
	var s: Vector2 = fr[1]
	plaza_founded = true
	lots.plaza_center = c
	lots.obstacles.append({"c": c, "f": f, "w": PLAZA_SIZE, "d": PLAZA_SIZE})
	var church_c := c - f * (BLOCK * 0.5 + 4.6 + 17.0)
	lots.obstacles.append({"c": church_c, "f": f, "w": 18.0, "d": 34.0})
	# Suelo empedrado de la plaza y un rollo (columna) en el centro.
	var top := -INF
	for p in Lots.corners_of(c, f, PLAZA_SIZE, PLAZA_SIZE):
		top = maxf(top, gen.height(p.x, p.y))
	var slab := MeshInstance3D.new()
	var bm := BoxMesh.new()
	bm.size = Vector3(PLAZA_SIZE, 4.0, PLAZA_SIZE)
	slab.mesh = bm
	var sm := ShaderMaterial.new()
	sm.shader = ROAD_SHADER
	sm.set_shader_parameter("kind", 1)
	sm.set_shader_parameter("base_color", Color(0.70, 0.66, 0.58))
	slab.material_override = sm
	var basis := Basis(Vector3(s.x, 0, s.y), Vector3.UP, Vector3(f.x, 0, f.y))
	slab.transform = Transform3D(basis, Vector3(c.x, top + 0.35 - 2.0, c.y))
	add_child(slab)
	var col_st := SurfaceTool.new()
	col_st.begin(Mesh.PRIMITIVE_TRIANGLES)
	BuildingGen.box(col_st, Vector3(0, 0, 0), Vector3(3.0, 0.8, 3.0), Color(0.62, 0.6, 0.56))
	BuildingGen.box(col_st, Vector3(0, 0.8, 0), Vector3(0.9, 4.5, 0.9), Color(0.85, 0.83, 0.78))
	var column := MeshInstance3D.new()
	column.mesh = col_st.commit()
	var bmat := BuildingGen.material()
	column.material_override = bmat
	column.transform = Transform3D(basis, Vector3(c.x, top + 0.35, c.y))
	add_child(column)
	var church := MeshInstance3D.new()
	church.mesh = BuildingGen.church_mesh()
	church.material_override = bmat
	var ch := INF
	for p in Lots.corners_of(church_c, f, 16.0, 32.0):
		ch = minf(ch, gen.height(p.x, p.y))
	church.transform = Transform3D(basis, Vector3(church_c.x, ch, church_c.y))
	add_child(church)
	# Calles empedradas alrededor de la plaza.
	var half := BLOCK * 0.5
	var q := [c + (s + f) * half, c + (-s + f) * half, c + (-s - f) * half, c + (s - f) * half]
	var prev_type := road_type
	road_type = RoadNetwork.Type.STONE
	var before := net.edges.keys()
	for i in 4:
		net.add_road(RoadNetwork.line(q[i], q[(i + 1) % 4]), road_type)
	road_type = prev_type
	lots.on_roads_changed(_new_edges(before))
	pool = 40
	trees.mark_dirty()
	_refresh_tool_lock()
	_select_tool(Tool.STRAIGHT)
	hud.toast("¡Fundada la ciudad! Traza calles y zonifica solares para los pobladores")
	return true


func place_service(type: int, c: Vector2, angle: float) -> bool:
	var err := services.problem(type, c, angle)
	if err != "":
		hud.toast(err)
		return false
	var cost: int = Services.INFO[type]["cost"]
	if cost > money:
		hud.toast("Fondos insuficientes: hacen falta %s pesos" % Hud.fmt(cost))
		return false
	money -= cost
	services.place(type, c, angle)
	services.update_water()
	lots.refresh_overlay()
	trees.mark_dirty()
	return true


func _buy_parcel(t: Vector2i) -> void:
	var err := parcels.buy_problem(t)
	if err != "":
		hud.toast(err)
		return
	var price := parcels.price()
	if price > money:
		hud.toast("La merced cuesta %s pesos" % Hud.fmt(price))
		return
	money -= price
	parcels.buy(t)
	hud.toast("Merced de tierras concedida")


func _tree_blocked(p: Vector2) -> bool:
	return net.clearance(p, 12.0) < 3.0 or lots.is_occupied(p)


# --- Información ---------------------------------------------------------------------

func _describe(p: Vector2) -> String:
	var sid := services.at(p)
	if sid != -1:
		return services.describe(sid)
	var id := lots.lot_at(p)
	if id != -1:
		var lot: Dictionary = lots.lots[id]
		var lvl: int = lot["level"]
		var ft := lots.frontage_type(lot)
		var txt := "%s\n" % Lots.LEVEL_NAMES[lvl]
		txt += "Zona: %s\n" % ("mixta (vivienda, tienda y taller)" if lot["zone"] > 0 else "sin zonificar")
		txt += "Habitantes: %d\n" % Lots.CAP[lvl]
		txt += "Valor del suelo: %d%%\n" % int(lots.land_value(lot) * 100.0)
		txt += "Calle: %s\n" % (RoadNetwork.NAMES[ft] if ft >= 0 else "—")
		if lvl > 0:
			var w: float = lot["water"]
			txt += "Agua: %s\n" % ("buena" if w >= 0.85 else ("regular (pozo)" if w >= 0.5 else "sin agua cerca"))
		if fires.is_burning(id):
			txt += "¡EN LLAMAS!\n"
		elif lvl == 0 and int(lot["ruin"]) > 0:
			txt += "Ruinas de un incendio: se podrá reconstruir en %d meses\n" % lot["ruin"]
		if lot["zone"] > 0 and lvl < 3:
			txt += "\n¿Por qué no crece?\n" + _why_not(lot)
		return txt
	var e := net.nearest_edge(p, 8.0)
	if e["edge"] != -1:
		var ed: Dictionary = net.edges[e["edge"]]
		return "%s\nLargo del tramo: %d m\n%s" % [RoadNetwork.NAMES[ed["type"]], int(RoadNetwork.polyline_length(ed["pts"])), agents.lane_info(e["edge"])]
	var t := parcels.tile_of(p)
	return "Terreno\nAltura: %.1f m\nParcela %d,%d: %s" % [gen.height(p.x, p.y), t.x, t.y, "tuya" if parcels.is_owned_tile(t) else "sin adquirir"]


func _why_not(lot: Dictionary) -> String:
	var lvl: int = lot["level"]
	var need: int = Lots.CAP[lvl + 1] - Lots.CAP[lvl]
	var reasons := []
	if pool < need:
		reasons.append("· Faltan pobladores: esperan barcos con más familias.")
	var water: float = lot["water"]
	if lvl == 1 and water < 0.5:
		reasons.append("· Falta agua: construye un pozo o un aljibe cerca.")
	if lvl == 2 and water < 0.85:
		reasons.append("· Necesita buena agua: un aljibe o los aguadores.")
	if lvl == 1:
		if lot["age"] < 4:
			reasons.append("· El rancho es muy reciente.")
		if lots.land_value(lot) <= 0.2:
			reasons.append("· Está lejos de la plaza mayor.")
	elif lvl == 2:
		if lot["age"] < 8:
			reasons.append("· La casa es muy reciente.")
		if lots.land_value(lot) <= 0.45:
			reasons.append("· Para una casa de altos debe estar cerca de la plaza.")
		if lots.frontage_type(lot) != RoadNetwork.Type.STONE:
			reasons.append("· Necesita una calle empedrada.")
	if reasons.is_empty():
		return "· Nada le falta: crecerá con el tiempo."
	return "\n".join(reasons)


# --- Selección con el ratón y vista previa -------------------------------------------

## Marcha a lo largo del rayo hasta tocar el terreno (o el agua).
func _pick(screen: Vector2) -> Variant:
	var cam := rig.cam
	var o := cam.project_ray_origin(screen)
	var d := cam.project_ray_normal(screen)
	var t := 0.0
	var prev := 0.0
	for i in 500:
		var p := o + d * t
		var h := maxf(gen.height(p.x, p.z), 0.0)
		if p.y <= h:
			var lo := prev
			var hi := t
			for k in 14:
				var mid := (lo + hi) * 0.5
				var q := o + d * mid
				if q.y <= maxf(gen.height(q.x, q.z), 0.0):
					hi = mid
				else:
					lo = mid
			return o + d * hi
		prev = t
		t += maxf(0.5, (p.y - h) * 0.6)
		if t > 80000.0:
			break
	return null


func _update_preview() -> void:
	_tris.clear()
	_cols.clear()
	_dyn_hint = ""
	if _has_hover:
		match tool:
			Tool.PLAZA:
				if not plaza_founded:
					var ok := plaza_problem(_hover, plaza_angle) == ""
					var fr := _plaza_frame(plaza_angle)
					_outline(Lots.corners_of(_hover, fr[0], BLOCK + 8.0, BLOCK + 8.0), 4.0, _ok_col(ok))
					_dyn_hint = plaza_problem(_hover, plaza_angle)
			Tool.STRAIGHT:
				_marker(_snap(_hover))
				if _stage == 1:
					_road_preview(RoadNetwork.line(_p0, _snap(_hover)))
			Tool.CURVE:
				_marker(_snap(_hover))
				if _stage == 1:
					_road_preview(RoadNetwork.line(_p0, _snap(_hover)))
				elif _stage == 2:
					_road_preview(_curve_points())
			Tool.GRID:
				_marker(_snap(_hover))
				if _stage == 1:
					_ribbon(RoadNetwork.line(_p0, _hover), 3.0, Color(1, 1, 1, 0.6))
				elif _stage == 2:
					var lines := _grid_lines(_p0, _p1, _hover)
					var cost := 0
					var err := ""
					for l in lines:
						cost += road_cost(l, road_type)
						if err == "":
							err = road_problem(l, road_type)
					for l in lines:
						_ribbon(l, RoadNetwork.WIDTH[road_type], _ok_col(err == "" and cost <= money))
					_dyn_hint = "Costo: %s pesos %s" % [Hud.fmt(cost), ("· " + err) if err != "" else ""]
			Tool.ZONE, Tool.DEZONE:
				_circle(_hover, brush, Color(0.5, 0.9, 0.4, 0.8) if tool == Tool.ZONE else Color(1.0, 0.45, 0.35, 0.8))
			Tool.WATER:
				var ok := services.problem(service_type, _hover, service_angle) == ""
				_outline(services.corners(service_type, _hover, services.frame(service_angle)), 1.5, _ok_col(ok))
				_circle(_hover, Services.INFO[service_type]["radius"], Color(0.4, 0.7, 1.0, 0.7))
				for sid in services.items:
					var it: Dictionary = services.items[sid]
					_circle(it["c"], Services.INFO[it["type"]]["radius"], Color(0.4, 0.7, 1.0, 0.35))
				_dyn_hint = "%s · %s" % [Services.INFO[service_type]["desc"], services.problem(service_type, _hover, service_angle)]
			Tool.BULLDOZE:
				var bs := services.at(_hover)
				if bs != -1:
					var it: Dictionary = services.items[bs]
					_outline(services.corners(it["type"], it["c"], it["f"]), 1.5, Color(1.0, 0.3, 0.25, 0.7))
				var e := net.nearest_edge(_hover, 10.0)
				if e["edge"] != -1:
					var ed: Dictionary = net.edges[e["edge"]]
					_ribbon(ed["pts"], RoadNetwork.WIDTH[ed["type"]] + 1.0, Color(1.0, 0.3, 0.25, 0.6))
			Tool.PARCELS:
				var t := parcels.tile_of(_hover)
				terrain.set_hover_tile(t)
				var err := parcels.buy_problem(t)
				_dyn_hint = "Parcela %d,%d · Precio: %s pesos%s" % [t.x, t.y, Hud.fmt(parcels.price()), (" · " + err) if err != "" else ""]
	if agents.selected_id != -1:
		var ap = agents.position_of(agents.selected_id)
		if ap != null:
			_circle(Vector2(ap.x, ap.z), 3.5, Color(1.0, 0.85, 0.3, 0.9))
	_im.clear_surfaces()
	if _tris.is_empty():
		return
	_im.surface_begin(Mesh.PRIMITIVE_TRIANGLES)
	for i in _tris.size():
		_im.surface_set_color(_cols[i])
		_im.surface_add_vertex(_tris[i])
	_im.surface_end()


func _ok_col(ok: bool) -> Color:
	return Color(0.45, 0.95, 0.45, 0.55) if ok else Color(1.0, 0.35, 0.3, 0.55)


func _road_preview(pts: PackedVector2Array) -> void:
	var err := road_problem(pts, road_type)
	var cost := road_cost(pts, road_type)
	_ribbon(pts, RoadNetwork.WIDTH[road_type], _ok_col(err == "" and cost <= money))
	_dyn_hint = "%s · Largo: %d m · Costo: %s pesos%s" % [RoadNetwork.NAMES[road_type], int(RoadNetwork.polyline_length(pts)), Hud.fmt(cost), (" · " + err) if err != "" else ""]


func _y(p: Vector2) -> float:
	return maxf(gen.height(p.x, p.y), 0.0) + 0.9


func _ribbon(pts: PackedVector2Array, w: float, col: Color) -> void:
	for i in pts.size() - 1:
		var a := pts[i]
		var b := pts[i + 1]
		var tan := (b - a).normalized()
		var n := Vector2(-tan.y, tan.x) * (w * 0.5)
		var a0 := Vector3(a.x + n.x, _y(a), a.y + n.y)
		var a1 := Vector3(a.x - n.x, _y(a), a.y - n.y)
		var b0 := Vector3(b.x + n.x, _y(b), b.y + n.y)
		var b1 := Vector3(b.x - n.x, _y(b), b.y - n.y)
		for v in [a0, a1, b1, a0, b1, b0]:
			_tris.append(v)
			_cols.append(col)


func _outline(corners: PackedVector2Array, w: float, col: Color) -> void:
	var pts := PackedVector2Array()
	for i in corners.size() + 1:
		var a := corners[i % corners.size()]
		pts.append(a)
	var dense := PackedVector2Array()
	for i in pts.size() - 1:
		for k in 8:
			dense.append(pts[i].lerp(pts[i + 1], k / 8.0))
	dense.append(pts[pts.size() - 1])
	_ribbon(dense, w, col)


func _circle(c: Vector2, r: float, col: Color) -> void:
	var pts := PackedVector2Array()
	for i in 49:
		var a := TAU * i / 48.0
		pts.append(c + Vector2(cos(a), sin(a)) * r)
	_ribbon(pts, 2.0, col)


func _marker(p: Vector2) -> void:
	var snapped := net.snap(p)
	var col := Color(1.0, 0.85, 0.3, 0.9) if (snapped["node"] != -1 or snapped["edge"] != -1) else Color(1, 1, 1, 0.7)
	var pts := PackedVector2Array()
	for i in 13:
		var a := TAU * i / 12.0
		pts.append(p + Vector2(cos(a), sin(a)) * 3.0)
	_ribbon(pts, 1.5, col)


# --- Modo demostración y pruebas ---------------------------------------------------

## Argumentos (después de `--`): --demo, --months=N, --time=0.8, --shot=ruta.png,
## --cam=x,z,distancia,giro,inclinación, --frames=N, --quit
func _handle_args() -> void:
	var args := {}
	for a in OS.get_cmdline_user_args():
		var parts := a.trim_prefix("--").split("=", true, 1)
		args[parts[0]] = parts[1] if parts.size() > 1 else ""
	if args.has("demo"):
		build_demo()
	if args.has("months"):
		fast_forward(int(args["months"]))
	if args.has("time"):
		day.time = float(args["time"])
		day.apply()
	if args.has("fire"):
		# Prende fuego a ranchos sin agua cerca (para probar la propagación).
		var cands := []
		for lid in lots.lots:
			if lots.lots[lid]["level"] == 1 and services.fire_source(lots.lots[lid]["c"]) == -1:
				cands.append(lid)
		if not cands.is_empty():
			var first: int = cands[rng.randi() % cands.size()]
			fires.ignite(first)
			var fc: Vector2 = lots.lots[first]["c"]
			for k in int(args["fire"]) - 1:
				var near := lots._nearby(fc)
				fires.ignite(near[rng.randi() % near.size()])
			if not args.has("cam"):
				rig.look_at_point(Vector3(fc.x, gen.height(fc.x, fc.y), fc.y), 110.0)
				rig.pitch = deg_to_rad(-26.0)
	if args.has("simulate"):
		simulate(float(args["simulate"]))
	if args.has("tool"):
		_select_tool(int(args["tool"]))
	if args.has("speed"):
		_select_speed(int(args["speed"]))
	if args.has("cam"):
		var v: PackedStringArray = (args["cam"] as String).split(",")
		var x := float(v[0])
		var z := float(v[1])
		rig.look_at_point(Vector3(x, gen.height(x, z), z), float(v[2]))
		rig.yaw = deg_to_rad(float(v[3]))
		rig.pitch = deg_to_rad(float(v[4]))
	if args.has("shot") or args.has("quit"):
		var frames := int(args.get("frames", "30"))
		for i in frames:
			await get_tree().process_frame
		if args.has("shot"):
			var img := get_viewport().get_texture().get_image()
			img.save_png(args["shot"])
			print("Captura guardada: ", args["shot"])
		print_stats()
		get_tree().quit()


func print_stats() -> void:
	var water := [0, 0, 0]
	for id in lots.lots:
		var lot: Dictionary = lots.lots[id]
		if lot["level"] > 0:
			var w: float = lot["water"]
			water[2 if w >= 0.85 else (1 if w >= 0.5 else 0)] += 1
	print("Agua: buena %d · regular %d · sin agua %d · fuentes %d · incendios activos %d, apagados %d, casas quemadas %d" % [water[2], water[1], water[0], services.items.size(), fires.burning.size(), fires.put_out_total, fires.destroyed_total])
	print("Calles: %d tramos, %d nodos, %.0f m · Solares: %d (zonificados %d, edificados %d) · Población: %d · Pesos: %d · Esperando: %d" % [
		net.edges.size(), net.nodes.size(), net.total_length(), lots.lots.size(), lots.count_zoned(),
		lots.count_built(), lots.population(), money, pool])


func build_demo() -> void:
	money = 200000
	var c := Vector2(13500, 16300)
	plaza_angle = deg_to_rad(8.0)
	found_plaza(c, plaza_angle)
	var fr := _plaza_frame(plaza_angle)
	var f: Vector2 = fr[0]
	var s: Vector2 = fr[1]
	var k := 3.0
	var a := c - s * (BLOCK * (0.5 + k)) - f * (BLOCK * (0.5 + k))
	road_type = RoadNetwork.Type.STONE
	build_grid(_grid_lines(a, a + s * BLOCK * (2 * k + 1), a + f * BLOCK * (2 * k + 1)))
	road_type = RoadNetwork.Type.DIRT
	var exit := c - f * (BLOCK * (0.5 + k))
	build_road(RoadNetwork.bezier(exit, exit - f * 350.0 + s * 250.0, exit - f * 700.0 - s * 50.0))
	# Agua: aljibes cerca de la plaza y pozos en los barrios.
	for i in range(-3, 4, 2):
		for j in range(-3, 4, 2):
			var corner := c + s * (BLOCK * (i * 0.5)) + f * (BLOCK * (j * 0.5)) + (s * signf(i) + f * signf(j)) * 13.0
			place_service(Services.Type.CISTERN, corner, plaza_angle)
			place_service(Services.Type.WELL, corner + s * (BLOCK * signf(i)) - (s * signf(i)) * 18.0, plaza_angle)
	lots.paint(c, 420.0, 1)
	lots.paint(exit - f * 400.0, 200.0, 1)
	_select_tool(Tool.INSPECT)


## Avanza el día y los agentes `seconds` segundos reales (para pruebas y capturas).
func simulate(seconds: float) -> void:
	var dt := 0.1
	var t0 := Time.get_ticks_usec()
	var n := int(seconds / dt)
	for i in n:
		day.advance(dt / day.day_length)
		agents.tick(dt)
		if i % 5 == 4:
			fires.step()
	print("Costo medio por paso de 0,1 s: %.2f ms" % ((Time.get_ticks_usec() - t0) / 1000.0 / maxf(n, 1)))
	day.apply()
	agents._render()
	var stuck := 0
	var replans := 0
	for ag in agents.agents:
		if float(ag["wait"]) > 120.0:
			stuck += 1
	for id in agents.residents:
		replans += int(agents.residents[id]["replans"])
	print("Agentes: %d en las calles · viajes completados %d · omitidos %d · atascados >2 min: %d · adelantamientos: %d · cambios de ruta aprendidos: %d" % [agents.active_count(), agents.trips_done, agents.trips_skipped, stuck, agents.overtakes, replans])


func fast_forward(months: int) -> void:
	services.update_water()
	for m in months:
		_month_tick()
		for g in int(MONTH_SECONDS / 0.5):
			_growth_tick()
