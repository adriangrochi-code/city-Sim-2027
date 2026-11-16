extends Node3D
## Solares: se generan a ambos lados de cada calle siguiendo la curva, se zonifican con
## pincel y crecen en tres niveles (rancho → casa de azotea → casa de altos).

const RoadNetwork = preload("res://scripts/roads/road_network.gd")
const TerrainGen = preload("res://scripts/world/terrain_gen.gd")
const Parcels = preload("res://scripts/world/parcels.gd")
const BuildingGen = preload("res://scripts/city/building_gen.gd")

signal lots_changed
signal lot_level_changed(id: int, old_level: int, new_level: int)

const W := 12.0          # frente del solar
const D := 38.0          # fondo del solar: hasta el medio de la manzana de 100 varas
const CAP := [0, 4, 8, 14]
const LEVEL_NAMES := ["Solar vacío", "Rancho", "Casa de azotea", "Casa de altos con mirador"]
const CELL := 30.0
const VARIANTS := 8

var net: RoadNetwork
var gen: TerrainGen
var parcels: Parcels
var lots := {}             # id -> {"c", "f", "zone", "level", "age", "seed", "node"}
var obstacles: Array = []  # rectángulos orientados {"c", "f", "w", "d"} (plaza, iglesia)
var plaza_center := Vector2(INF, INF)
var show_candidates := false
var overlay_mode := "zone"   # "zone" o "water"
var nuisances: Array = []    # saladeros: su olor baja el valor del suelo
const BRICKS := [0, 0, 8, 15]  # ladrillos para llegar a cada nivel
var bricks_available := 0       # lo fija main antes de cada paso de crecimiento
var bricks_used := 0
var extra_l3_bricks := 0        # ordenanza: aljibe obligatorio en casas de altos
var sick_check: Callable        # func(id) -> bool: las casas con fiebre no crecen
const ROOF_H := [0.0, 5.0, 5.6, 12.5]
var _next := 1
var _hash := {}
var _mm: MultiMesh
var _building_mat: ShaderMaterial
var _overlay_dirty := true


func setup(n: RoadNetwork, g: TerrainGen, p: Parcels) -> void:
	net = n
	gen = g
	parcels = p
	_building_mat = BuildingGen.material()
	var quad := PlaneMesh.new()
	quad.size = Vector2(1, 1)
	var mat := StandardMaterial3D.new()
	mat.roughness = 1.0
	mat.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	mat.vertex_color_use_as_albedo = true
	quad.material = mat
	_mm = MultiMesh.new()
	_mm.transform_format = MultiMesh.TRANSFORM_3D
	_mm.use_colors = true
	_mm.mesh = quad
	var mmi := MultiMeshInstance3D.new()
	mmi.multimesh = _mm
	mmi.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	add_child(mmi)


func _process(_delta: float) -> void:
	if _overlay_dirty:
		_overlay_dirty = false
		_rebuild_overlay()


# --- Geometría de los solares ---------------------------------------------------

static func side_of(f: Vector2) -> Vector2:
	return Vector2(f.y, -f.x)


static func corners_of(c: Vector2, f: Vector2, w: float, d: float) -> PackedVector2Array:
	var s := side_of(f) * (w * 0.5)
	var ff := f * (d * 0.5)
	return PackedVector2Array([c + s + ff, c - s + ff, c - s - ff, c + s - ff])


func lot_corners(lot: Dictionary, shrink := 0.0) -> PackedVector2Array:
	return corners_of(lot["c"], lot["f"], W - shrink, D - shrink)


static func polys_overlap(a: PackedVector2Array, b: PackedVector2Array) -> bool:
	for poly in [a, b]:
		var pp: PackedVector2Array = poly
		for i in pp.size():
			var e := pp[(i + 1) % pp.size()] - pp[i]
			var axis := Vector2(-e.y, e.x)
			var amin := INF
			var amax := -INF
			var bmin := INF
			var bmax := -INF
			for v in a:
				var p := axis.dot(v)
				amin = minf(amin, p)
				amax = maxf(amax, p)
			for v in b:
				var p := axis.dot(v)
				bmin = minf(bmin, p)
				bmax = maxf(bmax, p)
			if amax <= bmin or bmax <= amin:
				return false
	return true


func lot_at(p: Vector2) -> int:
	for id in _nearby(p):
		if Geometry2D.is_point_in_polygon(p, lot_corners(lots[id])):
			return id
	return -1


## Punto de la calle frente al solar (donde salen y llegan los vecinos).
func front_point(lot: Dictionary) -> Vector2:
	return lot["c"] + lot["f"] * (D * 0.5 + 1.0)


