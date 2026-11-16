extends Node3D
## Muralla colonial. No es defensiva (no hay asedios): marca el límite de la ciudad y
## controla lo que entra. Donde cruza una calle se abre un portón con guardas de aduana:
## las carretas se detienen a ser revisadas y pagan; eso genera colas reales en la entrada.

const TerrainGen = preload("res://scripts/world/terrain_gen.gd")
const Parcels = preload("res://scripts/world/parcels.gd")
const RoadNetwork = preload("res://scripts/roads/road_network.gd")
const Lots = preload("res://scripts/city/lots.gd")
const BuildingGen = preload("res://scripts/city/building_gen.gd")

const COST_PER_M := 18
const UPKEEP_PER_100M := 6
const HEIGHT := 7.0
const THICK := 5.0
const GATE_GAP := 14.0
const INSPECT_SECONDS := 25.0     # segundos simulados que tarda la revisión de una carreta
const DUTY_PER_CART := 6

var gen: TerrainGen
var parcels: Parcels
var net: RoadNetwork
var lots: Lots
var segments: Array = []    # [a: Vector2, b: Vector2]
var gates: Array = []       # {"id", "pos", "dir", "eid", "s"}
var carts_inspected := 0
var duty_month := 0
var _seg_nodes: Array = []
var _gate_nodes: Array = []


func setup(g: TerrainGen, p: Parcels, n: RoadNetwork, l: Lots) -> void:
	gen = g
	parcels = p
	net = n
	lots = l
	net.changed.connect(update_gates)


func total_length() -> float:
	var l := 0.0
	for s in segments:
		l += (s[0] as Vector2).distance_to(s[1])
	return l


func problem(a: Vector2, b: Vector2) -> String:
	if a.distance_to(b) < 10.0:
		return "Demasiado corta"
	var n := int(a.distance_to(b) / 8.0) + 1
	for i in n + 1:
		var p := a.lerp(b, float(i) / n)
		if not parcels.is_owned_pos(p):
			return "Fuera de tus parcelas"
		if gen.height(p.x, p.y) < 0.5:
			return "No se puede levantar sobre el agua"
	return ""


func cost(a: Vector2, b: Vector2) -> int:
	return int(a.distance_to(b) * COST_PER_M)


func add_segment(a: Vector2, b: Vector2) -> void:
	segments.append([a, b])
	var u := (b - a).normalized()
	var c := (a + b) * 0.5
	var len := a.distance_to(b)
	lots.clear_area(Lots.corners_of(c, u, THICK + 2.0, len))
	lots.obstacles.append({"c": c, "f": u, "w": THICK + 2.0, "d": len, "wall": true})
	update_gates()


## Recalcula los portones: un portón donde cada tramo de muralla cruza una calle.
func update_gates() -> void:
	gates.clear()
	for s in segments:
		var a: Vector2 = s[0]
		var b: Vector2 = s[1]
		for eid in net.edges:
			var pts: PackedVector2Array = net.edges[eid]["pts"]
			var cum: PackedFloat32Array = net.edges[eid]["cum"]
			for i in pts.size() - 1:
				var x = Geometry2D.segment_intersects_segment(a, b, pts[i], pts[i + 1])
				if x == null:
					continue
				var pt: Vector2 = x
				var dup := false
				for g in gates:
					if (g["pos"] as Vector2).distance_to(pt) < GATE_GAP:
						dup = true
				if not dup:
					gates.append({"id": gates.size(), "pos": pt, "dir": (b - a).normalized(), "eid": eid, "s": cum[i] + pts[i].distance_to(pt)})
	_rebuild_meshes()


## Portón que está por delante en este tramo, en el sentido de la marcha.
## Devuelve [id, distancia en `s` de marcha] o [] si no hay.
func gate_ahead(eid: int, dir: int, s: float, length: float) -> Array:
	var best := []
	for g in gates:
		if g["eid"] != eid:
			continue
		var gs: float = g["s"] if dir == 0 else length - float(g["s"])
		if gs >= s - 0.5 and (best.is_empty() or gs < float(best[1])):
			best = [g["id"], gs]
	return best


func inspected() -> void:
	carts_inspected += 1
	duty_month += DUTY_PER_CART


