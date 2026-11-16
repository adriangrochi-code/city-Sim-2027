extends Node3D
## El puerto colonial: muelle con barraca, fondeadero, barcos de vela y lanchas.
## Es la "válvula" entre la ciudad y el mundo: por aquí llegan pobladores y mercancías y
## salen los cueros. Los acontecimientos lejanos (ficticios, con guiños a la historia real)
## cambian esos flujos y la ciudad tiene que adaptarse.

const TerrainGen = preload("res://scripts/world/terrain_gen.gd")
const Parcels = preload("res://scripts/world/parcels.gd")
const RoadNetwork = preload("res://scripts/roads/road_network.gd")
const Lots = preload("res://scripts/city/lots.gd")
const BuildingGen = preload("res://scripts/city/building_gen.gd")

signal event(text: String)
signal sick_ship(people: int, landing: Vector2)

const PIER_COST := 4000
const PIER_UPKEEP := 40
const DUTY_PER_HIDE := 3
const DUTY_PER_SHIP := 180
const SHIP_SPEED := 9.0
const BOAT_SPEED := 4.0
const UNLOAD_TRIPS := 3          # viajes de lancha para descargar un barco
const SHIP_FACTOR := 20.0        # compresión de tiempo visual de barcos
const BOAT_FACTOR := 12.0

## Acontecimientos lejanos de la época colonial. Todos ficticios.
const EVENTS := [
	{"name": "Guerra entre las Coronas del Norte", "months": 8, "ships": 0.5, "price": 1.0, "people": 0.7,
		"news": "Gaceta: estalla la guerra entre las Coronas del Norte. Llegarán menos barcos y florecerá el contrabando."},
	{"name": "Reglamento de Comercio Libre", "months": 12, "ships": 1.8, "price": 1.1, "people": 1.2,
		"news": "Gaceta: la Corona publica el Reglamento de Comercio Libre. Se esperan muchos más barcos."},
	{"name": "Fiebre del cuero en ultramar", "months": 10, "ships": 1.2, "price": 1.8, "people": 1.0,
		"news": "Gaceta: en ultramar pagan los cueros a precio de oro. Las estancias se apuran a carnear."},
	{"name": "Barco de pobladores de las Islas", "months": 1, "ships": 1.0, "price": 1.0, "people": 3.0,
		"news": "Gaceta: zarpa un barco con muchas familias de las Islas Atlánticas rumbo a la ciudad."},
	{"name": "Bloqueo del río", "months": 4, "ships": 0.0, "price": 1.0, "people": 0.0,
		"news": "Gaceta: una escuadra extranjera bloquea el río. No entrarán ni saldrán barcos por un tiempo."},
	{"name": "Fiebre en los barcos", "months": 3, "ships": 0.8, "price": 1.0, "people": 0.6,
		"news": "Gaceta: llegan noticias de fiebres en los barcos. Los recién llegados harán cuarentena."},
]

## Ampliaciones del puerto: se crece por piezas, sin reconstruir.
const MODULES := [
	{"id": "alargar", "name": "Alargar el muelle", "cost": 2500, "boats": 1},
	{"id": "grua", "name": "Grúa de madera para la carga", "cost": 1800, "boats": 1, "needs": "alargar"},
	{"id": "barraca2", "name": "Segunda barraca de cueros", "cost": 2200, "duty": 0.25},
]

var modules: Array = []
# Efectos de las ordenanzas (los fija main cada mes).
var policy_ship := 1.0
var policy_duty := 1.0
var policy_no_export := false
var gen: TerrainGen
var parcels: Parcels
var net: RoadNetwork
var lots: Lots
var has_pier := false
var land_point := Vector2.ZERO     # donde se cargan y descargan las carretas (barraca)
var pier_end := Vector2.ZERO
var anchorage := Vector2.ZERO
var sea_dir := Vector2.ZERO
var hides := 0                      # cueros acopiados en la barraca
var total_exported := 0
var active_event: Dictionary = {}
var event_months := 0
var year_stats := {"ships": 0, "people": 0, "exported": 0, "imported": 0, "duty": 0}
var _ship_timer := 2.0              # meses hasta el próximo barco
var _ships: Array = []              # {"node", "pos", "state", "t", "people", "trips"}
var _boats: Array = []              # {"node", "t", "dir"}
var _rng := RandomNumberGenerator.new()
var _pending_people := 0
var _pending_duty := 0


func setup(g: TerrainGen, p: Parcels, n: RoadNetwork, l: Lots) -> void:
	gen = g
	parcels = p
	net = n
	lots = l
	_rng.seed = 1724


