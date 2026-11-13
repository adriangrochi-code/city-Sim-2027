extends Node3D
## Vecinos y vehículos de la época colonial: gente a pie, jinetes y carretas de bueyes.
##
## Cada vecino tiene casa, trabajo, rutina y temperamento. Los vehículos conducen con un
## modelo de seguimiento (IDM): guardan distancia, aceleran y frenan con suavidad, aflojan
## en las curvas y antes de doblar, ceden el paso en los cruces por orden de llegada, no
## entran a un cruce si no tienen salida libre, y los jinetes adelantan a las carretas
## solo cuando ven el carril contrario despejado. Las rutas se eligen por tiempo medido y
## los vecinos aprenden: si un trayecto les sale mal, al día siguiente prueban otro.

const RoadNetwork = preload("res://scripts/roads/road_network.gd")
const TerrainGen = preload("res://scripts/world/terrain_gen.gd")
const Lots = preload("res://scripts/city/lots.gd")
const DayNight = preload("res://scripts/world/day_night.gd")
const Router = preload("res://scripts/agents/router.gd")
const AgentMeshes = preload("res://scripts/agents/agent_meshes.gd")

enum Kind { WALK, RIDER, CART }
const KIND_NAMES := ["a pie", "a caballo", "en carreta de bueyes"]
const TIME_SCALE := 10.0      # los agentes se mueven 10 veces más rápido que el tiempo real
const MAX_AGENTS := 1200
const STEP := 0.1             # paso fijo de la simulación de movimiento (segundos simulados)
const APPROACH := 22.0        # distancia a la que se pide paso en un cruce
const PARAMS := {
	Kind.WALK: {"v0": 1.35, "a": 1.0, "b": 1.5, "T": 0.8, "s0": 0.8, "len": 0.6},
	Kind.RIDER: {"v0": 3.8, "a": 1.3, "b": 2.2, "T": 1.3, "s0": 2.0, "len": 2.6},
	Kind.CART: {"v0": 1.15, "a": 0.25, "b": 0.6, "T": 2.4, "s0": 3.0, "len": 7.5},
}
const TEMPERS := [
	{"name": "prudente", "v0": 0.9, "T": 1.35, "overtake": 0.0},
	{"name": "tranquilo", "v0": 1.0, "T": 1.0, "overtake": 0.5},
	{"name": "apurado", "v0": 1.12, "T": 0.75, "overtake": 1.0},
]
const FIRST := ["Juan", "José", "Francisco", "Manuel", "Pedro", "Domingo", "Bartolomé", "Tomás", "Ignacio", "Antonio", "Luis", "Miguel", "María", "Josefa", "Antonia", "Ana", "Isabel", "Catalina", "Rosa", "Juana", "Petrona", "Manuela", "Teresa", "Francisca"]
const LAST := ["Pérez", "Gómez", "Fernández", "Rodríguez", "Martínez", "Suárez", "Acosta", "Medina", "Correa", "Sosa", "Benítez", "Cabrera", "Díaz", "Ferreira", "Pereira", "Machado", "Hernández", "Morales", "Silva", "Viera", "Larrañaga", "Durán"]
const ORIGINS := ["criollo de la ciudad", "llegado de las Islas Atlánticas", "llegado de la Península", "llegado del interior"]
const ROLES := ["trabaja fuera de casa", "trabaja en su casa-taller", "no trabaja (niño o anciano)"]
const JOBS := ["artesano", "pulpero", "jornalero", "carpintero", "herrero", "panadero", "lavandera", "costurera", "peón", "albañil"]
const WALKER_COLORS := [Color(0.85, 0.82, 0.74), Color(0.55, 0.32, 0.25), Color(0.35, 0.40, 0.52), Color(0.62, 0.55, 0.38), Color(0.30, 0.30, 0.30), Color(0.70, 0.45, 0.35), Color(0.45, 0.52, 0.40)]
const HORSE_COLORS := [Color(0.42, 0.27, 0.16), Color(0.18, 0.13, 0.10), Color(0.62, 0.45, 0.28), Color(0.80, 0.78, 0.72), Color(0.30, 0.22, 0.15)]

var net: RoadNetwork
var gen: TerrainGen
var lots: Lots
var day: DayNight
var router := Router.new()
var speed_mult := 1.0
var residents := {}
var agents: Array = []
var by_id := {}
var node_occ := {}            # cruce -> agente que tiene el paso
var selected_id := -1
var trips_done := 0
var trips_skipped := 0
var overtakes := 0
var sim_time := 0.0

