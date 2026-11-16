extends Node3D
## Servicios de agua de la época colonial: pozos de balde, aljibes públicos y puestos de
## aguadores. Cada solar edificado recibe agua de la fuente más buena que tenga cerca y con
## capacidad libre. Sin agua, las casas no mejoran y los incendios no se pueden apagar.

const TerrainGen = preload("res://scripts/world/terrain_gen.gd")
const Parcels = preload("res://scripts/world/parcels.gd")
const RoadNetwork = preload("res://scripts/roads/road_network.gd")
const Lots = preload("res://scripts/city/lots.gd")
const BuildingGen = preload("res://scripts/city/building_gen.gd")

enum Type { WELL, CISTERN, WATER_CARRIERS, WATCH, ESTANCIA, SALADERO, LAZARETO, HORNO, ATAHONA, CANTERA }
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
	Type.LAZARETO: {"name": "Lazareto", "cost": 3000, "upkeep": 25, "cap": 0, "quality": 0.0, "radius": 0.0, "size": Vector2(30, 24),
		"desc": "Cuarentena para los barcos con fiebre a bordo: la enfermedad no entra a la ciudad. Lejos de la plaza y junto a la costa."},
	Type.HORNO: {"name": "Horno de ladrillos", "cost": 2000, "upkeep": 10, "cap": 0, "quality": 0.0, "radius": 0.0, "size": Vector2(24, 24),
		"desc": "Cuece ladrillos con barro del arroyo o la costa (a menos de 300 m). Las casas de azotea, las de altos y la iglesia los necesitan. Echa humo: lejos de la plaza."},
	Type.ATAHONA: {"name": "Atahona", "cost": 1500, "upkeep": 10, "cap": 0, "quality": 0.0, "radius": 0.0, "size": Vector2(16, 16),
		"desc": "Molino movido por mulas: muele el trigo de las chacras y da pan para unas 1.500 personas. Sin pan, los pobladores no se quedan."},
	Type.CANTERA: {"name": "Cantera de piedra", "cost": 1800, "upkeep": 10, "cap": 0, "quality": 0.0, "radius": 0.0, "size": Vector2(28, 22),
		"desc": "Saca piedra para empedrar las calles. Ruidosa y polvorienta: a más de 300 m de la plaza."},
}
const CONTAMINATION := 300.0
const STONE_PER_QUARRY := 60
const LAMP_UPKEEP := 0.5   # sebo por farol de calle y por mes
const BRICKS_PER_KILN := 40
const BREAD_PER_MILL := 1500