# --- Muelle ------------------------------------------------------------------------

## Busca hacia dónde está el agua profunda desde `p`. Devuelve [dirección, largo] o [].
func _find_sea(p: Vector2) -> Array:
	var best := []
	for k in 32:
		var a := TAU * k / 32.0
		var d := Vector2(cos(a), sin(a))
		for dist in range(10, 170, 5):
			var q := p + d * dist
			if gen.height(q.x, q.y) < -1.6:
				if best.is_empty() or dist < int(best[1]):
					best = [d, dist]
				break
	return best


func pier_problem(p: Vector2) -> String:
	if has_pier:
		return "La ciudad ya tiene su muelle"
	if not parcels.is_owned_pos(p):
		return "Fuera de tus parcelas"
	if gen.height(p.x, p.y) < 0.5:
		return "El muelle arranca en tierra firme, junto a la costa"
	if net.clearance(p, 50.0) > 35.0:
		return "Debe estar junto a una calle (para las carretas)"
	if _find_sea(p).is_empty():
		return "No hay agua profunda cerca: elige un punto de la costa"
	return ""


func build_pier(p: Vector2) -> bool:
	if pier_problem(p) != "":
		return false
	var sea := _find_sea(p)
	sea_dir = sea[0]
	var length := clampf(float(sea[1]) + 25.0, 40.0, 190.0)
	pier_end = p + sea_dir * length
	anchorage = pier_end + sea_dir * 220.0
	land_point = p
	has_pier = true
	var s := Vector2(sea_dir.y, -sea_dir.x)
	# Barraca de cueros en tierra, mirando al muelle.
	var barraca_c := p - sea_dir * 16.0
	lots.clear_area(Lots.corners_of(barraca_c, sea_dir, 18.0, 14.0))
	lots.obstacles.append({"c": barraca_c, "f": sea_dir, "w": 18.0, "d": 14.0})
	var st := SurfaceTool.new()
	st.begin(Mesh.PRIMITIVE_TRIANGLES)
	var wood := Color(0.42, 0.31, 0.21)
	var brick := Color(0.66, 0.40, 0.30)
	BuildingGen.box(st, Vector3(0, -1.0, 0), Vector3(18.0, 6.0, 12.0), brick)
	BuildingGen.gable(st, Vector3(0, 5.0, 0), 9.6, 6.6, 2.6, BuildingGen.TILE_ROOF, brick, 9.0, 6.0)
	BuildingGen.box(st, Vector3(0, 0, 6.05), Vector3(4.0, 3.6, 0.12), wood)
	for k in 5:
		BuildingGen.box(st, Vector3(-6.0 + k * 3.0, 0, 8.5), Vector3(1.6, 0.9, 2.2), Color(0.55, 0.42, 0.28))
	var barraca := MeshInstance3D.new()
	barraca.mesh = st.commit()
	barraca.material_override = BuildingGen.material()
	barraca.transform = Transform3D(Basis(Vector3(s.x, 0, s.y), Vector3.UP, Vector3(sea_dir.x, 0, sea_dir.y)), Vector3(barraca_c.x, gen.height(barraca_c.x, barraca_c.y), barraca_c.y))
	add_child(barraca)
	# Muelle de madera sobre pilotes.
	var st2 := SurfaceTool.new()
	st2.begin(Mesh.PRIMITIVE_TRIANGLES)
	BuildingGen.box(st2, Vector3(0, 1.6, length * 0.5), Vector3(6.0, 0.35, length), wood)
	var z := 4.0
	while z < length:
		for x in [-2.6, 2.6]:
			BuildingGen.box(st2, Vector3(x, -6.0, z), Vector3(0.4, 7.7, 0.4), wood.darkened(0.35))
		z += 8.0
	BuildingGen.box(st2, Vector3(0, 1.95, length - 3.0), Vector3(1.2, 1.2, 1.2), Color(0.5, 0.38, 0.25))
	var pier := MeshInstance3D.new()
	pier.mesh = st2.commit()
	pier.material_override = BuildingGen.material()
	pier.transform = Transform3D(Basis(Vector3(s.x, 0, s.y), Vector3.UP, Vector3(sea_dir.x, 0, sea_dir.y)), Vector3(p.x, 0.0, p.y))
	add_child(pier)
	for k in 2:
		var boat := _make_boat()
		add_child(boat)
		_boats.append({"node": boat, "t": float(k) * 0.5, "dir": 1.0, "busy": false})
	return true


# --- Mes a mes ---------------------------------------------------------------------