var _by_lot := {}
var _next_res := 1
var _next_agent := 1
var _schedule: Array = []
var _sched_i := 0
var _sched_day := -1
var _pending: Array = []      # viajes de vuelta programados [tiempo_sim, Callable]
var _acc := 0.0
var _rng := RandomNumberGenerator.new()
var _mm := {}
var _occ_wait0 := {}


func setup(n: RoadNetwork, g: TerrainGen, l: Lots, d: DayNight) -> void:
	net = n
	gen = g
	lots = l
	day = d
	_rng.seed = 2027
	router.rebuild(net)
	net.changed.connect(_on_roads_changed)
	lots.lot_level_changed.connect(_on_lot_level)
	_mm["walk"] = _make_mm(AgentMeshes.walker())
	_mm["horse"] = _make_mm(AgentMeshes.horse())
	_mm["rider"] = _make_mm(AgentMeshes.rider())
	_mm["cart"] = _make_mm(AgentMeshes.cart())


func _make_mm(mesh: Mesh) -> MultiMesh:
	var mm := MultiMesh.new()
	mm.transform_format = MultiMesh.TRANSFORM_3D
	mm.use_colors = true
	mm.mesh = mesh
	mm.instance_count = MAX_AGENTS
	mm.visible_instance_count = 0
	var mmi := MultiMeshInstance3D.new()
	mmi.multimesh = mm
	add_child(mmi)
	return mm


func _process(delta: float) -> void:
	tick(delta * speed_mult)
	_render()


## Avanza la simulación de agentes `real_dt` segundos reales (ya escalados por la velocidad).
func tick(real_dt: float) -> void:
	_spawn_scheduled()
	_acc += real_dt * TIME_SCALE
	var steps := 0
	while _acc >= STEP and steps < 40:
		_acc -= STEP
		steps += 1
		_step(STEP)
	if steps == 40:
		_acc = 0.0


# --- Vecinos -----------------------------------------------------------------------

func _on_lot_level(lot_id: int, old_level: int, new_level: int) -> void:
	var want: int = Lots.CAP[new_level]
	var have: Array = _by_lot.get(lot_id, [])
	while have.size() > want:
		residents.erase(have.pop_back())
	while have.size() < want:
		have.append(_new_resident(lot_id, new_level))
	_by_lot[lot_id] = have


func _new_resident(lot_id: int, level: int) -> int:
	var id := _next_res
	_next_res += 1
	var rich := level >= 3
	var kind := Kind.RIDER if _rng.randf() < (0.4 if rich else 0.08) else Kind.WALK
	var temper := _rng.randi() % TEMPERS.size()
	residents[id] = {
		"id": id,
		"name": "%s %s" % [FIRST[_rng.randi() % FIRST.size()], LAST[_rng.randi() % LAST.size()]],
		"origin": ORIGINS[_rng.randi() % ORIGINS.size()],
		"job": JOBS[_rng.randi() % JOBS.size()],
		# En la ciudad colonial la mayoría trabajaba en su propia casa (taller, tienda).
		"role": 0 if _rng.randf() < 0.3 else (1 if _rng.randf() < 0.55 else 2),
		"home": lot_id,
		"work": -1,
		"kind": kind,
		"temper": temper,
		"am": clampf(6.4 + _rng.randfn(0.0, 0.7), 5.0, 9.0),
		"pm": clampf(17.2 + _rng.randfn(0.0, 0.7), 15.5, 19.5),
		"route_seed": _rng.randi() % 100000,
		"last_trip": "",
		"replans": 0,
	}
	return id


func _work_lot(r: Dictionary) -> int:
	var w: int = r["work"]
	if w != -1 and lots.lots.has(w) and lots.lots[w]["level"] > 0:
		return w
	# Busca un trabajo: un taller o tienda en otro solar edificado.
	var keys := _by_lot.keys()
	for k in 6:
		if keys.is_empty():
			break
		var cand: int = keys[_rng.randi() % keys.size()]
		if cand != r["home"] and lots.lots.has(cand):
			r["work"] = cand
			return cand
	r["work"] = -1
	return -1


# --- Rutinas diarias -----------------------------------------------------------------

