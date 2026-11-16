extends Node3D
## Servicios de agua de la época colonial: pozos de balde, aljibes públicos y puestos de
## aguadores. Cada solar edificado recibe agua de la fuente más buena que tenga cerca y con
## capacidad libre. Sin agua, las casas no mejoran y los incendios no se pueden apagar.

const TerrainGen = preload("res://scripts/world/terrain_gen.gd")
const Parcels = preload("res://scripts/world/parcels.gd")
const RoadNetwork = preload("res://scripts/roads/road_network.gd")
const Lots = preload("res://scripts/city/lots.gd")
const BuildingGen = preload("res://scripts/city/building_gen.gd")

enum Type { WELL, CISTERN, WATER_CARRIERS, WATCH, ESTANCIA, SALADERO }
const INFO := {
	Type.WELL: {"name": "Pozo de balde", "cost": 400, "upkeep": 4, "cap": 60, "quality": 0.7, "radius": 110.0, "size": Vector2(5, 5),
		"desc": "Agua de pozo para unas 60 personas cercanas. Calidad regular."},
	Type.CISTERN: {"name": "Aljibe público", "cost": 1500, "upkeep": 12, "cap": 160, "quality": 1.0, "radius": 170.0, "size": Vector2(10, 10),
		"desc": "Junta el agua de lluvia en una cisterna. Buena agua para unas 160 personas."},
	Type.WATER_CARRIERS: {"name": "Puesto de aguadores", "cost": 2500, "upkeep": 30, "cap": 360, "quality": 0.9, "radius": 1100.0, "size": Vector2(14, 18),
		"desc": "Los aguadores llenan barriles en el arroyo y los reparten en carro. Debe estar a menos de 250 m de un arroyo."},
	Type.WATCH: {"name": "Puesto de serenos", "cost": 1200, "upkeep": 25, "cap": 0, "quality": 0.0, "radius": 260.0, "size": Vector2(6, 8),
		"desc": "De noche los serenos encienden los faroles de las calles cercanas y hacen la ronda cantando la hora. Con calles iluminadas, los vecinos salen a la tertulia."},
	Type.ESTANCIA: {"name": "Estancia", "cost": 3000, "upkeep": 0, "cap": 0, "quality": 0.0, "radius": 0.0, "size": Vector2(60, 60),
		"desc": "Cría ganado en el campo, lejos de la ciudad. Sus carretas llevan el ganado al saladero (o al mercado si no hay)."},
	Type.SALADERO: {"name": "Saladero", "cost": 5000, "upkeep": 20, "cap": 0, "quality": 0.0, "radius": 350.0, "size": Vector2(36, 44),
		"desc": "Carnea el ganado y sala los cueros y el tasajo para embarcar. Necesita agua cerca. Huele mal: baja el valor del suelo y contamina los pozos cercanos."},
}
const CONTAMINATION := 300.0
const FIRE_REACH := {Type.WELL: 140.0, Type.CISTERN: 180.0, Type.WATER_CARRIERS: 450.0}

var gen: TerrainGen
var parcels: Parcels
var net: RoadNetwork
var lots: Lots
var items := {}     # id -> {"id", "type", "c", "f", "node", "served"}
var lamps := PackedVector3Array()
var _next := 1
var _lamp_mm: MultiMesh
var lantern_mat: ShaderMaterial


func setup(g: TerrainGen, p: Parcels, n: RoadNetwork, l: Lots) -> void:
	gen = g
	parcels = p
	net = n
	lots = l
	lantern_mat = BuildingGen.material().duplicate()
	lantern_mat.set_shader_parameter("lit_ratio", 1.0)
	_lamp_mm = MultiMesh.new()
	_lamp_mm.transform_format = MultiMesh.TRANSFORM_3D
	_lamp_mm.mesh = _lamp_mesh()
	var mmi := MultiMeshInstance3D.new()
	mmi.multimesh = _lamp_mm
	mmi.material_override = lantern_mat
	mmi.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	add_child(mmi)
	net.changed.connect(rebuild_lamps)


func frame(angle: float) -> Vector2:
	return Vector2(sin(angle), cos(angle))


func corners(type: int, c: Vector2, f: Vector2) -> PackedVector2Array:
	var size: Vector2 = INFO[type]["size"]
	return Lots.corners_of(c, f, size.x, size.y)


