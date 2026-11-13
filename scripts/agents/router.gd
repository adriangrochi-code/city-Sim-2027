extends AStar2D
## Búsqueda de rutas sobre la red de calles.
## - Peatones: el camino más corto.
## - Vehículos: el más rápido según el tiempo *medido* en cada carril (congestión real),
##   con una percepción personal distinta para cada vecino. Así no todos eligen la misma
##   calle y la gente evita los atascos que ya conoce.

const RoadNetwork = preload("res://scripts/roads/road_network.gd")
const REF_SPEED := {RoadNetwork.Type.DIRT: 2.6, RoadNetwork.Type.STONE: 3.2}

var net: RoadNetwork
var lane_time := {}      # clave de carril (tramo * 2 + sentido) -> segundos medidos (media móvil)
var _pair := {}          # Vector2i(nodo menor, nodo mayor) -> tramo
var _time_mode := true
var _seed := 0


func rebuild(n: RoadNetwork) -> void:
	net = n
	clear()
	_pair.clear()
	for nid in net.nodes:
		add_point(nid, net.nodes[nid])
	for eid in net.edges:
		var e: Dictionary = net.edges[eid]
		var a: int = e["a"]
		var b: int = e["b"]
		var k := Vector2i(mini(a, b), maxi(a, b))
		if _pair.has(k) and float(net.edges[_pair[k]]["len"]) <= float(e["len"]):
			continue
		_pair[k] = eid
		if not are_points_connected(a, b):
			connect_points(a, b)
	for key in lane_time.keys():
		if not net.edges.has(int(key) / 2):
			lane_time.erase(key)


func edge_between(a: int, b: int) -> int:
	return _pair.get(Vector2i(mini(a, b), maxi(a, b)), -1)


func free_time(eid: int) -> float:
	var e: Dictionary = net.edges[eid]
	return float(e["len"]) / float(REF_SPEED[e["type"]])


func lane_cost(eid: int, dir: int) -> float:
	var ft := free_time(eid)
	return maxf(ft, float(lane_time.get(eid * 2 + dir, ft)))


## Registra cuánto tardó un vehículo en recorrer un carril completo.
func record(eid: int, dir: int, seconds: float) -> void:
	var key := eid * 2 + dir
	var old: float = lane_time.get(key, free_time(eid))
	lane_time[key] = lerpf(old, seconds, 0.25)


func find(from_node: int, to_node: int, by_time: bool, seed_value: int) -> PackedInt64Array:
	_time_mode = by_time
	_seed = seed_value
	if from_node == to_node:
		return PackedInt64Array([from_node])
	if not has_point(from_node) or not has_point(to_node):
		return PackedInt64Array()
	return get_id_path(from_node, to_node)


func _compute_cost(from_id: int, to_id: int) -> float:
	var eid := edge_between(from_id, to_id)
	if eid == -1:
		return 1e9
	if not _time_mode:
		return net.edges[eid]["len"]
	var dir := 0 if net.edges[eid]["a"] == from_id else 1
	# Percepción personal: cada vecino "ve" los tramos un poco distinto (±15 %).
	var noise := 0.85 + 0.3 * fposmod(sin(float(eid * 7919 + _seed) * 12.9898) * 43758.5453, 1.0)
	return lane_cost(eid, dir) * noise


func _estimate_cost(from_id: int, to_id: int) -> float:
	var d := get_point_position(from_id).distance_to(get_point_position(to_id))
	return d if not _time_mode else d / 4.5