func _build_schedule() -> void:
	_schedule.clear()
	_sched_i = 0
	_sched_day = day.day_index
	var sunday := day.weekday() == 6
	for id in residents:
		var r: Dictionary = residents[id]
		# Pequeña variación diaria sobre su horario habitual.
		var jit := _rng.randfn(0.0, 0.12)
		if sunday:
			if _rng.randf() < 0.6:
				var go := 9.2 + _rng.randfn(0.0, 0.35)
				_schedule.append([go / 24.0, id, "a misa en la iglesia matriz"])
				_schedule.append([(go + 1.8 + _rng.randf() * 0.6) / 24.0, id, "de vuelta a casa"])
		else:
			if r["role"] == 0:
				_schedule.append([(float(r["am"]) + jit) / 24.0, id, "al trabajo"])
				_schedule.append([(float(r["pm"]) + jit) / 24.0, id, "de vuelta a casa"])
			if _rng.randf() < (0.12 if r["role"] == 0 else 0.2):
				var m := 10.5 + _rng.randf() * 1.5
				_schedule.append([m / 24.0, id, "al mercado de la plaza"])
				_schedule.append([(m + 0.8 + _rng.randf() * 0.7) / 24.0, id, "de vuelta del mercado"])
	# Carretas que traen cueros y alimentos del campo durante el día.
	var carts := 4 + int(lots.population() / 70.0)
	for k in carts:
		_schedule.append([(6.5 + _rng.randf() * 9.0) / 24.0, -1, "trae cueros y trigo del campo"])
	_schedule.sort_custom(func(a, b): return a[0] < b[0])
	# Si el día ya empezó (al cargar o saltar el tiempo), no se lanzan de golpe los viajes pasados.
	while _sched_i < _schedule.size() and float(_schedule[_sched_i][0]) < day.time - 0.01:
		_sched_i += 1


func _spawn_scheduled() -> void:
	if day == null:
		return
	if _sched_day != day.day_index:
		_build_schedule()
	var now := day.time
	while _sched_i < _schedule.size() and float(_schedule[_sched_i][0]) <= now:
		var item: Array = _schedule[_sched_i]
		_sched_i += 1
		if item[1] == -1:
			_spawn_cart()
		elif residents.has(item[1]):
			_spawn_resident_trip(residents[item[1]], item[2])
	for i in range(_pending.size() - 1, -1, -1):
		if float(_pending[i][0]) <= sim_time:
			var cb: Callable = _pending[i][1]
			_pending.remove_at(i)
			cb.call()


func _plaza_point() -> Vector2:
	return lots.plaza_center


func _spawn_resident_trip(r: Dictionary, purpose: String) -> void:
	if not lots.lots.has(r["home"]):
		return
	var home: Vector2 = lots.front_point(lots.lots[r["home"]])
	var a: Vector2
	var b: Vector2
	if purpose == "al trabajo":
		var w := _work_lot(r)
		if w == -1:
			return
		a = home
		b = lots.front_point(lots.lots[w])
	elif purpose == "de vuelta a casa":
		if r.get("away_at_plaza", false):
			a = _plaza_point()
		elif r["role"] == 0 and _work_lot(r) != -1:
			a = lots.front_point(lots.lots[r["work"]])
		else:
			return
		b = home
		r["away_at_plaza"] = false
	elif purpose == "de vuelta del mercado":
		a = _plaza_point()
		b = home
	else:
		a = home
		b = _plaza_point()
		r["away_at_plaza"] = purpose.begins_with("a misa")
	var kind: int = r["kind"]
	# Para trayectos muy cortos hasta el jinete va a pie.
	if a.distance_to(b) < 150.0:
		kind = Kind.WALK
	spawn(kind, a, b, r["id"], purpose, r["route_seed"], TEMPERS[r["temper"]])


func _spawn_cart() -> void:
	var entry := _entry_node()
	if entry == -1 or lots.plaza_center.x == INF:
		return
	var from: Vector2 = net.nodes[entry]
	var temper: Dictionary = TEMPERS[_rng.randi() % TEMPERS.size()]
	var id := spawn(Kind.CART, from, _plaza_point(), -1, "trae cueros y trigo del campo", _rng.randi() % 100000, temper)
	if id != -1:
		by_id[id]["carter"] = "%s %s" % [FIRST[_rng.randi() % 12], LAST[_rng.randi() % LAST.size()]]


## Entrada desde el campo: el extremo de calle más alejado de la plaza.
func _entry_node() -> int:
	var best := -1
	var bd := 0.0
	for nid in net.nodes:
		if net.degree(nid) != 1:
			continue
		var d := (net.nodes[nid] as Vector2).distance_to(lots.plaza_center)
		if d > bd:
			bd = d
			best = nid
	return best