## Ampliaciones: los edificios crecen por piezas en lugar de demolerse y reconstruirse.
## Cada módulo suma capacidad, alcance o viajes, y se ve en el modelo.
const MODULES := {
	Type.WELL: [
		{"id": "brocal", "name": "Brocal de piedra con techito", "cost": 300, "cap": 25},
	],
	Type.CISTERN: [
		{"id": "cisterna2", "name": "Segunda cisterna", "cost": 900, "cap": 120},
		{"id": "cisterna3", "name": "Tercera cisterna", "cost": 1100, "cap": 120, "needs": "cisterna2"},
		{"id": "canaletas", "name": "Canaletas desde las azoteas vecinas", "cost": 600, "radius": 50.0},
	],
	Type.WATER_CARRIERS: [
		{"id": "carros", "name": "Más carros y mulas", "cost": 1200, "cap": 200, "carts": 3, "upkeep": 10},
		{"id": "noria", "name": "Noria en el arroyo", "cost": 1800, "cap": 250, "radius": 300.0},
	],
	Type.WATCH: [
		{"id": "serenos", "name": "Más serenos", "cost": 600, "serenos": 2, "radius": 90.0, "upkeep": 10},
		{"id": "faroles", "name": "Faroles de vidrio en las esquinas", "cost": 800, "radius": 80.0},
	],
	Type.ESTANCIA: [
		{"id": "corral2", "name": "Segundo corral", "cost": 1200, "trips": 2},
		{"id": "puesto", "name": "Puesto de peones con mangrullo", "cost": 900, "trips": 2},
	],
	Type.HORNO: [
		{"id": "horno2", "name": "Segundo horno", "cost": 1500, "bricks": 40},
	],
	Type.ATAHONA: [
		{"id": "piedra2", "name": "Segunda piedra de moler", "cost": 1200, "bread": 1500},
	],
	Type.CANTERA: [
		{"id": "frente2", "name": "Segundo frente de cantera", "cost": 1200, "stone": 60},
	],
	Type.SALADERO: [
		{"id": "galpon", "name": "Galpón de salazón nuevo", "cost": 2000, "trips": 2},
		{"id": "graseria", "name": "Grasería (sebo y velas)", "cost": 2500, "trips": 2, "upkeep": 10},
	],
}
## Modernización: el edificio pasa a su versión mejor sin cambiar de lugar.
const MODERNIZE := {Type.WELL: Type.CISTERN}
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
# Efectos de las ordenanzas (los fija main cada mes).
var water_factor := 1.0
var private_cisterns := false
var watch_free := false
var trip_bonus := 0


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
	if type == Type.LAZARETO:
		if lots.plaza_center.x != INF and c.distance_to(lots.plaza_center) < 400.0:
			return "El lazareto va lejos de la ciudad (a más de 400 m de la plaza)"
		if not _near_coast(c, 150.0):
			return "El lazareto va junto a la costa (a menos de 150 m)"
	if type == Type.CANTERA and lots.plaza_center.x != INF and c.distance_to(lots.plaza_center) < 300.0:
		return "La cantera es ruidosa: a más de 300 m de la plaza"
	if type == Type.HORNO:
		if lots.plaza_center.x != INF and c.distance_to(lots.plaza_center) < 250.0:
			return "El horno echa humo: a más de 250 m de la plaza"
		if gen.arroyo_distance(c.x, c.y) > 300.0 and not _near_coast(c, 300.0):
			return "El horno necesita barro: un arroyo o la costa a menos de 300 m"
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
	mi.mesh = _mesh(type, [])
	mi.material_override = BuildingGen.material()
	var base := INF
	for p in corners(type, c, f):
		base = minf(base, gen.height(p.x, p.y))
	var s := Lots.side_of(f)
	mi.transform = Transform3D(Basis(Vector3(s.x, 0, s.y), Vector3.UP, Vector3(f.x, 0, f.y)), Vector3(c.x, base, c.y))
	add_child(mi)
	items[id] = {"id": id, "type": type, "c": c, "f": f, "node": mi, "served": 0, "stock": 0, "modules": []}
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
		out.append([it["type"], it["c"], atan2((it["f"] as Vector2).x, (it["f"] as Vector2).y), it["stock"], it["modules"]])
	return out


func upkeep() -> int:
	var total := 0
	for id in items:
		if watch_free and items[id]["type"] == Type.WATCH:
			continue
		total += int(INFO[items[id]["type"]]["upkeep"]) + int(module_sum(id, "upkeep"))
	if not watch_free:
		total += int(street_lamp_count() * LAMP_UPKEEP)
	return total


func street_lamp_count() -> int:
	var n := 0
	for eid in net.edges:
		if net.edges[eid].get("lamps", false):
			n += int(float(net.edges[eid]["len"]) / 30.0) + 1
	return n


func stone_per_month() -> int:
	var total := 0
	for id in of_type(Type.CANTERA):
		total += STONE_PER_QUARRY + int(module_sum(id, "stone"))
	return total


# --- Ampliar y modernizar ------------------------------------------------------------

func module_sum(id: int, key: String) -> float:
	var it: Dictionary = items[id]
	var total := 0.0
	for m in MODULES.get(it["type"], []):
		if (it["modules"] as Array).has(m["id"]):
			total += float(m.get(key, 0.0))
	return total