## ¿Este barco trae fiebre? Más probable durante "Fiebre en los barcos".
func is_sick() -> bool:
	var p := 0.06
	if not active_event.is_empty() and String(active_event["name"]).begins_with("Fiebre en los barcos"):
		p = 0.5
	return _rng.randf() < p


func mult(key: String) -> float:
	return float(active_event.get(key, 1.0)) if not active_event.is_empty() else 1.0


## Devuelve los pobladores que llegaron "por la playa" (sin muelle) este mes.
func month_tick(pop: int) -> int:
	if month_count() % 12 == 0:
		year_stats = {"ships": 0, "people": 0, "exported": 0, "imported": 0, "duty": 0}
	_months += 1
	# Acontecimientos lejanos.
	if not active_event.is_empty():
		event_months -= 1
		if event_months <= 0:
			event.emit("Gaceta: termina «%s»." % active_event["name"])
			active_event = {}
	elif _rng.randf() < 0.07:
		active_event = EVENTS[_rng.randi() % EVENTS.size()]
		event_months = int(active_event["months"])
		event.emit(active_event["news"])
	# Barcos.
	var beach_people := 0
	_ship_timer -= mult("ships") * policy_ship
	if _ship_timer <= 0.0 and mult("ships") > 0.0:
		_ship_timer = 3.0 + _rng.randf() * 1.5
		var people := int((20 + _rng.randi_range(0, 25) + pop * 0.02) * mult("people"))
		year_stats["ships"] += 1
		if has_pier:
			_spawn_ship(people)
		else:
			# Sin muelle desembarcan como pueden en la playa: llegan menos.
			beach_people = int(people * 0.6)
			year_stats["people"] += beach_people
			if is_sick():
				sick_ship.emit(beach_people, lots.plaza_center)
				beach_people = 0
			event.emit("Un barco desembarca %d pobladores en la playa (no hay muelle)" % beach_people)
	return beach_people


var _months := 0
func month_count() -> int:
	return _months


## Una carreta descargó cueros en la barraca.
func deliver_hides(n: int) -> void:
	hides += n


## Pobladores y aduana acumulados desde la última consulta (los cobra main).
func take_arrivals() -> Array:
	var out := [_pending_people, _pending_duty]
	_pending_people = 0
	_pending_duty = 0
	return out


func actions(money: int) -> Array:
	var out := []
	if not has_pier:
		return out
	for m in MODULES:
		if modules.has(m["id"]):
			continue
		var needs: String = m.get("needs", "")
		if needs != "" and not modules.has(needs):
			continue
		out.append(["Ampliar: %s (%d pesos)" % [m["name"], m["cost"]], money >= int(m["cost"]), "port_module", m["id"]])
	return out


func module_cost(mid: String) -> int:
	for m in MODULES:
		if m["id"] == mid:
			return m["cost"]
	return 0


## Agrega una ampliación: más lanchas (descarga más rápida) o más aduana, y se ve en el puerto.
func add_module(mid: String) -> void:
	modules.append(mid)
	var s := Vector2(sea_dir.y, -sea_dir.x)
	var basis := Basis(Vector3(s.x, 0, s.y), Vector3.UP, Vector3(sea_dir.x, 0, sea_dir.y))
	var st := SurfaceTool.new()
	st.begin(Mesh.PRIMITIVE_TRIANGLES)
	var wood := Color(0.42, 0.31, 0.21)
	var origin := Vector3(pier_end.x, 0.0, pier_end.y)
	match mid:
		"alargar":
			BuildingGen.box(st, Vector3(0, 1.6, 15.0), Vector3(6.0, 0.35, 30.0), wood)
			for z in [4.0, 12.0, 20.0, 28.0]:
				for x in [-2.6, 2.6]:
					BuildingGen.box(st, Vector3(x, -6.0, z), Vector3(0.4, 7.7, 0.4), wood.darkened(0.35))
			pier_end += sea_dir * 30.0
		"grua":
			BuildingGen.box(st, Vector3(2.0, 1.9, -4.0), Vector3(0.6, 9.0, 0.6), wood)
			BuildingGen.box(st, Vector3(2.0, 10.6, -1.0), Vector3(0.4, 0.4, 7.0), wood)
			BuildingGen.box(st, Vector3(2.0, 7.5, 2.4), Vector3(0.1, 3.0, 0.1), Color(0.2, 0.2, 0.2))
		"barraca2":
			var brick := Color(0.66, 0.40, 0.30)
			origin = Vector3(land_point.x, 0.0, land_point.y) - Vector3(sea_dir.x, 0, sea_dir.y) * 16.0 + Vector3(s.x, 0, s.y) * 22.0
			lots.clear_area(Lots.corners_of(Vector2(origin.x, origin.z), sea_dir, 18.0, 14.0))
			lots.obstacles.append({"c": Vector2(origin.x, origin.z), "f": sea_dir, "w": 18.0, "d": 14.0})
			origin.y = gen.height(origin.x, origin.z)
			BuildingGen.box(st, Vector3(0, -1.0, 0), Vector3(16.0, 6.0, 12.0), brick)
			BuildingGen.gable(st, Vector3(0, 5.0, 0), 8.6, 6.6, 2.6, BuildingGen.TILE_ROOF, brick, 8.0, 6.0)
			BuildingGen.box(st, Vector3(0, 0, 6.05), Vector3(4.0, 3.6, 0.12), wood)
	var mi := MeshInstance3D.new()
	mi.mesh = st.commit()
	mi.material_override = BuildingGen.material()
	mi.transform = Transform3D(basis, origin)
	add_child(mi)
	for m in MODULES:
		if m["id"] == mid and int(m.get("boats", 0)) > 0:
			var boat := _make_boat()
			add_child(boat)
			_boats.append({"node": boat, "t": 0.0, "dir": 1.0, "busy": false})
	event.emit("Puerto ampliado: %s" % mid_name(mid).to_lower())