# --- Planificación de rutas ------------------------------------------------------------

func spawn(kind: int, from_p: Vector2, to_p: Vector2, res_id: int, purpose: String, seed_value: int, temper: Dictionary) -> int:
	if agents.size() >= MAX_AGENTS:
		trips_skipped += 1
		return -1
	var legs := plan(from_p, to_p, kind != Kind.WALK, seed_value)
	if legs.is_empty():
		trips_skipped += 1
		return -1
	var base: Dictionary = PARAMS[kind]
	var p := base.duplicate()
	p["v0"] = float(base["v0"]) * float(temper["v0"]) * _rng.randf_range(0.92, 1.08)
	p["T"] = float(base["T"]) * float(temper["T"])
	var expect := 0.0
	for leg in legs:
		expect += (float(leg[3]) - float(leg[2])) / float(p["v0"])
	var id := _next_agent
	_next_agent += 1
	var col: Color = WALKER_COLORS[_rng.randi() % WALKER_COLORS.size()]
	var ag := {
		"id": id, "kind": kind, "legs": legs, "li": 0, "s": float(legs[0][2]), "v": 0.0,
		"p": p, "temper": temper, "res": res_id, "purpose": purpose, "t0": sim_time,
		"lane_t0": sim_time, "expect": expect, "arr": -1.0, "wait": 0.0, "over": false,
		"lat": 0.0, "col": col, "col2": HORSE_COLORS[_rng.randi() % HORSE_COLORS.size()],
		"vis": Vector3.INF, "head": Vector3.FORWARD, "state": "en marcha",
	}
	agents.append(ag)
	by_id[id] = ag
	return id


## Ruta como lista de tramos: [tramo, sentido, s_inicio, s_fin], con `s` medido en el
## sentido de la marcha.
func plan(from_p: Vector2, to_p: Vector2, by_time: bool, seed_value: int) -> Array:
	var e0 := net.nearest_edge(from_p, 45.0)
	var e1 := net.nearest_edge(to_p, 70.0)
	if e0["edge"] == -1 or e1["edge"] == -1:
		return []
	var id0: int = e0["edge"]
	var id1: int = e1["edge"]
	var s0 := net.edge_s_of(id0, e0["seg"], e0["point"])
	var s1 := net.edge_s_of(id1, e1["seg"], e1["point"])
	var ed0: Dictionary = net.edges[id0]
	var ed1: Dictionary = net.edges[id1]
	var l0: float = ed0["len"]
	var l1: float = ed1["len"]
	if id0 == id1:
		if s1 >= s0:
			return [[id0, 0, s0, s1]]
		return [[id0, 1, l0 - s0, l0 - s1]]
	var a0: int = ed0["a"]
	var b0: int = ed0["b"]
	var start: int = a0 if (net.nodes[a0] as Vector2).distance_to(to_p) < (net.nodes[b0] as Vector2).distance_to(to_p) else b0
	var a1: int = ed1["a"]
	var b1: int = ed1["b"]
	var end: int = a1 if (net.nodes[a1] as Vector2).distance_to(from_p) < (net.nodes[b1] as Vector2).distance_to(from_p) else b1
	var path := router.find(start, end, by_time, seed_value)
	if path.is_empty():
		return []
	var legs := []
	var dir0 := 0 if start == b0 else 1
	legs.append([id0, dir0, s0 if dir0 == 0 else l0 - s0, l0])
	for i in path.size() - 1:
		var eid := router.edge_between(path[i], path[i + 1])
		if eid == -1:
			return []
		var dir := 0 if net.edges[eid]["a"] == path[i] else 1
		legs.append([eid, dir, 0.0, float(net.edges[eid]["len"])])
	var dir1 := 0 if end == a1 else 1
	legs.append([id1, dir1, 0.0, s1 if dir1 == 0 else l1 - s1])
	# Quita tramos de longitud nula (por ejemplo, si se sale justo desde un nodo).
	var out := []
	for leg in legs:
		if float(leg[3]) - float(leg[2]) > 0.05:
			out.append(leg)
	return out


func _on_roads_changed() -> void:
	router.rebuild(net)
	for ag in agents.duplicate():
		for leg in ag["legs"]:
			if not net.edges.has(leg[0]):
				_remove(ag)
				break
	node_occ.clear()


# --- Movimiento ---------------------------------------------------------------------