func cap_of(id: int) -> int:
	return int((int(INFO[items[id]["type"]]["cap"]) + int(module_sum(id, "cap"))) * water_factor)


func radius_of(id: int) -> float:
	return float(INFO[items[id]["type"]]["radius"]) + module_sum(id, "radius")


## Acciones disponibles al inspeccionar: [texto, se puede, acción, argumento].
func actions_for(id: int, money: int) -> Array:
	var it: Dictionary = items[id]
	var out := []
	for m in MODULES.get(it["type"], []):
		if (it["modules"] as Array).has(m["id"]):
			continue
		var needs: String = m.get("needs", "")
		if needs != "" and not (it["modules"] as Array).has(needs):
			continue
		out.append(["Ampliar: %s (%s pesos)" % [m["name"], _fmt(m["cost"])], money >= int(m["cost"]), "module", m["id"]])
	if MODERNIZE.has(it["type"]):
		var to: int = MODERNIZE[it["type"]]
		var cost := modernize_cost(id)
		var err := _modernize_problem(id)
		out.append(["Modernizar a %s (%s pesos)%s" % [String(INFO[to]["name"]).to_lower(), _fmt(cost), (" · " + err) if err != "" else ""], money >= cost and err == "", "modernize", to])
	return out


func module_cost(id: int, mid: String) -> int:
	for m in MODULES.get(items[id]["type"], []):
		if m["id"] == mid:
			return m["cost"]
	return 0


func add_module(id: int, mid: String) -> void:
	(items[id]["modules"] as Array).append(mid)
	_rebuild_mesh(id)
	if items[id]["type"] == Type.WATCH:
		rebuild_lamps()


func modernize_cost(id: int) -> int:
	var to: int = MODERNIZE[items[id]["type"]]
	return maxi(200, int(INFO[to]["cost"]) - int(INFO[items[id]["type"]]["cost"]) / 2)


func _modernize_problem(id: int) -> String:
	var it: Dictionary = items[id]
	var to: int = MODERNIZE[it["type"]]
	var cs := corners(to, it["c"], it["f"])
	for p in cs:
		if net.clearance(p, 20.0) < 0.5:
			return "no hay lugar (choca con una calle)"
	for ob in lots.obstacles:
		if (ob["c"] as Vector2).distance_to(it["c"]) < 0.01:
			continue
		if Lots.polys_overlap(cs, Lots.corners_of(ob["c"], ob["f"], ob["w"], ob["d"])):
			return "no hay lugar"
	return ""


## Convierte el edificio en su versión moderna en el mismo lugar (por ejemplo, pozo → aljibe).
func modernize(id: int) -> void:
	var it: Dictionary = items[id]
	var to: int = MODERNIZE[it["type"]]
	var size: Vector2 = INFO[to]["size"]
	lots.clear_area(corners(to, it["c"], it["f"]))
	for ob in lots.obstacles:
		if (ob["c"] as Vector2).distance_to(it["c"]) < 0.01:
			ob["w"] = size.x
			ob["d"] = size.y
	it["type"] = to
	it["modules"] = []
	_rebuild_mesh(id)


func _rebuild_mesh(id: int) -> void:
	var it: Dictionary = items[id]
	var mi: MeshInstance3D = it["node"]
	mi.mesh = _mesh(it["type"], it["modules"])
	var base := INF
	for p in corners(it["type"], it["c"], it["f"]):
		base = minf(base, gen.height(p.x, p.y))
	mi.position.y = base


static func _fmt(n: int) -> String:
	var t := str(n)
	return t.substr(0, t.length() - 3) + "." + t.substr(t.length() - 3) if t.length() > 3 else t