func mid_name(mid: String) -> String:
	for m in MODULES:
		if m["id"] == mid:
			return m["name"]
	return mid


func describe() -> String:
	var txt := "Puerto\n"
	if not has_pier:
		txt += "Sin muelle: los barcos desembarcan en la playa y llega menos gente.\nConstruye el muelle en la costa, junto a una calle.\n"
	else:
		txt += "Cueros en la barraca: %d\nBarcos en el fondeadero: %d · lanchas: %d\n" % [hides, _ships.size(), _boats.size()]
		if not modules.is_empty():
			var names := []
			for mid in modules:
				names.append(mid_name(mid))
			txt += "Ampliaciones: %s\n" % ", ".join(names)
	txt += "\nEste año:\n· Barcos: %d\n· Pobladores llegados: %d\n· Cueros exportados: %d\n· Fardos de mercancía importados: %d\n· Aduana: %d pesos\n" % [
		year_stats["ships"], year_stats["people"], year_stats["exported"], year_stats["imported"], year_stats["duty"]]
	if not active_event.is_empty():
		txt += "\nAhora: %s (%d meses más)\n" % [active_event["name"], event_months]
		txt += "Barcos ×%.1f · Precio del cuero ×%.1f · Pobladores ×%.1f" % [mult("ships"), mult("price"), mult("people")]
	return txt


# --- Barcos y lanchas ------------------------------------------------------------

func _spawn_ship(people: int) -> void:
	var node := _make_ship()
	add_child(node)
	var start := anchorage + sea_dir * 1500.0
	_ships.append({"node": node, "pos": start, "state": "llegando", "t": 0.0, "people": people, "trips": 0})
	event.emit("Avistan un barco en el río: trae %d pobladores y mercancías" % people)


func _process(delta: float) -> void:
	_tick(delta * time_scale)


var time_scale := 1.0


func _tick(dt: float) -> void:
	if not has_pier:
		return
	var unloading := -1
	for i in range(_ships.size() - 1, -1, -1):
		var sh: Dictionary = _ships[i]
		var pos: Vector2 = sh["pos"]
		match sh["state"]:
			"llegando":
				var to := anchorage - pos
				var step := SHIP_SPEED * dt * SHIP_FACTOR
				if to.length() <= step:
					sh["pos"] = anchorage
					sh["state"] = "descargando"
				else:
					sh["pos"] = pos + to.normalized() * step
			"descargando":
				unloading = i
				if int(sh["trips"]) >= UNLOAD_TRIPS:
					_finish_unloading(sh)
					sh["state"] = "zarpando"
			"zarpando":
				sh["pos"] = pos + sea_dir * SHIP_SPEED * dt * SHIP_FACTOR
				if (sh["pos"] as Vector2).distance_to(anchorage) > 1700.0:
					(sh["node"] as Node).queue_free()
					_ships.remove_at(i)
					continue
		var node: Node3D = sh["node"]
		var heading := -sea_dir if sh["state"] == "llegando" else sea_dir
		node.position = Vector3(sh["pos"].x, 0.0, sh["pos"].y)
		node.basis = Basis.looking_at(-Vector3(heading.x, 0, heading.y), Vector3.UP)
		node.position.y = sin(Time.get_ticks_msec() * 0.001 + i) * 0.15
	# Lanchas: van y vienen entre el barco y el muelle mientras hay algo que descargar.
	for b in _boats:
		var node: Node3D = b["node"]
		if unloading == -1:
			b["t"] = maxf(0.0, float(b["t"]) - dt * 0.05)
		else:
			b["t"] = float(b["t"]) + float(b["dir"]) * dt * BOAT_SPEED * BOAT_FACTOR / maxf(anchorage.distance_to(pier_end), 1.0)
			if float(b["t"]) >= 1.0:
				b["t"] = 1.0
				b["dir"] = -1.0
			elif float(b["t"]) <= 0.0:
				b["t"] = 0.0
				b["dir"] = 1.0
				_ships[unloading]["trips"] = int(_ships[unloading]["trips"]) + 1
		var bi := _boats.find(b)
		var side := Vector2(sea_dir.y, -sea_dir.x) * ((float(bi) - (_boats.size() - 1) * 0.5) * 7.0)
		var p := pier_end.lerp(anchorage - sea_dir * 25.0, float(b["t"])) + side
		node.position = Vector3(p.x, 0.1, p.y)
		var dir := sea_dir * float(b["dir"])
		node.basis = Basis.looking_at(-Vector3(dir.x, 0, dir.y), Vector3.UP)