static func lane_key(leg: Array) -> int:
	return int(leg[0]) * 2 + int(leg[1])


func _end_node(leg: Array) -> int:
	var e: Dictionary = net.edges[leg[0]]
	return e["b"] if int(leg[1]) == 0 else e["a"]


func _step(dt: float) -> void:
	sim_time += dt
	# 1. Carriles: vehículos ordenados del más adelantado al último.
	var lanes := {}
	for ag in agents:
		if ag["kind"] == Kind.WALK:
			continue
		var key := lane_key(ag["legs"][ag["li"]])
		if not lanes.has(key):
			lanes[key] = []
		(lanes[key] as Array).append(ag)
	for key in lanes:
		var lane: Array = lanes[key]
		lane.sort_custom(func(x, y): return x["s"] > y["s"])
		for i in lane.size():
			lane[i]["lane_idx"] = i

	# 2. Cruces: se pide paso al acercarse; se concede por orden de llegada si la salida
	#    está libre (no se bloquea el cruce).
	#    Solo pide paso el primero de cada fila que va a cruzar (el resto espera detrás).
	var requests := {}
	for key in lanes:
		var first: Dictionary = {}
		for cand in lanes[key]:
			if cand["li"] < cand["legs"].size() - 1:
				first = cand
				break
		if first.is_empty():
			continue
		var ag: Dictionary = first
		var leg: Array = ag["legs"][ag["li"]]
		if float(leg[3]) - float(ag["s"]) > APPROACH:
			continue
		var nid := _end_node(leg)
		if net.degree(nid) < 3:
			continue
		if ag["arr"] < 0.0:
			ag["arr"] = sim_time
		if not requests.has(nid):
			requests[nid] = []
		(requests[nid] as Array).append(ag)
	for nid in node_occ.keys():
		if not _holds(node_occ[nid], nid):
			node_occ.erase(nid)
		elif float(by_id[node_occ[nid]]["wait"]) > float(_occ_wait0.get(nid, 0.0)) + 60.0:
			# Salvaguarda: si quien tiene el paso no avanza en un minuto, lo cede.
			node_occ.erase(nid)
	for nid in requests:
		if node_occ.has(nid):
			continue
		var best: Dictionary = {}
		for ag in requests[nid]:
			if not _exit_clear(ag, lanes):
				continue
			if best.is_empty() or ag["arr"] < best["arr"]:
				best = ag
		if not best.is_empty():
			node_occ[nid] = best["id"]
			_occ_wait0[nid] = best["wait"]

	# 3. Conducción (IDM).
	for ag in agents:
		if ag["kind"] == Kind.WALK:
			ag["v"] = ag["p"]["v0"]
			ag["s"] = float(ag["s"]) + float(ag["v"]) * dt
			continue
		_drive(ag, lanes, dt)

	# 4. Cambios de tramo y llegadas.
	for ag in agents.duplicate():
		_advance_legs(ag)


func _holds(agent_id: int, nid: int) -> bool:
	if not by_id.has(agent_id):
		return false
	var ag: Dictionary = by_id[agent_id]
	var leg: Array = ag["legs"][ag["li"]]
	if _end_node(leg) == nid:
		return true
	# Ya cruzó: libera el cruce cuando se alejó unos metros.
	if ag["li"] > 0 and _end_node(ag["legs"][ag["li"] - 1]) == nid:
		return float(ag["s"]) - float(leg[2]) < 7.0
	return false


func _exit_clear(ag: Dictionary, lanes: Dictionary) -> bool:
	var next: Array = ag["legs"][ag["li"] + 1]
	var lane: Array = lanes.get(lane_key(next), [])
	if lane.is_empty():
		return true
	var last: Dictionary = lane[lane.size() - 1]
	return float(last["s"]) - float(last["p"]["len"]) > float(ag["p"]["len"]) + 3.0


