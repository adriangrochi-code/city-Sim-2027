extends RefCounted
## Red de calles como grafo: nodos (cruces y extremos) y tramos (polilíneas).
## Las calles se dibujan como curvas (Bézier cuadrática) y se guardan muestreadas.
## Al cruzar o tocar otra calle se crean los cruces automáticamente.

signal changed

enum Type { DIRT, STONE }
const NAMES := {Type.DIRT: "Camino de tierra", Type.STONE: "Calle empedrada"}
const WIDTH := {Type.DIRT: 6.0, Type.STONE: 8.0}
const MIN_RADIUS := {Type.DIRT: 14.0, Type.STONE: 22.0}
const COST_PER_M := {Type.DIRT: 2.0, Type.STONE: 7.0}
const SNAP := 9.0
const STEP := 4.0
const CELL := 40.0

var nodes := {}        # id -> Vector2
var node_edges := {}   # id -> Array de ids de tramos
var edges := {}        # id -> {"a", "b", "pts": PackedVector2Array, "type"}
var _next_node := 1
var _next_edge := 1
var _hash := {}        # Vector2i -> Array de [id_tramo, índice_segmento]


# --- Geometría ------------------------------------------------------------

static func bezier(p0: Vector2, c: Vector2, p1: Vector2) -> PackedVector2Array:
	var approx := p0.distance_to(c) + c.distance_to(p1)
	var n := maxi(1, int(ceil(approx / STEP)))
	var out := PackedVector2Array()
	for i in n + 1:
		var t := float(i) / n
		var u := 1.0 - t
		out.append(p0 * (u * u) + c * (2.0 * u * t) + p1 * (t * t))
	return out


static func line(p0: Vector2, p1: Vector2) -> PackedVector2Array:
	return bezier(p0, (p0 + p1) * 0.5, p1)


static func polyline_length(pts: PackedVector2Array) -> float:
	var l := 0.0
	for i in pts.size() - 1:
		l += pts[i].distance_to(pts[i + 1])
	return l


static func min_radius(pts: PackedVector2Array) -> float:
	var best := INF
	for i in range(1, pts.size() - 1):
		var a := pts[i - 1]
		var b := pts[i]
		var c := pts[i + 1]
		var area2 := absf((b - a).cross(c - a))
		if area2 < 1e-3:
			continue
		best = minf(best, a.distance_to(b) * b.distance_to(c) * c.distance_to(a) / (2.0 * area2))
	return best


static func point_at(pts: PackedVector2Array, t: float) -> Vector2:
	var i := clampi(int(floor(t)), 0, pts.size() - 2)
	return pts[i].lerp(pts[i + 1], clampf(t - i, 0.0, 1.0))


static func sub_polyline(pts: PackedVector2Array, t0: float, t1: float) -> PackedVector2Array:
	var out := PackedVector2Array([point_at(pts, t0)])
	for i in range(int(floor(t0)) + 1, int(ceil(t1))):
		if i >= 0 and i < pts.size() and pts[i].distance_to(out[out.size() - 1]) > 0.05:
			out.append(pts[i])
	var last := point_at(pts, t1)
	if last.distance_to(out[out.size() - 1]) > 0.05:
		out.append(last)
	return out


# --- Consultas -------------------------------------------------------------

func width_of(eid: int) -> float:
	return WIDTH[edges[eid]["type"]]


## Busca a qué engancharse cerca de un punto: un nodo, un tramo o nada.
func snap(p: Vector2) -> Dictionary:
	var best_node := -1
	var bd := SNAP
	for id in nodes:
		var d := (nodes[id] as Vector2).distance_to(p)
		if d < bd:
			bd = d
			best_node = id
	if best_node != -1:
		return {"pos": nodes[best_node], "node": best_node, "edge": -1, "seg": -1}
	var e := nearest_edge(p, SNAP)
	if e["edge"] != -1:
		return {"pos": e["point"], "node": -1, "edge": e["edge"], "seg": e["seg"]}
	return {"pos": p, "node": -1, "edge": -1, "seg": -1}