## "" si se puede construir; si no, el motivo.
func problem(type: int, c: Vector2, angle: float) -> String:
	var f := frame(angle)
	var cs := corners(type, c, f)
	for p in Array(cs) + [c]:
		if not parcels.is_owned_pos(p):
			return "Fuera de tus parcelas"
		if gen.height(p.x, p.y) < 0.8:
			return "Demasiado cerca del agua"
		if net.clearance(p, 20.0) < 0.5:
			return "Está encima de una calle"
	var size: Vector2 = INFO[type]["size"]
	if net.clearance(c, 80.0) > maxf(22.0, size.y * 0.5 + 10.0):
		return "Debe estar junto a una calle o camino"
	if type == Type.ESTANCIA and lots.plaza_center.x != INF and c.distance_to(lots.plaza_center) < 600.0:
		return "Las estancias están en el campo, a más de 600 m de la plaza"
	if type == Type.SALADERO:
		if lots.plaza_center.x != INF and c.distance_to(lots.plaza_center) < 350.0:
			return "Demasiado cerca de la plaza: el saladero huele mal"
		if gen.arroyo_distance(c.x, c.y) > 220.0 and not _near_coast(c, 220.0):
			return "El saladero necesita un arroyo o la costa a menos de 220 m"
	for ob in lots.obstacles:
		if Lots.polys_overlap(cs, Lots.corners_of(ob["c"], ob["f"], ob["w"], ob["d"])):
			return "El lugar está ocupado"
	if type == Type.WATER_CARRIERS and gen.arroyo_distance(c.x, c.y) > 250.0:
		return "Los aguadores necesitan un arroyo a menos de 250 m"
	return ""


func place(type: int, c: Vector2, angle: float) -> int:
	var f := frame(angle)
	var size: Vector2 = INFO[type]["size"]
	var id := _next
	_next += 1
	lots.clear_area(corners(type, c, f))
	lots.obstacles.append({"c": c, "f": f, "w": size.x, "d": size.y})
	var mi := MeshInstance3D.new()
	mi.mesh = _mesh(type)
	mi.material_override = BuildingGen.material()
	var base := INF
	for p in corners(type, c, f):
		base = minf(base, gen.height(p.x, p.y))
	var s := Lots.side_of(f)
	mi.transform = Transform3D(Basis(Vector3(s.x, 0, s.y), Vector3.UP, Vector3(f.x, 0, f.y)), Vector3(c.x, base, c.y))
	add_child(mi)
	items[id] = {"id": id, "type": type, "c": c, "f": f, "node": mi, "served": 0, "stock": 0}
	_update_nuisances()
	if type == Type.WATCH:
		rebuild_lamps()
	return id


func remove(id: int) -> void:
	if not items.has(id):
		return
	var it: Dictionary = items[id]
	(it["node"] as Node).queue_free()
	for i in range(lots.obstacles.size() - 1, -1, -1):
		if (lots.obstacles[i]["c"] as Vector2).distance_to(it["c"]) < 0.01:
			lots.obstacles.remove_at(i)
	items.erase(id)
	rebuild_lamps()
	_update_nuisances()


func at(p: Vector2) -> int:
	for id in items:
		var it: Dictionary = items[id]
		if Geometry2D.is_point_in_polygon(p, corners(it["type"], it["c"], it["f"])):
			return id
	return -1


func save_data() -> Array:
	var out := []
	for id in items:
		var it: Dictionary = items[id]
		out.append([it["type"], it["c"], atan2((it["f"] as Vector2).x, (it["f"] as Vector2).y), it["stock"]])
	return out


func upkeep() -> int:
	var total := 0
	for id in items:
		total += int(INFO[items[id]["type"]]["upkeep"])
	return total


## Reparte el agua: primero las mejores fuentes, cada una hasta su capacidad, a los solares
## edificados más cercanos dentro de su alcance.
func update_water() -> void:
	for id in lots.lots:
		lots.lots[id]["water"] = 0.0
		lots.lots[id]["water_src"] = -1
	var order := items.keys()
	order.sort_custom(func(a, b): return float(INFO[items[a]["type"]]["quality"]) > float(INFO[items[b]["type"]]["quality"]))
	for sid in order:
		var it: Dictionary = items[sid]
		var info: Dictionary = INFO[it["type"]]
		if int(info["cap"]) <= 0:
			continue
		var radius: float = info["radius"]
		var cands := []
		for id in lots.lots:
			var lot: Dictionary = lots.lots[id]
			if lot["level"] == 0 or lot["water_src"] != -1:
				continue
			var d := (lot["c"] as Vector2).distance_to(it["c"])
			if d <= radius:
				cands.append([d, id])
		cands.sort_custom(func(a, b): return a[0] < b[0])
		var used := 0
		var quality: float = info["quality"]
		if it["type"] == Type.WELL and contaminated(it["c"]):
			quality = 0.35
		for cnd in cands:
			var lot: Dictionary = lots.lots[cnd[1]]
			var need: int = Lots.CAP[lot["level"]]
			if used + need > int(info["cap"]):
				break
			used += need
			lot["water"] = quality
			lot["water_src"] = sid
		it["served"] = used