func _drive(ag: Dictionary, lanes: Dictionary, dt: float) -> void:
	var p: Dictionary = ag["p"]
	var legs: Array = ag["legs"]
	var li: int = ag["li"]
	var leg: Array = legs[li]
	var s: float = ag["s"]
	var v: float = ag["v"]
	var to_end := float(leg[3]) - s
	var gap := INF
	var lead_v := 0.0
	var lane: Array = lanes[lane_key(leg)]
	var idx: int = ag["lane_idx"]
	var leader: Dictionary = lane[idx - 1] if idx > 0 else {}
	ag["state"] = "en marcha"

	# Adelantamiento prudente: solo jinetes, detrás de algo lento y con el contrario libre.
	if ag["over"]:
		if leader.is_empty() or s > float(ag["over_s"]) or not _opposite_clear(ag, 25.0):
			ag["over"] = false
		else:
			leader = {}
			ag["state"] = "adelantando"
	elif not leader.is_empty() and ag["kind"] == Kind.RIDER and float(leader["v"]) < 0.6 * float(p["v0"]):
		var lgap := float(leader["s"]) - float(leader["p"]["len"]) - s
		if lgap < 14.0 and to_end > 45.0 and _rng.randf() < float(ag["temper"]["overtake"]) * 0.05 and _opposite_clear(ag, 60.0):
			ag["over"] = true
			overtakes += 1
			ag["over_s"] = float(leader["s"]) + 6.0
			leader = {}
			ag["state"] = "adelantando"

	if not leader.is_empty():
		gap = float(leader["s"]) - float(leader["p"]["len"]) - s
		lead_v = leader["v"]
	elif li < legs.size() - 1:
		var nlane: Array = lanes.get(lane_key(legs[li + 1]), [])
		if not nlane.is_empty():
			var last: Dictionary = nlane[nlane.size() - 1]
			gap = to_end + float(last["s"]) - float(last["p"]["len"]) - float(legs[li + 1][2])
			lead_v = last["v"]

	# Ceder el paso: detenerse antes del cruce si no le toca.
	if li < legs.size() - 1:
		var nid := _end_node(leg)
		if net.degree(nid) >= 3 and to_end < APPROACH and node_occ.get(nid, -1) != ag["id"]:
			var stop := to_end - 5.0
			if stop < gap:
				gap = stop
				lead_v = 0.0
				if v < 0.3:
					ag["state"] = "cediendo el paso"
	else:
		# Llegar al destino frenando suave (el objetivo es el propio punto de llegada).
		var stop_end := to_end + float(p["s0"]) + 0.3
		if stop_end < gap:
			gap = stop_end
			lead_v = 0.0

	# Velocidad deseada: tipo de calle, curvas y giros.
	var v0: float = p["v0"]
	if net.edges[leg[0]]["type"] == RoadNetwork.Type.DIRT:
		v0 *= 0.88
	v0 = minf(v0, _curve_speed(leg, s))
	if li < legs.size() - 1 and to_end < 18.0 and _turn_angle(leg, legs[li + 1]) > 0.5:
		v0 = minf(v0, 1.6)

	# Modelo de conductor inteligente (IDM).
	var a: float = p["a"]
	var b: float = p["b"]
	var free := 1.0 - pow(v / maxf(v0, 0.1), 4.0)
	var acc := a * free
	if gap < INF:
		var sstar := float(p["s0"]) + maxf(0.0, v * float(p["T"]) + v * (v - lead_v) / (2.0 * sqrt(a * b)))
		acc = a * (free - pow(sstar / maxf(gap, 0.05), 2.0))
	acc = clampf(acc, -4.0 * b, a)
	v = maxf(0.0, v + acc * dt)
	var ds := v * dt
	if gap < INF:
		ds = minf(ds, maxf(gap - 0.2, 0.0))
	ag["v"] = ds / dt
	ag["s"] = s + ds
	if ag["v"] < 0.1:
		ag["wait"] = float(ag["wait"]) + dt
		if ag["state"] == "en marcha":
			ag["state"] = "esperando en la fila"


func _opposite_clear(ag: Dictionary, ahead: float) -> bool:
	var leg: Array = ag["legs"][ag["li"]]
	var L: float = net.edges[leg[0]]["len"]
	var my_poly := float(ag["s"]) if int(leg[1]) == 0 else L - float(ag["s"])
	for other in agents:
		if other["kind"] == Kind.WALK or other["id"] == ag["id"]:
			continue
		var ol: Array = other["legs"][other["li"]]
		if ol[0] != leg[0] or ol[1] == leg[1]:
			continue
		var op := float(other["s"]) if int(ol[1]) == 0 else L - float(other["s"])
		var rel := (op - my_poly) * (1.0 if int(leg[1]) == 0 else -1.0)
		if rel > -5.0 and rel < ahead:
			return false
	return true