func nearest_edge(p: Vector2, maxd: float) -> Dictionary:
	var res := {"edge": -1, "seg": -1, "point": p, "dist": maxd}
	var c := _cell(p)
	var r := int(ceil(maxd / CELL))
	for dy in range(-r, r + 1):
		for dx in range(-r, r + 1):
			var k := c + Vector2i(dx, dy)
			if not _hash.has(k):
				continue
			for entry in _hash[k]:
				var eid: int = entry[0]
				var si: int = entry[1]
				var pts: PackedVector2Array = edges[eid]["pts"]
				var q := Geometry2D.get_closest_point_to_segment(p, pts[si], pts[si + 1])
				var d := p.distance_to(q)
				if d < float(res["dist"]):
					res = {"edge": eid, "seg": si, "point": q, "dist": d}
	return res


## Distancia desde un punto al borde de la calzada más cercana (negativa si está encima).
func clearance(p: Vector2, maxd: float = 30.0) -> float:
	var best := INF
	var c := _cell(p)
	var r := int(ceil(maxd / CELL))
	for dy in range(-r, r + 1):
		for dx in range(-r, r + 1):
			var k := c + Vector2i(dx, dy)
			if not _hash.has(k):
				continue
			for entry in _hash[k]:
				var eid: int = entry[0]
				var si: int = entry[1]
				var pts: PackedVector2Array = edges[eid]["pts"]
				var q := Geometry2D.get_closest_point_to_segment(p, pts[si], pts[si + 1])
				best = minf(best, p.distance_to(q) - width_of(eid) * 0.5)
	return best


func total_length() -> float:
	var l := 0.0
	for eid in edges:
		l += polyline_length(edges[eid]["pts"])
	return l


# --- Modificación ------------------------------------------------------------

## Agrega una calle. Devuelve los ids de los tramos nuevos.
func add_road(pts_in: PackedVector2Array, type: int) -> Array:
	if pts_in.size() < 2:
		return []
	var path := pts_in.duplicate()
	var start := _resolve_endpoint(path[0])
	var end := _resolve_endpoint(path[path.size() - 1])
	if start == end:
		return []
	path[0] = nodes[start]
	path[path.size() - 1] = nodes[end]

	var cuts := [[0.0, start], [float(path.size() - 1), end]]
	var hits := _find_intersections(path)
	hits.sort_custom(func(a, b): return a[0] < b[0])
	for h in hits:
		var pt: Vector2 = h[1]
		if pt.distance_to(nodes[start]) < SNAP or pt.distance_to(nodes[end]) < SNAP:
			continue
		var nid := _resolve_endpoint(pt)
		var dup := false
		for c in cuts:
			if c[1] == nid:
				dup = true
		if not dup:
			cuts.append([h[0], nid])
	cuts.sort_custom(func(a, b): return a[0] < b[0])

	var new_ids := []
	for i in cuts.size() - 1:
		var n0: int = cuts[i][1]
		var n1: int = cuts[i + 1][1]
		if n0 == n1 or _edge_between(n0, n1) != -1:
			continue
		var sub := sub_polyline(path, cuts[i][0], cuts[i + 1][0])
		if sub.size() < 2:
			continue
		sub[0] = nodes[n0]
		sub[sub.size() - 1] = nodes[n1]
		if polyline_length(sub) < 2.0:
			continue
		new_ids.append(_add_edge(n0, n1, sub, type))
	_rebuild_hash()
	changed.emit()
	return new_ids


func remove_edge(eid: int) -> void:
	if not edges.has(eid):
		return
	var a: int = edges[eid]["a"]
	var b: int = edges[eid]["b"]
	_remove_edge_internal(eid)
	for nid in [a, b]:
		if (node_edges[nid] as Array).is_empty():
			nodes.erase(nid)
			node_edges.erase(nid)
	_rebuild_hash()
	changed.emit()


func _resolve_endpoint(p: Vector2) -> int:
	var s := snap(p)
	if s["node"] != -1:
		return s["node"]
	if s["edge"] != -1:
		return _split_edge(s["edge"], s["seg"], s["pos"])
	return _add_node(p)