## Fuente de agua más cercana que alcanza para apagar un fuego en `p` (o -1).
func fire_source(p: Vector2) -> int:
	var best := -1
	var bd := INF
	for id in items:
		var it: Dictionary = items[id]
		if not FIRE_REACH.has(it["type"]):
			continue
		var d := (it["c"] as Vector2).distance_to(p)
		if d <= float(FIRE_REACH[it["type"]]) and d < bd:
			bd = d
			best = id
	return best


## ¿El agua de un pozo en `p` está contaminada por un saladero cercano?
func contaminated(p: Vector2) -> bool:
	for id in items:
		var it: Dictionary = items[id]
		if it["type"] == Type.SALADERO and (it["c"] as Vector2).distance_to(p) < CONTAMINATION:
			return true
	return false


func of_type(type: int) -> Array:
	var out := []
	for id in items:
		if items[id]["type"] == type:
			out.append(id)
	return out


func _near_coast(c: Vector2, r: float) -> bool:
	for k in 16:
		var a := TAU * k / 16.0
		for d in [r * 0.33, r * 0.66, r]:
			var q: Vector2 = c + Vector2(cos(a), sin(a)) * float(d)
			if gen.height(q.x, q.y) < -0.5:
				return true
	return false


func _update_nuisances() -> void:
	lots.nuisances.clear()
	for id in items:
		if items[id]["type"] == Type.SALADERO:
			lots.nuisances.append(items[id]["c"])


## ¿Hay faroles encendidos de noche en este punto?
func lit_at(p: Vector2) -> bool:
	for id in items:
		var it: Dictionary = items[id]
		if it["type"] == Type.WATCH and (it["c"] as Vector2).distance_to(p) <= float(INFO[Type.WATCH]["radius"]):
			return true
	return false


## Faroles cada ~30 m a los lados de las calles cercanas a cada puesto de serenos.
func rebuild_lamps() -> void:
	var out := PackedVector3Array()
	var dirs := []
	for id in items:
		var it: Dictionary = items[id]
		if it["type"] != Type.WATCH:
			continue
		var c: Vector2 = it["c"]
		var r: float = INFO[Type.WATCH]["radius"]
		for eid in net.edges:
			var e: Dictionary = net.edges[eid]
			var L: float = e["len"]
			var w: float = RoadNetwork.WIDTH[e["type"]]
			var s := 8.0
			var side := 1.0
			while s < L - 4.0:
				var pt: Array = net.edge_point(eid, s)
				var p: Vector2 = pt[0]
				if p.distance_to(c) <= r:
					var tan: Vector2 = pt[1]
					var q := p + Vector2(-tan.y, tan.x) * side * (w * 0.5 + 0.7)
					var dup := false
					for k in range(maxi(0, out.size() - 40), out.size()):
						if Vector2(out[k].x, out[k].z).distance_to(q) < 12.0:
							dup = true
							break
					if not dup:
						out.append(Vector3(q.x, gen.height(q.x, q.y), q.y))
				s += 30.0
				side = -side
	lamps = out
	_lamp_mm.instance_count = out.size()
	for i in out.size():
		_lamp_mm.set_instance_transform(i, Transform3D(Basis(), out[i]))


func _lamp_mesh() -> ArrayMesh:
	var st := SurfaceTool.new()
	st.begin(Mesh.PRIMITIVE_TRIANGLES)
	BuildingGen.box(st, Vector3(0, -0.3, 0), Vector3(0.18, 3.6, 0.18), Color(0.12, 0.11, 0.10))
	BuildingGen.box(st, Vector3(0, 3.2, 0), Vector3(0.55, 0.12, 0.55), Color(0.12, 0.11, 0.10))
	BuildingGen.box(st, Vector3(0, 3.32, 0), Vector3(0.45, 0.6, 0.45), Color(0.95, 0.75, 0.4, 0.0))
	BuildingGen.box(st, Vector3(0, 3.92, 0), Vector3(0.6, 0.15, 0.6), Color(0.12, 0.11, 0.10))
	return st.commit()