## Velocidad máxima cómoda en curva: v = sqrt(aceleración lateral × radio).
func _curve_speed(leg: Array, s: float) -> float:
	var L: float = net.edges[leg[0]]["len"]
	var dir: int = leg[1]
	var ps := s if dir == 0 else L - s
	var ahead := 10.0 if dir == 0 else -10.0
	var t0: Vector2 = net.edge_point(leg[0], ps)[1]
	var t1: Vector2 = net.edge_point(leg[0], clampf(ps + ahead, 0.0, L))[1]
	var ang := absf(t0.angle_to(t1))
	if ang < 0.02:
		return INF
	var radius := 10.0 / ang
	return sqrt(1.2 * radius)


func _turn_angle(leg: Array, next: Array) -> float:
	var L: float = net.edges[leg[0]]["len"]
	var t_in: Vector2 = net.edge_point(leg[0], L if int(leg[1]) == 0 else 0.0)[1]
	if int(leg[1]) == 1:
		t_in = -t_in
	var L2: float = net.edges[next[0]]["len"]
	var t_out: Vector2 = net.edge_point(next[0], 0.0 if int(next[1]) == 0 else L2)[1]
	if int(next[1]) == 1:
		t_out = -t_out
	return absf(t_in.angle_to(t_out))


func _advance_legs(ag: Dictionary) -> void:
	var legs: Array = ag["legs"]
	while true:
		var leg: Array = legs[ag["li"]]
		var last := int(ag["li"]) >= legs.size() - 1
		if last and float(ag["s"]) >= float(leg[3]) - 0.6:
			_arrive(ag)
			return
		if float(ag["s"]) < float(leg[3]):
			return
		if ag["li"] >= legs.size() - 1:
			_arrive(ag)
			return
		var over := float(ag["s"]) - float(leg[3])
		# Tiempo medido en el carril (solo si lo recorrió completo).
		if ag["kind"] != Kind.WALK and float(leg[2]) < 0.01:
			router.record(leg[0], leg[1], sim_time - float(ag["lane_t0"]))
		ag["li"] = int(ag["li"]) + 1
		ag["lane_t0"] = sim_time
		ag["arr"] = -1.0
		ag["over"] = false
		var nxt: Array = legs[ag["li"]]
		if not net.edges.has(nxt[0]):
			_remove(ag)
			return
		ag["s"] = float(nxt[2]) + over


func _arrive(ag: Dictionary) -> void:
	trips_done += 1
	var took := sim_time - float(ag["t0"])
	var res_id: int = ag["res"]
	if residents.has(res_id):
		var r: Dictionary = residents[res_id]
		r["last_trip"] = "%s: %d min (esperaba %d)" % [ag["purpose"], game_minutes(took), game_minutes(float(ag["expect"]))]
		# Aprende: si tardó mucho más de lo esperado, mañana prueba otra ruta.
		if took > float(ag["expect"]) * 1.5 and ag["kind"] != Kind.WALK:
			r["route_seed"] = _rng.randi() % 100000
			r["replans"] = int(r["replans"]) + 1
	elif ag["kind"] == Kind.CART and String(ag["purpose"]).begins_with("trae"):
		# La carreta descarga en el mercado y vuelve al campo más tarde.
		var entry := _entry_node()
		if entry != -1:
			var at: Vector2 = _plaza_point()
			var dest: Vector2 = net.nodes[entry]
			var temper: Dictionary = ag["temper"]
			var carter: String = ag.get("carter", "un carretero")
			var back := func():
				var nid := spawn(Kind.CART, at, dest, -1, "vuelve al campo con mercancías", _rng.randi() % 100000, temper)
				if nid != -1:
					by_id[nid]["carter"] = carter
			_pending.append([sim_time + 600.0 + _rng.randf() * 900.0, back])
	_remove(ag)


func _remove(ag: Dictionary) -> void:
	agents.erase(ag)
	by_id.erase(ag["id"])
	if selected_id == ag["id"]:
		selected_id = -1


# --- Dibujo, selección e información --------------------------------------------------

func _target_pos(ag: Dictionary) -> Array:
	var leg: Array = ag["legs"][ag["li"]]
	var eid: int = leg[0]
	var L: float = net.edges[eid]["len"]
	var dir: int = leg[1]
	var ps := float(ag["s"]) if dir == 0 else L - float(ag["s"])
	var pt: Array = net.edge_point(eid, ps)
	var tan: Vector2 = pt[1] if dir == 0 else -(pt[1] as Vector2)
	var w: float = RoadNetwork.WIDTH[net.edges[eid]["type"]]
	var lat := w * 0.25
	if ag["kind"] == Kind.WALK:
		lat = w * 0.5 - 0.4 + float(ag["id"] % 3) * 0.25
	elif ag["over"]:
		lat = -w * 0.25
	ag["lat"] = lerpf(float(ag["lat"]), lat, 0.15)
	var right := Vector2(-tan.y, tan.x)
	var p2: Vector2 = (pt[0] as Vector2) + right * float(ag["lat"])
	return [Vector3(p2.x, gen.height(p2.x, p2.y) + 0.45, p2.y), Vector3(tan.x, 0, tan.y)]