func take_duty() -> int:
	var d := duty_month
	duty_month = 0
	return d


func upkeep() -> int:
	return int(total_length() / 100.0 * UPKEEP_PER_100M)


func _rebuild_meshes() -> void:
	for n in _seg_nodes + _gate_nodes:
		(n as Node).queue_free()
	_seg_nodes.clear()
	_gate_nodes.clear()
	var stone := Color(0.60, 0.57, 0.52)
	for s in segments:
		var a: Vector2 = s[0]
		var b: Vector2 = s[1]
		var u := (b - a).normalized()
		# Corta el tramo donde hay portones.
		var cuts := [0.0]
		var total := a.distance_to(b)
		var on_seg := []
		for g in gates:
			var gp: Vector2 = g["pos"]
			var t := (gp - a).dot(u)
			if t > 0.0 and t < total and Geometry2D.get_closest_point_to_segment(gp, a, b).distance_to(gp) < 0.5:
				on_seg.append(t)
		on_seg.sort()
		var pieces := []
		var start := 0.0
		for t in on_seg:
			pieces.append([start, float(t) - GATE_GAP * 0.5])
			start = float(t) + GATE_GAP * 0.5
		pieces.append([start, total])
		for pc in pieces:
			var l0: float = pc[0]
			var l1: float = pc[1]
			if l1 - l0 < 1.0:
				continue
			var st := SurfaceTool.new()
			st.begin(Mesh.PRIMITIVE_TRIANGLES)
			var len := l1 - l0
			var mid := a + u * ((l0 + l1) * 0.5)
			var base := minf(gen.height(a.x + u.x * l0, a.y + u.y * l0), gen.height(a.x + u.x * l1, a.y + u.y * l1))
			base = minf(base, gen.height(mid.x, mid.y))
			BuildingGen.box(st, Vector3(0, -3.0, 0), Vector3(THICK, HEIGHT + 3.0, len), stone)
			# Almenas.
			var k := 0.0
			while k < len - 1.5:
				BuildingGen.box(st, Vector3(THICK * 0.5 - 0.4, HEIGHT, -len * 0.5 + k + 0.75), Vector3(0.8, 1.1, 1.5), stone.darkened(0.05))
				k += 3.0
			var mi := MeshInstance3D.new()
			mi.mesh = st.commit()
			mi.material_override = BuildingGen.material()
			var side := Vector2(u.y, -u.x)
			mi.transform = Transform3D(Basis(Vector3(side.x, 0, side.y), Vector3.UP, Vector3(u.x, 0, u.y)), Vector3(mid.x, base, mid.y))
			add_child(mi)
			_seg_nodes.append(mi)
	for g in gates:
		var gp: Vector2 = g["pos"]
		var u: Vector2 = g["dir"]
		var side := Vector2(u.y, -u.x)
		var st := SurfaceTool.new()
		st.begin(Mesh.PRIMITIVE_TRIANGLES)
		for sgn in [-1.0, 1.0]:
			BuildingGen.box(st, Vector3(0, -3.0, sgn * (GATE_GAP * 0.5 - 1.8)), Vector3(THICK + 2.0, HEIGHT + 6.0, 3.6), stone.lightened(0.05))
			BuildingGen.box(st, Vector3(0, HEIGHT + 3.0, sgn * (GATE_GAP * 0.5 - 1.8)), Vector3(THICK + 2.6, 0.6, 4.2), stone.darkened(0.1))
		BuildingGen.box(st, Vector3(0, HEIGHT - 1.0, 0), Vector3(THICK + 1.0, 3.0, GATE_GAP), stone)
		BuildingGen.box(st, Vector3(0, HEIGHT + 2.0, 0), Vector3(1.4, 1.6, 2.4), Color(0.93, 0.91, 0.85))
		var mi := MeshInstance3D.new()
		mi.mesh = st.commit()
		mi.material_override = BuildingGen.material()
		mi.transform = Transform3D(Basis(Vector3(side.x, 0, side.y), Vector3.UP, Vector3(u.x, 0, u.y)), Vector3(gp.x, gen.height(gp.x, gp.y), gp.y))
		add_child(mi)
		_gate_nodes.append(mi)