func describe(id: int) -> String:
	var it: Dictionary = items[id]
	var info: Dictionary = INFO[it["type"]]
	if it["type"] == Type.WATCH:
		return "%s\n%s\nFaroles a su cargo: %d\nMantenimiento: %d pesos al mes" % [info["name"], info["desc"], lamps.size(), info["upkeep"]]
	if it["type"] == Type.ESTANCIA:
		return "%s\n%s" % [info["name"], info["desc"]]
	if it["type"] == Type.SALADERO:
		return "%s\n%s\nGanado esperando faena: %d\nMantenimiento: %d pesos al mes" % [info["name"], info["desc"], it["stock"], info["upkeep"]]
	if it["type"] == Type.WELL and contaminated(it["c"]):
		return "%s\n¡Agua contaminada por el saladero!\nAbastece a %d personas con agua mala" % [info["name"], it["served"]]
	return "%s\n%s\nAbastece a %d de %d personas\nMantenimiento: %d pesos al mes" % [info["name"], info["desc"], it["served"], info["cap"], info["upkeep"]]


# --- Mallas ------------------------------------------------------------------------

func _mesh(type: int) -> ArrayMesh:
	var st := SurfaceTool.new()
	st.begin(Mesh.PRIMITIVE_TRIANGLES)
	var stone := Color(0.64, 0.61, 0.56)
	var wood := Color(0.40, 0.29, 0.19)
	var cal := Color(0.93, 0.91, 0.85)
	match type:
		Type.WELL:
			BuildingGen.box(st, Vector3(0, -0.5, 0), Vector3(5.0, 0.6, 5.0), stone.darkened(0.1))
			for k in 4:
				var xf := Transform3D(Basis(Vector3.UP, PI * 0.5 * k), Vector3.ZERO)
				BuildingGen.box(st, Vector3(0, 0.1, 0.75), Vector3(1.8, 0.9, 0.3), stone, xf)
			BuildingGen.box(st, Vector3(0, 0.1, 0), Vector3(1.2, 0.85, 1.2), Color(0.12, 0.16, 0.2))
			for x in [-1.1, 1.1]:
				BuildingGen.box(st, Vector3(x, 0.1, 0), Vector3(0.18, 2.6, 0.18), wood)
			BuildingGen.box(st, Vector3(0, 2.6, 0), Vector3(2.5, 0.15, 0.15), wood)
			BuildingGen.box(st, Vector3(0, 1.4, 0), Vector3(0.35, 0.4, 0.35), wood.darkened(0.2))
		Type.CISTERN:
			BuildingGen.box(st, Vector3(0, -0.6, 0), Vector3(10.0, 0.8, 10.0), Color(0.70, 0.66, 0.58))
			BuildingGen.box(st, Vector3(0, 0.2, 0), Vector3(2.4, 1.0, 2.4), cal)
			BuildingGen.box(st, Vector3(0, 1.2, 0), Vector3(2.6, 0.15, 2.6), stone)
			for x in [-1.0, 1.0]:
				BuildingGen.box(st, Vector3(x, 1.35, 0), Vector3(0.1, 1.6, 0.1), Color(0.12, 0.12, 0.12))
			BuildingGen.box(st, Vector3(0, 2.9, 0), Vector3(2.1, 0.1, 0.1), Color(0.12, 0.12, 0.12))
			BuildingGen.box(st, Vector3(0, 2.2, 0), Vector3(0.3, 0.45, 0.3), wood)
		Type.WATCH:
			BuildingGen.box(st, Vector3(0, -0.5, 0), Vector3(5.0, 3.6, 6.0), cal)
			BuildingGen.gable(st, Vector3(0, 3.1, 0), 2.8, 3.3, 1.4, BuildingGen.TILE_ROOF, cal, 2.5, 3.0)
			BuildingGen.box(st, Vector3(0, 0, 3.02), Vector3(1.0, 2.1, 0.1), wood)
			BuildingGen.box(st, Vector3(1.6, 1.8, 3.2), Vector3(0.45, 0.6, 0.45), Color(0.95, 0.75, 0.4, 0.0))
		Type.ESTANCIA:
			var adobe := Color(0.90, 0.86, 0.76)
			BuildingGen.box(st, Vector3(-12, -0.6, 14), Vector3(16.0, 4.2, 7.0), adobe)
			BuildingGen.gable(st, Vector3(-12, 3.6, 14), 8.5, 4.0, 2.2, BuildingGen.TILE_ROOF, adobe, 8.0, 3.5)
			BuildingGen.box(st, Vector3(-12, 0, 17.55), Vector3(1.2, 2.2, 0.1), wood)
			BuildingGen.box(st, Vector3(-12, -0.4, 22.0), Vector3(16.0, 0.4, 3.0), Color(0.55, 0.45, 0.32))
			# Corral de palo a pique.
			for side in 4:
				var xf := Transform3D(Basis(Vector3.UP, PI * 0.5 * side), Vector3(8.0, 0, -6.0))
				for k in 13:
					BuildingGen.box(st, Vector3(-12.0 + k * 2.0, 0, 12.0), Vector3(0.25, 1.7, 0.25), wood, xf)
				BuildingGen.box(st, Vector3(0, 1.1, 12.0), Vector3(24.0, 0.15, 0.15), wood, xf)
			# Ganado.
			for k in 14:
				var cx := 8.0 + float((k * 37) % 19) - 9.0
				var cz := -6.0 + float((k * 53) % 19) - 9.0
				var cow := Transform3D(Basis(Vector3.UP, float(k) * 0.9), Vector3(cx, 0, cz))
				var hide := Color(0.45, 0.30, 0.20) if k % 3 else Color(0.20, 0.16, 0.13)
				BuildingGen.box(st, Vector3(0, 0.5, 0), Vector3(0.7, 0.8, 1.7), hide, cow)
				BuildingGen.box(st, Vector3(0, 0.95, 1.0), Vector3(0.4, 0.4, 0.5), hide, cow)
			# Ombú.
			BuildingGen.box(st, Vector3(-20, 0, -18), Vector3(2.4, 3.5, 2.4), Color(0.40, 0.33, 0.25))
			BuildingGen.box(st, Vector3(-20, 3.0, -18), Vector3(10.0, 3.5, 10.0), Color(0.27, 0.38, 0.17))
			BuildingGen.box(st, Vector3(-20, 6.5, -18), Vector3(6.0, 1.5, 6.0), Color(0.29, 0.40, 0.18))
		Type.SALADERO:
			var brick := Color(0.66, 0.40, 0.30)
			for gx in [-9.0, 9.0]:
				BuildingGen.box(st, Vector3(gx, -0.6, 8.0), Vector3(14.0, 5.0, 24.0), brick)
				var rot := Transform3D(Basis(Vector3.UP, PI * 0.5), Vector3.ZERO)
				BuildingGen.gable(st, Vector3(-8.0, 4.4, gx), 12.5, 7.5, 2.4, BuildingGen.TILE_ROOF, brick, 12.0, 7.0, rot)
			BuildingGen.box(st, Vector3(16.0, 0, 18.0), Vector3(1.6, 14.0, 1.6), brick.darkened(0.2))
			# Tendederos de cueros al sol.
			for row in 4:
				for k in 6:
					var x := -15.0 + k * 6.0
					var z := -8.0 - row * 3.5
					BuildingGen.box(st, Vector3(x, 0, z), Vector3(0.2, 2.2, 0.2), wood)
					BuildingGen.box(st, Vector3(x + 3.0, 0.4, z), Vector3(5.2, 1.6, 0.08), Color(0.62, 0.48, 0.32))
				BuildingGen.box(st, Vector3(0, 2.1, -8.0 - row * 3.5), Vector3(32.0, 0.1, 0.1), wood)
		Type.WATER_CARRIERS:
			BuildingGen.box(st, Vector3(0, -0.8, 0), Vector3(14.0, 1.0, 18.0), Color(0.55, 0.48, 0.36))
			BuildingGen.box(st, Vector3(0, -0.5, -4.0), Vector3(10.0, 3.6, 7.0), Color(0.62, 0.50, 0.36))
			BuildingGen.gable(st, Vector3(0, 3.1, -4.0), 5.4, 3.9, 1.8, BuildingGen.THATCH[0], Color(0.62, 0.50, 0.36), 5.0, 3.5)
			for i in 6:
				BuildingGen.box(st, Vector3(-4.5 + i * 1.6, 0.2, 3.0), Vector3(1.0, 1.3, 1.0), wood)
			BuildingGen.box(st, Vector3(3.5, 0.6, 6.5), Vector3(1.6, 0.4, 3.0), wood)
			for x in [2.6, 4.4]:
				BuildingGen.box(st, Vector3(x, 0.1, 6.5), Vector3(0.12, 1.4, 1.4), wood.darkened(0.3))
			BuildingGen.box(st, Vector3(3.5, 1.0, 6.5), Vector3(1.2, 1.2, 1.6), wood.lightened(0.1))
	return st.commit()