func _render() -> void:
	var counts := {"walk": 0, "horse": 0, "rider": 0, "cart": 0}
	for ag in agents:
		var tp := _target_pos(ag)
		var target: Vector3 = tp[0]
		var vis: Vector3 = ag["vis"]
		if vis == Vector3.INF or vis.distance_to(target) > 30.0:
			vis = target
		else:
			vis = vis.lerp(target, 0.35)
		var head: Vector3 = (ag["head"] as Vector3).lerp(tp[1], 0.25)
		if head.length() < 0.01:
			head = tp[1]
		ag["vis"] = vis
		ag["head"] = head
		var basis := Basis.looking_at(-head.normalized(), Vector3.UP)
		var xf := Transform3D(basis, vis)
		match ag["kind"]:
			Kind.WALK:
				_put("walk", counts, xf, ag["col"])
			Kind.RIDER:
				_put("horse", counts, xf, ag["col2"])
				_put("rider", counts, xf, ag["col"])
			Kind.CART:
				_put("cart", counts, xf, Color(1, 1, 1))
	for k in counts:
		(_mm[k] as MultiMesh).visible_instance_count = counts[k]


func _put(key: String, counts: Dictionary, xf: Transform3D, col: Color) -> void:
	var i: int = counts[key]
	if i >= MAX_AGENTS:
		return
	var mm: MultiMesh = _mm[key]
	mm.set_instance_transform(i, xf)
	mm.set_instance_color(i, col)
	counts[key] = i + 1


func pick(p: Vector2, radius: float) -> int:
	var best := -1
	var bd := radius
	for ag in agents:
		var v: Vector3 = ag["vis"]
		if v == Vector3.INF:
			continue
		var d := Vector2(v.x, v.z).distance_to(p)
		if d < bd:
			bd = d
			best = ag["id"]
	return best


func position_of(id: int) -> Variant:
	if not by_id.has(id):
		return null
	return by_id[id]["vis"]


## Segundos simulados de agentes -> minutos del reloj del juego.
func game_minutes(sim_seconds: float) -> int:
	return int(sim_seconds / TIME_SCALE / day.day_length * 1440.0)


func describe(id: int) -> String:
	if not by_id.has(id):
		return ""
	var ag: Dictionary = by_id[id]
	var txt := ""
	if residents.has(ag["res"]):
		var r: Dictionary = residents[ag["res"]]
		txt += "%s\n%s, %s\n%s\n" % [r["name"], String(r["job"]).capitalize(), r["origin"], String(ROLES[r["role"]]).capitalize()]
		txt += "Temperamento: %s\n" % TEMPERS[r["temper"]]["name"]
	elif ag["kind"] == Kind.CART:
		txt += "Carreta de bueyes\nCarretero: %s\nTemperamento: %s\n" % [ag.get("carter", "—"), ag["temper"]["name"]]
	txt += "Va %s, %s\n" % [ag["purpose"], KIND_NAMES[ag["kind"]]]
	txt += "Estado: %s\n" % ag["state"]
	txt += "Velocidad: %.1f km/h\n" % (float(ag["v"]) * 3.6)
	txt += "En camino: %d min (prevé %d)\n" % [game_minutes(sim_time - float(ag["t0"])), game_minutes(float(ag["expect"]))]
	if residents.has(ag["res"]):
		var r2: Dictionary = residents[ag["res"]]
		if r2["last_trip"] != "":
			txt += "Último viaje: %s\n" % r2["last_trip"]
		if r2["replans"] > 0:
			txt += "Cambió de ruta %d veces por demoras\n" % r2["replans"]
	txt += "\nF: seguir con la cámara"
	return txt


func lane_info(eid: int) -> String:
	var ft := router.free_time(eid)
	var t0 := router.lane_cost(eid, 0)
	var t1 := router.lane_cost(eid, 1)
	return "Tiempo de paso medido: %d s / %d s (libre: %d s)" % [int(t0), int(t1), int(ft)]


func active_count() -> int:
	return agents.size()