func _split_edge(eid: int, si: int, pt: Vector2) -> int:
	var e: Dictionary = edges[eid]
	var pts: PackedVector2Array = e["pts"]
	var a: int = e["a"]
	var b: int = e["b"]
	if pt.distance_to(nodes[a]) < 1.0:
		return a
	if pt.distance_to(nodes[b]) < 1.0:
		return b
	var first := pts.slice(0, si + 1)
	if first[first.size() - 1].distance_to(pt) > 0.05:
		first.append(pt)
	var second := PackedVector2Array([pt])
	for i in range(si + 1, pts.size()):
		if pts[i].distance_to(second[second.size() - 1]) > 0.05:
			second.append(pts[i])
	var type: int = e["type"]
	_remove_edge_internal(eid)
	var nid := _add_node(pt)
	if first.size() >= 2:
		_add_edge(a, nid, first, type)
	if second.size() >= 2:
		_add_edge(nid, b, second, type)
	_rebuild_hash()
	return nid


func _find_intersections(path: PackedVector2Array) -> Array:
	var hits := []
	for i in path.size() - 1:
		var a1 := path[i]
		var a2 := path[i + 1]
		var seg_len := a1.distance_to(a2)
		if seg_len < 0.001:
			continue
		var checked := {}
		for k in _cells_of(a1, a2):
			if not _hash.has(k):
				continue
			for entry in _hash[k]:
				var key: String = "%d_%d" % [entry[0], entry[1]]
				if checked.has(key):
					continue
				checked[key] = true
				var pts: PackedVector2Array = edges[entry[0]]["pts"]
				var x = Geometry2D.segment_intersects_segment(a1, a2, pts[entry[1]], pts[entry[1] + 1])
				if x == null:
					continue
				var pt: Vector2 = x
				var dup := false
				for h in hits:
					if (h[1] as Vector2).distance_to(pt) < 1.0:
						dup = true
				if not dup:
					hits.append([i + a1.distance_to(pt) / seg_len, pt])
	return hits


func _edge_between(a: int, b: int) -> int:
	for eid in node_edges.get(a, []):
		var e: Dictionary = edges[eid]
		if (e["a"] == a and e["b"] == b) or (e["a"] == b and e["b"] == a):
			return eid
	return -1


func _add_node(p: Vector2) -> int:
	var id := _next_node
	_next_node += 1
	nodes[id] = p
	node_edges[id] = []
	return id


func _add_edge(a: int, b: int, pts: PackedVector2Array, type: int) -> int:
	var id := _next_edge
	_next_edge += 1
	edges[id] = {"a": a, "b": b, "pts": pts, "type": type}
	(node_edges[a] as Array).append(id)
	(node_edges[b] as Array).append(id)
	return id


func _remove_edge_internal(eid: int) -> void:
	var e: Dictionary = edges[eid]
	(node_edges[e["a"]] as Array).erase(eid)
	(node_edges[e["b"]] as Array).erase(eid)
	edges.erase(eid)


func _cell(p: Vector2) -> Vector2i:
	return Vector2i(floori(p.x / CELL), floori(p.y / CELL))


func _cells_of(a: Vector2, b: Vector2) -> Array:
	var out := []
	var mn := _cell(Vector2(minf(a.x, b.x), minf(a.y, b.y)))
	var mx := _cell(Vector2(maxf(a.x, b.x), maxf(a.y, b.y)))
	for y in range(mn.y, mx.y + 1):
		for x in range(mn.x, mx.x + 1):
			out.append(Vector2i(x, y))
	return out


func _rebuild_hash() -> void:
	_hash.clear()
	for eid in edges:
		var pts: PackedVector2Array = edges[eid]["pts"]
		for si in pts.size() - 1:
			for k in _cells_of(pts[si], pts[si + 1]):
				if not _hash.has(k):
					_hash[k] = []
				(_hash[k] as Array).append([eid, si])