func land_value(lot: Dictionary) -> float:
	if plaza_center.x == INF:
		return 0.2
	var d := (lot["c"] as Vector2).distance_to(plaza_center)
	var v := clampf(exp(-d / 650.0), 0.0, 1.0)
	for n in nuisances:
		if (lot["c"] as Vector2).distance_to(n) < 350.0:
			v *= 0.4
			break
	return v


func frontage_type(lot: Dictionary) -> int:
	var front: Vector2 = lot["c"] + lot["f"] * (D * 0.5 + 3.0)
	var e := net.nearest_edge(front, 10.0)
	if e["edge"] == -1:
		return -1
	return net.edges[e["edge"]]["type"]


func population() -> int:
	var p := 0
	for id in lots:
		p += CAP[lots[id]["level"]]
	return p


func count_zoned() -> int:
	var n := 0
	for id in lots:
		if lots[id]["zone"] > 0:
			n += 1
	return n


func count_built() -> int:
	var n := 0
	for id in lots:
		if lots[id]["level"] > 0:
			n += 1
	return n


## ¿Este punto está dentro de un solar edificado o de un obstáculo? (para quitar árboles)
func is_occupied(p: Vector2) -> bool:
	var id := lot_at(p)
	if id != -1 and (lots[id]["zone"] > 0):
		return true
	for ob in obstacles:
		if Geometry2D.is_point_in_polygon(p, corners_of(ob["c"], ob["f"], ob["w"], ob["d"])):
			return true
	return false


# --- Cambios en la red de calles -----------------------------------------------

func on_roads_changed(new_edges: Array) -> void:
	# Quitar solares que quedaron debajo de una calle o sin frente a una calle.
	for id in lots.keys():
		var lot: Dictionary = lots[id]
		var bad := false
		for p in lot_corners(lot, 0.6):
			if net.clearance(p) < 0.2:
				bad = true
				break
		if not bad and net.clearance(lot["c"]) < 0.2:
			bad = true
		var front: Vector2 = lot["c"] + lot["f"] * (D * 0.5)
		if not bad and net.clearance(front, 20.0) > 3.0:
			bad = true
		if bad:
			_remove_lot(id)
	for eid in new_edges:
		if net.edges.has(eid):
			_generate_along(eid)
	_overlay_dirty = true
	lots_changed.emit()


func _generate_along(eid: int) -> void:
	var e: Dictionary = net.edges[eid]
	var pts: PackedVector2Array = e["pts"]
	var hw: float = RoadNetwork.WIDTH[e["type"]] * 0.5
	var cum := PackedFloat32Array([0.0])
	for i in pts.size() - 1:
		cum.append(cum[i] + pts[i].distance_to(pts[i + 1]))
	var total := cum[cum.size() - 1]
	var s := 3.0 + W * 0.5
	var seg := 0
	while s <= total - 3.0 - W * 0.5:
		while seg < pts.size() - 2 and cum[seg + 1] < s:
			seg += 1
		var seg_len := maxf(cum[seg + 1] - cum[seg], 0.001)
		var pt := pts[seg].lerp(pts[seg + 1], (s - cum[seg]) / seg_len)
		var tan := (pts[seg + 1] - pts[seg]).normalized()
		for sgn in [-1.0, 1.0]:
			var nrm := Vector2(-tan.y, tan.x) * float(sgn)
			var c := pt + nrm * (hw + 0.6 + D * 0.5)
			var f := -nrm
			if _valid(c, f):
				_add_lot(c, f)
		s += W


func _valid(c: Vector2, f: Vector2) -> bool:
	var cs := corners_of(c, f, W, D)
	var hmin := INF
	var hmax := -INF
	var samples := Array(cs)
	samples.append(c)
	for p in samples:
		if not parcels.is_owned_pos(p):
			return false
		var h := gen.height(p.x, p.y)
		if h < 0.6:
			return false
		hmin = minf(hmin, h)
		hmax = maxf(hmax, h)
	if hmax - hmin > 5.0:
		return false
	var test := corners_of(c, f, W - 0.6, D - 0.6)
	for p in test:
		if net.clearance(p) < 0.3:
			return false
	if net.clearance(c) < 0.3 or net.clearance(c - f * (D * 0.5 - 0.3)) < 0.3:
		return false
	for id in _nearby(c):
		if polys_overlap(test, lot_corners(lots[id], 0.6)):
			return false
	for ob in obstacles:
		if polys_overlap(test, corners_of(ob["c"], ob["f"], ob["w"], ob["d"])):
			return false
	return true


