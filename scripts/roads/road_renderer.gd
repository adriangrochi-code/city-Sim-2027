extends Node3D
## Dibuja la red de calles: una cinta por tramo que sigue el terreno y un disco en cada nodo.

const RoadNetwork = preload("res://scripts/roads/road_network.gd")
const TerrainGen = preload("res://scripts/world/terrain_gen.gd")
const ROAD_SHADER = preload("res://shaders/road.gdshader")
const OFFSET := 0.45

var net: RoadNetwork
var gen: TerrainGen
var _mats := {}
var _edge_meshes := {}
var _node_meshes := {}


func setup(n: RoadNetwork, g: TerrainGen) -> void:
	net = n
	gen = g
	_mats[RoadNetwork.Type.DIRT] = _make_mat(0, Color(0.50, 0.40, 0.28))
	_mats[RoadNetwork.Type.STONE] = _make_mat(1, Color(0.56, 0.54, 0.50))
	net.changed.connect(rebuild)
	rebuild()


func _make_mat(kind: int, col: Color) -> ShaderMaterial:
	var m := ShaderMaterial.new()
	m.shader = ROAD_SHADER
	m.set_shader_parameter("kind", kind)
	m.set_shader_parameter("base_color", col)
	return m


func rebuild() -> void:
	for eid in _edge_meshes.keys():
		if not net.edges.has(eid):
			(_edge_meshes[eid] as Node).queue_free()
			_edge_meshes.erase(eid)
	for eid in net.edges:
		if _edge_meshes.has(eid):
			continue
		var e: Dictionary = net.edges[eid]
		var mi := MeshInstance3D.new()
		mi.mesh = _ribbon(e["pts"], RoadNetwork.WIDTH[e["type"]])
		mi.material_override = _mats[e["type"]]
		mi.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
		add_child(mi)
		_edge_meshes[eid] = mi
	for nid in _node_meshes.keys():
		(_node_meshes[nid] as Node).queue_free()
	_node_meshes.clear()
	for nid in net.nodes:
		var best_w := 0.0
		var best_t := RoadNetwork.Type.DIRT
		for eid in net.node_edges[nid]:
			var t: int = net.edges[eid]["type"]
			var w: float = RoadNetwork.WIDTH[t]
			if w > best_w:
				best_w = w
				best_t = t
		var mi := MeshInstance3D.new()
		mi.mesh = _disc(net.nodes[nid], best_w * 0.5)
		mi.material_override = _mats[best_t]
		mi.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
		add_child(mi)
		_node_meshes[nid] = mi


func _h(p: Vector2) -> float:
	return gen.height(p.x, p.y) + OFFSET


func _ribbon(pts: PackedVector2Array, w: float) -> ArrayMesh:
	var st := SurfaceTool.new()
	st.begin(Mesh.PRIMITIVE_TRIANGLES)
	var n := pts.size()
	var dist := 0.0
	var prev_l := Vector3.ZERO
	var prev_r := Vector3.ZERO
	var prev_v := 0.0
	for i in n:
		var tan: Vector2
		if i == 0:
			tan = pts[1] - pts[0]
		elif i == n - 1:
			tan = pts[n - 1] - pts[n - 2]
		else:
			tan = pts[i + 1] - pts[i - 1]
		tan = tan.normalized()
		var nrm := Vector2(-tan.y, tan.x) * (w * 0.5)
		var l2 := pts[i] + nrm
		var r2 := pts[i] - nrm
		var l := Vector3(l2.x, _h(l2), l2.y)
		var r := Vector3(r2.x, _h(r2), r2.y)
		if i > 0:
			dist += pts[i].distance_to(pts[i - 1])
			var v := dist / w
			_v(st, prev_r, Vector2(1, prev_v))
			_v(st, r, Vector2(1, v))
			_v(st, prev_l, Vector2(0, prev_v))
			_v(st, r, Vector2(1, v))
			_v(st, l, Vector2(0, v))
			_v(st, prev_l, Vector2(0, prev_v))
			prev_v = v
		prev_l = l
		prev_r = r
	return st.commit()


func _v(st: SurfaceTool, p: Vector3, uv: Vector2) -> void:
	st.set_normal(Vector3.UP)
	st.set_uv(uv)
	st.add_vertex(p)


func _disc(c: Vector2, r: float) -> ArrayMesh:
	var st := SurfaceTool.new()
	st.begin(Mesh.PRIMITIVE_TRIANGLES)
	var center := Vector3(c.x, _h(c) + 0.02, c.y)
	var seg := 20
	for i in seg:
		var a0 := TAU * i / seg
		var a1 := TAU * (i + 1) / seg
		var p0 := c + Vector2(cos(a0), sin(a0)) * r
		var p1 := c + Vector2(cos(a1), sin(a1)) * r
		# Visto desde arriba (x a la derecha, z hacia abajo), el ángulo crece en sentido horario.
		_v(st, center, Vector2.ZERO)
		_v(st, Vector3(p0.x, _h(p0) + 0.02, p0.y), Vector2.ZERO)
		_v(st, Vector3(p1.x, _h(p1) + 0.02, p1.y), Vector2.ZERO)
	return st.commit()