func _modules_text(id: int) -> String:
	var names := []
	for m in MODULES.get(items[id]["type"], []):
		if (items[id]["modules"] as Array).has(m["id"]):
			names.append(m["name"])
	return ("\nAmpliaciones: " + ", ".join(names)) if not names.is_empty() else ""


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
		var radius: float = radius_of(sid)
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
			if used + need > cap_of(sid):
				break
			used += need
			lot["water"] = quality
			lot["water_src"] = sid
		it["served"] = used
	# Ordenanza: las casas de altos tienen su propio aljibe.
	if private_cisterns:
		for id in lots.lots:
			if lots.lots[id]["level"] == 3:
				lots.lots[id]["water"] = maxf(float(lots.lots[id]["water"]), 1.0)


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


## Ladrillos que cuecen por mes todos los hornos.
func bricks_per_month() -> int:
	var total := 0
	for id in of_type(Type.HORNO):
		total += BRICKS_PER_KILN + int(module_sum(id, "bricks"))
	return total


## Personas a las que alcanza el pan (las chacras privadas ya dan para algunas).
func bread_capacity() -> int:
	var total := 600
	for id in of_type(Type.ATAHONA):
		total += BREAD_PER_MILL + int(module_sum(id, "bread"))
	return total


## ¿Hay faroles encendidos de noche en este punto?
func lit_at(p: Vector2) -> bool:
	for id in items:
		var it: Dictionary = items[id]
		if it["type"] == Type.WATCH and (it["c"] as Vector2).distance_to(p) <= radius_of(id):
			return true
	return false


## Faroles cada ~30 m a los lados de las calles cercanas a cada puesto de serenos.
func rebuild_lamps() -> void:
	var out := PackedVector3Array()
	var dirs := []
	for eid in net.edges:
		var e2: Dictionary = net.edges[eid]
		if not e2.get("lamps", false):
			continue
		var w2: float = RoadNetwork.WIDTH[e2["type"]]
		var s2 := 6.0
		var side2 := 1.0
		while s2 < float(e2["len"]) - 2.0:
			var pt2: Array = net.edge_point(eid, s2)
			var tan2: Vector2 = pt2[1]
			var q2: Vector2 = (pt2[0] as Vector2) + Vector2(-tan2.y, tan2.x) * side2 * (w2 * 0.5 + 0.7)
			out.append(Vector3(q2.x, gen.height(q2.x, q2.y), q2.y))
			s2 += 30.0
			side2 = -side2
	for id in items:
		var it: Dictionary = items[id]
		if it["type"] != Type.WATCH:
			continue
		var c: Vector2 = it["c"]
		var r: float = radius_of(id)
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
		return "%s\n%s\nFaroles a su cargo: %d · serenos: %d%s" % [info["name"], info["desc"], lamps.size(), 2 + int(module_sum(id, "serenos")), _modules_text(id)]
	if it["type"] == Type.LAZARETO or it["type"] == Type.HORNO or it["type"] == Type.ATAHONA or it["type"] == Type.CANTERA:
		var extra := ""
		if it["type"] == Type.CANTERA:
			extra = "\nPiedra por mes: %d" % (STONE_PER_QUARRY + int(module_sum(id, "stone")))
		if it["type"] == Type.HORNO:
			extra = "\nLadrillos por mes: %d" % (BRICKS_PER_KILN + int(module_sum(id, "bricks")))
		elif it["type"] == Type.ATAHONA:
			extra = "\nPan para %d personas" % (BREAD_PER_MILL + int(module_sum(id, "bread")))
		return "%s\n%s%s%s" % [info["name"], info["desc"], extra, _modules_text(id)]
	if it["type"] == Type.ESTANCIA:
		return "%s\n%s\nCarretas de ganado por día: %d%s" % [info["name"], info["desc"], 3 + int(module_sum(id, "trips")), _modules_text(id)]
	if it["type"] == Type.SALADERO:
		return "%s\n%s\nGanado esperando faena: %d · carretas de cueros por día: %d%s" % [info["name"], info["desc"], it["stock"], 3 + int(module_sum(id, "trips")), _modules_text(id)]
	if it["type"] == Type.WELL and contaminated(it["c"]):
		return "%s\n¡Agua contaminada por el saladero!\nAbastece a %d personas con agua mala" % [info["name"], it["served"]]
	return "%s\n%s\nAbastece a %d de %d personas · alcance %d m%s\nMantenimiento: %d pesos al mes" % [info["name"], info["desc"], it["served"], cap_of(id), int(radius_of(id)), _modules_text(id), int(info["upkeep"]) + int(module_sum(id, "upkeep"))]