func _add_lot(c: Vector2, f: Vector2) -> int:
	var id := _next
	_next += 1
	lots[id] = {"id": id, "c": c, "f": f, "zone": 0, "level": 0, "age": 0, "seed": randi() % VARIANTS, "node": null,
		"water": 0.0, "water_src": -1, "ruin": 0}
	var k := _cell(c)
	if not _hash.has(k):
		_hash[k] = []
	(_hash[k] as Array).append(id)
	return id


func _remove_lot(id: int) -> void:
	var lot: Dictionary = lots[id]
	if lot["level"] > 0:
		lot_level_changed.emit(id, lot["level"], 0)
	if lot["node"] != null:
		(lot["node"] as Node).queue_free()
	(_hash[_cell(lot["c"])] as Array).erase(id)
	lots.erase(id)


## Quita los solares que se superponen con un rectángulo (al fundar la plaza).
func clear_area(corners: PackedVector2Array) -> void:
	for id in lots.keys():
		if polys_overlap(corners, lot_corners(lots[id], 0.6)):
			_remove_lot(id)
	_overlay_dirty = true


func _cell(p: Vector2) -> Vector2i:
	return Vector2i(floori(p.x / CELL), floori(p.y / CELL))


func _nearby(p: Vector2) -> Array:
	var out := []
	var c := _cell(p)
	for dy in range(-1, 2):
		for dx in range(-1, 2):
			var k := c + Vector2i(dx, dy)
			if _hash.has(k):
				out.append_array(_hash[k])
	return out


# --- Zonificación y crecimiento ---------------------------------------------------

func paint(center: Vector2, radius: float, zone: int) -> int:
	var changed := 0
	var r := int(ceil(radius / CELL)) + 1
	var c := _cell(center)
	for dy in range(-r, r + 1):
		for dx in range(-r, r + 1):
			var k := c + Vector2i(dx, dy)
			if not _hash.has(k):
				continue
			for id in _hash[k]:
				var lot: Dictionary = lots[id]
				if (lot["c"] as Vector2).distance_to(center) > radius or lot["zone"] == zone:
					continue
				lot["zone"] = zone
				if zone == 0 and lot["level"] > 0:
					_set_level(lot, 0)
				changed += 1
	if changed > 0:
		_overlay_dirty = true
		lots_changed.emit()
	return changed


## Un paso de crecimiento. `pool` = pobladores esperando vivienda. Devuelve cuántos se instalan.
func grow_step(pool: int, rng: RandomNumberGenerator) -> int:
	var zoned := []
	for id in lots:
		if lots[id]["zone"] > 0 and lots[id]["level"] < 3:
			zoned.append(id)
	if zoned.is_empty():
		return 0
	var used := 0
	var tries := 2 + zoned.size() / 25
	for k in tries:
		var lot: Dictionary = lots[zoned[rng.randi() % zoned.size()]]
		var lvl: int = lot["level"]
		if lvl >= 3:
			continue
		var need: int = CAP[lvl + 1] - CAP[lvl]
		if pool - used < need:
			continue
		var ok := false
		var water: float = lot["water"]
		if BRICKS[lvl + 1] + (extra_l3_bricks if lvl == 2 else 0) > bricks_available - bricks_used:
			continue
		if sick_check.is_valid() and sick_check.call(lot["id"]):
			continue
		if lvl == 0:
			ok = int(lot["ruin"]) <= 0 and rng.randf() < 0.55
		elif lvl == 1:
			ok = lot["age"] >= 4 and land_value(lot) > 0.2 and water >= 0.5 and rng.randf() < 0.15
		else:
			ok = lot["age"] >= 8 and land_value(lot) > 0.45 and water >= 0.85 and frontage_type(lot) == RoadNetwork.Type.STONE and rng.randf() < 0.1
		if ok:
			_set_level(lot, lvl + 1)
			used += need
			bricks_used += BRICKS[lvl + 1] + (extra_l3_bricks if lvl == 2 else 0)
	if used > 0:
		_overlay_dirty = true
		lots_changed.emit()
	return used


func age_lots() -> void:
	for id in lots:
		var lot: Dictionary = lots[id]
		if lot["level"] > 0:
			lot["age"] += 1
		elif int(lot["ruin"]) > 0:
			lot["ruin"] = int(lot["ruin"]) - 1
			if int(lot["ruin"]) == 0 and lot["node"] != null:
				(lot["node"] as Node).queue_free()
				lot["node"] = null


## El fuego destruye la casa: quedan ruinas unos meses y después se puede reconstruir.
func burn_down(id: int) -> void:
	var lot: Dictionary = lots[id]
	_set_level(lot, 0)
	lot["ruin"] = 3
	_add_ruin(lot)
	_overlay_dirty = true
	lots_changed.emit()


