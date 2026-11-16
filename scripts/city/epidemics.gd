extends Node3D
## Epidemias que llegan por barco. Con lazareto, los recién llegados hacen cuarentena en una
## punta alejada y la enfermedad no entra; sin lazareto, la fiebre desembarca con ellos y se
## propaga de casa en casa, más rápido donde el agua es mala.

const Lots = preload("res://scripts/city/lots.gd")
const Services = preload("res://scripts/city/services.gd")
const TerrainGen = preload("res://scripts/world/terrain_gen.gd")

signal event(text: String)

const SPREAD_DIST := 40.0
const SICK_MONTHS := 4
const QUARANTINE_MONTHS := 2

var lots: Lots
var services: Services
var gen: TerrainGen
var infected := {}        # id de solar -> meses que lleva enfermo
var quarantine: Array = [] # [meses que faltan, personas]
var spread_mult := 1.0   # ordenanza de limpieza
var deaths_total := 0
var outbreaks_total := 0
var quarantined_total := 0
var _rng := RandomNumberGenerator.new()
var _mm: MultiMesh


func setup(l: Lots, s: Services, g: TerrainGen) -> void:
	lots = l
	services = s
	gen = g
	_rng.seed = 1857
	var q := QuadMesh.new()
	q.size = Vector2(3.0, 3.0)
	var m := StandardMaterial3D.new()
	m.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	m.billboard_mode = BaseMaterial3D.BILLBOARD_ENABLED
	m.billboard_keep_scale = true
	m.albedo_color = Color(0.85, 0.80, 0.20)
	q.material = m
	_mm = MultiMesh.new()
	_mm.transform_format = MultiMesh.TRANSFORM_3D
	_mm.mesh = q
	var mmi := MultiMeshInstance3D.new()
	mmi.multimesh = _mm
	mmi.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	add_child(mmi)


func has_lazareto() -> bool:
	return not services.of_type(Services.Type.LAZARETO).is_empty()


## Un barco llegó con fiebre a bordo. Devuelve cuántas personas pasan ya a la ciudad.
func sick_ship(people: int, landing: Vector2) -> int:
	if has_lazareto():
		quarantine.append([QUARANTINE_MONTHS, people])
		quarantined_total += people
		event.emit("Fiebre a bordo: %d pasajeros hacen cuarentena en el lazareto" % people)
		return 0
	outbreaks_total += 1
	event.emit("¡Fiebre! Un barco trae la enfermedad y no hay lazareto")
	# La enfermedad empieza en las casas más cercanas al desembarco.
	var cands := []
	for id in lots.lots:
		if lots.lots[id]["level"] > 0:
			cands.append([(lots.lots[id]["c"] as Vector2).distance_to(landing), id])
	cands.sort_custom(func(a, b): return a[0] < b[0])
	for k in mini(3, cands.size()):
		infected[cands[k][1]] = 0
	_refresh_markers()
	return people


## Se llama dos veces por segundo de juego: contagios entre vecinos.
func step() -> void:
	if infected.is_empty():
		return
	var new_cases := []
	for id in infected.keys():
		if not lots.lots.has(id) or lots.lots[id]["level"] == 0:
			infected.erase(id)
			continue
		var lot: Dictionary = lots.lots[id]
		for oid in lots._nearby(lot["c"]):
			if oid == id or infected.has(oid) or lots.lots[oid]["level"] == 0:
				continue
			var other: Dictionary = lots.lots[oid]
			if (other["c"] as Vector2).distance_to(lot["c"]) > SPREAD_DIST:
				continue
			var w: float = other["water"]
			var factor := 1.6 if w < 0.5 else (1.0 if w < 0.85 else 0.45)
			if _rng.randf() < 0.012 * factor * spread_mult:
				new_cases.append(oid)
	for id in new_cases:
		infected[id] = 0
	if not new_cases.is_empty():
		_refresh_markers()


## Una vez por mes: la fiebre se cobra víctimas, las casas se recuperan, terminan cuarentenas.
## Devuelve las personas que salen de la cuarentena hacia la ciudad.
func month_tick() -> int:
	var released := 0
	for i in range(quarantine.size() - 1, -1, -1):
		quarantine[i][0] = int(quarantine[i][0]) - 1
		if int(quarantine[i][0]) <= 0:
			released += int(quarantine[i][1])
			quarantine.remove_at(i)
	if released > 0:
		event.emit("Terminó la cuarentena: %d pobladores entran sanos a la ciudad" % released)
	if infected.is_empty():
		return released
	var dead := 0
	for id in infected.keys():
		infected[id] = int(infected[id]) + 1
		if lots.lots.has(id) and _rng.randf() < 0.18:
			dead += 1
			var lot: Dictionary = lots.lots[id]
			if int(lot["level"]) > 1 and _rng.randf() < 0.4:
				lots._set_level(lot, int(lot["level"]) - 1)
		if int(infected[id]) >= SICK_MONTHS:
			infected.erase(id)
	deaths_total += dead
	if dead > 0:
		event.emit("La fiebre se cobra %d víctimas este mes" % dead)
	if infected.is_empty():
		event.emit("Terminó la epidemia")
	_refresh_markers()
	return released


func attraction() -> float:
	## Con epidemia llega menos gente.
	return 0.5 if not infected.is_empty() else 1.0


func is_sick(id: int) -> bool:
	return infected.has(id)


func _refresh_markers() -> void:
	_mm.instance_count = infected.size()
	var i := 0
	for id in infected:
		if not lots.lots.has(id):
			continue
		var c: Vector2 = lots.lots[id]["c"]
		var lvl: int = lots.lots[id]["level"]
		_mm.set_instance_transform(i, Transform3D(Basis(), Vector3(c.x, gen.height(c.x, c.y) + Lots.ROOF_H[lvl] + 4.0, c.y)))
		i += 1
	_mm.visible_instance_count = i