# --- Mallas ------------------------------------------------------------------------

func _mesh(type: int, modules: Array) -> ArrayMesh:
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
		Type.LAZARETO:
			BuildingGen.box(st, Vector3(0, -0.6, -4.0), Vector3(26.0, 4.6, 8.0), cal)
			BuildingGen.gable(st, Vector3(0, 4.0, -4.0), 13.4, 4.4, 2.0, BuildingGen.TILE_ROOF, cal, 13.0, 4.0)
			BuildingGen.box(st, Vector3(-9.0, -0.6, 5.0), Vector3(6.0, 6.5, 6.0), cal)
			BuildingGen.gable(st, Vector3(-9.0, 5.9, 5.0), 3.3, 3.3, 1.8, BuildingGen.TILE_ROOF, cal, 3.0, 3.0)
			BuildingGen.box(st, Vector3(-9.0, 7.6, 7.9), Vector3(0.3, 1.4, 0.3), Color(0.2, 0.2, 0.2))
			for k in 6:
				BuildingGen.box(st, Vector3(-10.0 + k * 4.0, 1.0, 0.03), Vector3(1.0, 1.6, 0.1), BuildingGen.WINDOW)
			for side in 4:
				var xf := Transform3D(Basis(Vector3.UP, PI * 0.5 * side), Vector3.ZERO)
				BuildingGen.box(st, Vector3(0, -0.5, 11.5 if side % 2 == 0 else 14.5), Vector3(29.0 if side % 2 == 0 else 23.0, 2.4, 0.5), stone, xf)
		Type.HORNO:
			var brick := Color(0.66, 0.38, 0.28)
			BuildingGen.box(st, Vector3(-4.0, -0.5, -3.0), Vector3(8.0, 4.5, 8.0), brick.darkened(0.15))
			BuildingGen.box(st, Vector3(-4.0, 4.0, -3.0), Vector3(6.0, 1.2, 6.0), brick.darkened(0.25))
			BuildingGen.box(st, Vector3(-4.0, 5.2, -3.0), Vector3(1.4, 4.0, 1.4), brick.darkened(0.3))
			BuildingGen.box(st, Vector3(-4.0, 0.0, 1.05), Vector3(1.6, 1.4, 0.1), Color(0.9, 0.5, 0.2, 0.0))
			for k in 4:
				BuildingGen.box(st, Vector3(4.0 + (k % 2) * 4.0, 0.0, -6.0 + (k / 2) * 5.0), Vector3(3.0, 1.4, 2.0), brick)
			BuildingGen.box(st, Vector3(5.0, -0.3, 7.0), Vector3(8.0, 0.5, 6.0), Color(0.45, 0.36, 0.28))
		Type.ATAHONA:
			var adobe: Color = BuildingGen.ADOBE[1]
			BuildingGen.box(st, Vector3(0, -0.5, -3.0), Vector3(8.0, 4.0, 6.0), adobe)
			BuildingGen.gable(st, Vector3(0, 3.5, -3.0), 4.4, 3.4, 1.8, BuildingGen.THATCH[1], adobe, 4.0, 3.0)
			BuildingGen.box(st, Vector3(0, -0.3, 4.0), Vector3(9.0, 0.4, 9.0), Color(0.52, 0.42, 0.30))
			BuildingGen.box(st, Vector3(0, 0.1, 4.0), Vector3(1.8, 0.8, 1.8), Color(0.62, 0.60, 0.56))
			BuildingGen.box(st, Vector3(0, 0.9, 4.0), Vector3(0.25, 0.25, 7.0), Color(0.40, 0.29, 0.19))
			BuildingGen.box(st, Vector3(0, 0.0, 7.2), Vector3(0.6, 1.3, 1.5), Color(0.40, 0.30, 0.22))
		Type.CANTERA:
			var rock := Color(0.58, 0.56, 0.52)
			BuildingGen.box(st, Vector3(-6.0, -1.0, -4.0), Vector3(14.0, 6.0, 10.0), rock.darkened(0.1))
			BuildingGen.box(st, Vector3(-8.0, 5.0, -6.0), Vector3(8.0, 2.5, 6.0), rock.darkened(0.2))
			for k in 6:
				BuildingGen.box(st, Vector3(3.0 + (k % 3) * 2.4, 0.0, 2.0 + (k / 3) * 2.4), Vector3(1.6, 0.9, 1.6), rock)
			BuildingGen.box(st, Vector3(7.0, 0.4, -5.0), Vector3(1.6, 0.6, 3.0), wood)
			BuildingGen.box(st, Vector3(7.0, 0.0, -5.0), Vector3(1.9, 1.0, 0.15), wood.darkened(0.3))
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
	_module_geometry(st, modules, stone, wood, cal)
	return st.commit()