## Completa al instante los barcos en camino (para el avance rápido de pruebas).
func flush_ships() -> void:
	for sh in _ships:
		if sh["state"] != "zarpando":
			_finish_unloading(sh)
		(sh["node"] as Node).queue_free()
	_ships.clear()


func _finish_unloading(sh: Dictionary) -> void:
	var people: int = sh["people"]
	var bales := 20 + _rng.randi_range(0, 30)
	var exported := 0 if policy_no_export else mini(hides, 400)
	hides -= exported
	var duty := int((DUTY_PER_SHIP + exported * DUTY_PER_HIDE * mult("price")) * (1.0 + (0.25 if modules.has("barraca2") else 0.0)) * policy_duty)
	year_stats["people"] += people
	year_stats["imported"] += bales
	year_stats["exported"] += exported
	total_exported += exported
	year_stats["duty"] += duty
	if is_sick():
		sick_ship.emit(people, land_point)
	else:
		_pending_people += people
	_pending_duty += duty
	event.emit("Desembarcan %d pobladores · se embarcan %d cueros · aduana: %d pesos" % [people, exported, duty])


func _make_ship() -> Node3D:
	var st := SurfaceTool.new()
	st.begin(Mesh.PRIMITIVE_TRIANGLES)
	var hull := Color(0.30, 0.22, 0.15)
	var sail := Color(0.92, 0.89, 0.80)
	BuildingGen.box(st, Vector3(0, -1.5, 0), Vector3(8.0, 4.0, 30.0), hull)
	BuildingGen.box(st, Vector3(0, -1.5, 16.5), Vector3(5.0, 3.6, 3.0), hull)
	BuildingGen.box(st, Vector3(0, 2.5, -12.0), Vector3(7.5, 2.4, 6.0), hull.lightened(0.1))
	for z in [-8.0, 2.0, 11.0]:
		BuildingGen.box(st, Vector3(0, 2.5, z), Vector3(0.5, 22.0, 0.5), hull)
		BuildingGen.box(st, Vector3(0, 8.0, z + 0.4), Vector3(11.0, 6.5, 0.2), sail)
		BuildingGen.box(st, Vector3(0, 15.5, z + 0.4), Vector3(8.5, 5.0, 0.2), sail)
	var mi := MeshInstance3D.new()
	mi.mesh = st.commit()
	mi.material_override = BuildingGen.material()
	var root := Node3D.new()
	root.add_child(mi)
	return root


func _make_boat() -> Node3D:
	var st := SurfaceTool.new()
	st.begin(Mesh.PRIMITIVE_TRIANGLES)
	BuildingGen.box(st, Vector3(0, -0.4, 0), Vector3(2.6, 1.0, 7.0), Color(0.45, 0.33, 0.22))
	BuildingGen.box(st, Vector3(0, 0.6, -0.5), Vector3(1.8, 0.7, 2.5), Color(0.62, 0.52, 0.38))
	for z in [1.5, 2.6]:
		BuildingGen.box(st, Vector3(0, 0.6, z), Vector3(0.45, 1.2, 0.35), Color(0.6, 0.35, 0.25))
	var mi := MeshInstance3D.new()
	mi.mesh = st.commit()
	mi.material_override = BuildingGen.material()
	var root := Node3D.new()
	root.add_child(mi)
	return root