## Datos mínimos de cada solar para guardar la partida.
func save_data() -> Array:
	var out := []
	for id in lots:
		var l: Dictionary = lots[id]
		out.append({"id": id, "c": l["c"], "f": l["f"], "zone": l["zone"], "level": l["level"], "age": l["age"], "seed": l["seed"], "ruin": l["ruin"]})
	return out


func restore(data: Array) -> void:
	for id in lots.keys():
		_remove_lot(id)
	for d in data:
		var id: int = d["id"]
		_next = maxi(_next, id + 1)
		lots[id] = {"id": id, "c": d["c"], "f": d["f"], "zone": d["zone"], "level": 0, "age": 0, "seed": d["seed"], "node": null,
			"water": 0.0, "water_src": -1, "ruin": d["ruin"]}
		var k := _cell(d["c"])
		if not _hash.has(k):
			_hash[k] = []
		(_hash[k] as Array).append(id)
		if int(d["level"]) > 0:
			_set_level(lots[id], d["level"])
		lots[id]["age"] = d["age"]
		if int(d["ruin"]) > 0:
			_add_ruin(lots[id])
	_overlay_dirty = true
	lots_changed.emit()


func _add_ruin(lot: Dictionary) -> void:
	var mi := MeshInstance3D.new()
	mi.mesh = BuildingGen.ruin_mesh(lot["seed"])
	mi.material_override = _building_mat
	var c: Vector2 = lot["c"]
	var f: Vector2 = lot["f"]
	var s := side_of(f)
	var base := INF
	for p in lot_corners(lot):
		base = minf(base, gen.height(p.x, p.y))
	mi.transform = Transform3D(Basis(Vector3(s.x, 0, s.y), Vector3.UP, Vector3(f.x, 0, f.y)), Vector3(c.x, base, c.y))
	add_child(mi)
	lot["node"] = mi


func refresh_overlay() -> void:
	_overlay_dirty = true


func set_overlay_mode(mode: String) -> void:
	if mode != overlay_mode:
		overlay_mode = mode
		_overlay_dirty = true


func _set_level(lot: Dictionary, lvl: int) -> void:
	var old: int = lot["level"]
	lot["level"] = lvl
	if old != lvl:
		lot_level_changed.emit(lot["id"], old, lvl)
	lot["age"] = 0
	if lot["node"] != null:
		(lot["node"] as Node).queue_free()
		lot["node"] = null
	if lvl == 0:
		return
	var c: Vector2 = lot["c"]
	var f: Vector2 = lot["f"]
	var s := side_of(f)
	var base := INF
	for p in lot_corners(lot):
		base = minf(base, gen.height(p.x, p.y))
	var mi := MeshInstance3D.new()
	mi.mesh = BuildingGen.get_mesh(lvl, lot["seed"], W, D)
	mi.material_override = _building_mat
	mi.transform = Transform3D(Basis(Vector3(s.x, 0, s.y), Vector3.UP, Vector3(f.x, 0, f.y)), Vector3(c.x, base, c.y))
	add_child(mi)
	lot["node"] = mi


func _rebuild_overlay() -> void:
	var items := []
	for id in lots:
		var lot: Dictionary = lots[id]
		if overlay_mode == "water" and lot["level"] > 0:
			var w: float = lot["water"]
			var col := Color(1.0, 0.3, 0.25, 0.6)
			if w >= 0.85:
				col = Color(0.25, 0.55, 1.0, 0.6)
			elif w >= 0.5:
				col = Color(0.45, 0.8, 0.9, 0.6)
			items.append([lot, col])
			continue
		if lot["level"] > 0:
			continue
		if lot["zone"] > 0:
			items.append([lot, Color(0.45, 0.85, 0.35, 0.45)])
		elif show_candidates:
			items.append([lot, Color(1.0, 0.97, 0.85, 0.18)])
	_mm.instance_count = items.size()
	for i in items.size():
		var lot: Dictionary = items[i][0]
		var c: Vector2 = lot["c"]
		var f: Vector2 = lot["f"]
		var s := side_of(f)
		var b := Basis(Vector3(s.x, 0, s.y) * (W - 1.2), Vector3.UP, Vector3(f.x, 0, f.y) * (D - 1.2))
		var y := gen.height(c.x, c.y) + 0.6
		if overlay_mode == "water" and lot["level"] > 0:
			y += ROOF_H[lot["level"]]
		_mm.set_instance_transform(i, Transform3D(b, Vector3(c.x, y, c.y)))
		_mm.set_instance_color(i, items[i][1])


func set_show_candidates(on: bool) -> void:
	if on != show_candidates:
		show_candidates = on
		_overlay_dirty = true