## Lo que se agrega al modelo con cada ampliación.
func _module_geometry(st: SurfaceTool, modules: Array, stone: Color, wood: Color, cal: Color) -> void:
	var tile: Color = BuildingGen.TILE_ROOF
	for mid in modules:
		match mid:
			"brocal":
				for x in [-1.3, 1.3]:
					for z in [-1.3, 1.3]:
						BuildingGen.box(st, Vector3(x, 0.1, z), Vector3(0.2, 2.6, 0.2), wood)
				BuildingGen.gable(st, Vector3(0, 2.7, 0), 1.8, 1.8, 0.9, tile, wood, 1.5, 1.5)
			"cisterna2", "cisterna3":
				var x := 3.0 if mid == "cisterna2" else -3.0
				BuildingGen.box(st, Vector3(x, 0.2, -3.0), Vector3(2.2, 1.0, 2.2), cal)
				BuildingGen.box(st, Vector3(x, 1.2, -3.0), Vector3(2.4, 0.15, 2.4), stone)
				BuildingGen.box(st, Vector3(x, 1.35, -3.0), Vector3(1.6, 1.4, 0.1), Color(0.12, 0.12, 0.12))
			"canaletas":
				for k in 4:
					var xf := Transform3D(Basis(Vector3.UP, PI * 0.5 * k), Vector3.ZERO)
					BuildingGen.box(st, Vector3(0, 0.25, 3.0), Vector3(0.4, 0.2, 4.0), stone.darkened(0.2), xf)
			"carros":
				for k in 3:
					BuildingGen.box(st, Vector3(-4.0 + k * 2.6, 0.4, 6.5), Vector3(1.4, 1.1, 2.4), wood.lightened(0.1))
					BuildingGen.box(st, Vector3(-4.0 + k * 2.6, 0.0, 8.0), Vector3(0.5, 1.4, 1.4), Color(0.40, 0.30, 0.22))
			"noria":
				BuildingGen.box(st, Vector3(-6.5, 0.0, -1.0), Vector3(0.4, 6.0, 6.0), wood.darkened(0.2))
				BuildingGen.box(st, Vector3(-6.5, 2.6, -1.0), Vector3(1.4, 0.8, 0.8), wood)
			"serenos":
				BuildingGen.box(st, Vector3(-2.0, -0.5, -2.5), Vector3(2.4, 3.0, 2.4), cal)
				BuildingGen.gable(st, Vector3(-2.0, 2.5, -2.5), 1.5, 1.5, 0.8, tile, cal, 1.2, 1.2)
			"faroles":
				for x in [-2.6, 2.6]:
					BuildingGen.box(st, Vector3(x, 0, 3.6), Vector3(0.15, 3.4, 0.15), Color(0.12, 0.11, 0.10))
					BuildingGen.box(st, Vector3(x, 3.4, 3.6), Vector3(0.45, 0.55, 0.45), Color(0.95, 0.75, 0.4, 0.0))
			"corral2":
				for side in 4:
					var xf := Transform3D(Basis(Vector3.UP, PI * 0.5 * side), Vector3(-14.0, 0, -12.0))
					for k in 7:
						BuildingGen.box(st, Vector3(-6.0 + k * 2.0, 0, 7.0), Vector3(0.25, 1.7, 0.25), wood, xf)
					BuildingGen.box(st, Vector3(0, 1.1, 7.0), Vector3(14.0, 0.15, 0.15), wood, xf)
				for k in 6:
					BuildingGen.box(st, Vector3(-17.0 + (k % 3) * 3.0, 0.5, -15.0 + (k / 3) * 5.0), Vector3(0.7, 0.8, 1.7), Color(0.45, 0.30, 0.20))
			"puesto":
				BuildingGen.box(st, Vector3(20.0, -0.5, 20.0), Vector3(6.0, 3.2, 5.0), BuildingGen.ADOBE[0])
				BuildingGen.gable(st, Vector3(20.0, 2.7, 20.0), 3.3, 2.8, 1.6, BuildingGen.THATCH[0], BuildingGen.ADOBE[0], 3.0, 2.5)
				for x in [24.5, 26.5]:
					for z in [17.5, 19.5]:
						BuildingGen.box(st, Vector3(x, 0, z), Vector3(0.25, 8.0, 0.25), wood)
				BuildingGen.box(st, Vector3(25.5, 8.0, 18.5), Vector3(2.6, 0.2, 2.6), wood)
			"horno2":
				BuildingGen.box(st, Vector3(-4.0, -0.5, 6.0), Vector3(7.0, 4.0, 7.0), Color(0.56, 0.32, 0.24))
				BuildingGen.box(st, Vector3(-4.0, 3.5, 6.0), Vector3(1.2, 3.5, 1.2), Color(0.46, 0.27, 0.20))
			"piedra2":
				BuildingGen.box(st, Vector3(5.5, 0.1, -3.0), Vector3(1.8, 0.8, 1.8), Color(0.62, 0.60, 0.56))
				BuildingGen.box(st, Vector3(5.5, 0.9, -3.0), Vector3(0.25, 0.25, 6.0), Color(0.40, 0.29, 0.19))
			"frente2":
				BuildingGen.box(st, Vector3(6.0, -1.0, -6.0), Vector3(8.0, 4.0, 6.0), Color(0.50, 0.48, 0.45))
			"galpon":
				BuildingGen.box(st, Vector3(0, -0.6, -20.0), Vector3(30.0, 4.0, 4.0), Color(0.66, 0.40, 0.30))
				BuildingGen.gable(st, Vector3(0, 3.4, -20.0), 15.3, 2.4, 1.4, tile, Color(0.66, 0.40, 0.30), 15.0, 2.0)
			"graseria":
				BuildingGen.box(st, Vector3(-15.0, -0.5, 18.0), Vector3(5.0, 4.0, 6.0), Color(0.55, 0.36, 0.28))
				BuildingGen.box(st, Vector3(-15.0, 3.5, 19.5), Vector3(1.2, 7.0, 1.2), Color(0.45, 0.30, 0.24))
				BuildingGen.box(st, Vector3(-12.0, 0.0, 18.0), Vector3(1.8, 1.6, 1.8), Color(0.25, 0.22, 0.20))
