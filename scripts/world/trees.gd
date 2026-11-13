extends Node3D
## Vegetación: monte nativo a orillas de los arroyos y ombúes solitarios en el campo.
## Los árboles desaparecen donde se construyen calles, solares o la plaza.

const TerrainGen = preload("res://scripts/world/terrain_gen.gd")

var gen: TerrainGen
var is_blocked: Callable   # func(p: Vector2) -> bool
var _trees := []           # [pos: Vector3, scale: float, color: Color, ombu: bool]
var _trunk: MultiMesh
var _crown: MultiMesh
var _dirty := true
var _cooldown := 0.0


func setup(g: TerrainGen, blocked: Callable) -> void:
	gen = g
	is_blocked = blocked
	var rng := RandomNumberGenerator.new()
	rng.seed = 99
	for path in gen.arroyos:
		var pts: PackedVector2Array = path
		for i in range(0, pts.size() - 1):
			for k in 3:
				var p := pts[i] + Vector2(rng.randf_range(-1, 1), rng.randf_range(-1, 1)).normalized() * rng.randf_range(14.0, 95.0)
				_try_add(p, rng.randf_range(0.6, 1.0), false, rng)
	for k in 4500:
		var p := Vector2(rng.randf_range(0, TerrainGen.MAP_SIZE), rng.randf_range(0, TerrainGen.MAP_SIZE))
		_try_add(p, rng.randf_range(0.9, 1.4), true, rng)
	_trunk = _make_mm(_trunk_mesh(), false)
	_crown = _make_mm(_crown_mesh(), true)


func _try_add(p: Vector2, s: float, ombu: bool, rng: RandomNumberGenerator) -> void:
	var h := gen.height(p.x, p.y)
	if h < 1.0:
		return
	var col := Color(0.24, 0.34, 0.16).lerp(Color(0.34, 0.42, 0.20), rng.randf())
	if ombu:
		col = Color(0.27, 0.38, 0.17).lerp(Color(0.31, 0.43, 0.20), rng.randf())
	_trees.append([Vector3(p.x, h, p.y), s, col, ombu])


func _make_mm(mesh: Mesh, colors: bool) -> MultiMesh:
	var mm := MultiMesh.new()
	mm.transform_format = MultiMesh.TRANSFORM_3D
	mm.use_colors = colors
	mm.mesh = mesh
	var mmi := MultiMeshInstance3D.new()
	mmi.multimesh = mm
	add_child(mmi)
	return mm


func _trunk_mesh() -> Mesh:
	var m := CylinderMesh.new()
	m.top_radius = 0.25
	m.bottom_radius = 0.45
	m.height = 3.2
	m.radial_segments = 6
	m.rings = 1
	var mat := StandardMaterial3D.new()
	mat.albedo_color = Color(0.30, 0.24, 0.18)
	m.material = mat
	return m


func _crown_mesh() -> Mesh:
	var m := SphereMesh.new()
	m.radius = 3.0
	m.height = 4.4
	m.radial_segments = 9
	m.rings = 5
	var mat := StandardMaterial3D.new()
	mat.vertex_color_use_as_albedo = true
	mat.roughness = 0.95
	m.material = mat
	return m


func mark_dirty() -> void:
	_dirty = true


func _process(delta: float) -> void:
	_cooldown -= delta
	if _dirty and _cooldown <= 0.0:
		_dirty = false
		_cooldown = 0.5
		_rebuild()


func _rebuild() -> void:
	var visible := []
	for t in _trees:
		var pos: Vector3 = t[0]
		if is_blocked.is_valid() and is_blocked.call(Vector2(pos.x, pos.z)):
			continue
		visible.append(t)
	_trunk.instance_count = visible.size()
	_crown.instance_count = visible.size()
	for i in visible.size():
		var t: Array = visible[i]
		var pos: Vector3 = t[0]
		var s: float = t[1]
		var ombu: bool = t[3]
		var cs := Vector3(1.9, 1.15, 1.9) * s if ombu else Vector3(1.0, 1.0, 1.0) * s
		var ts := Vector3(1.6, 1.0, 1.6) * s if ombu else Vector3(1.0, 1.0, 1.0) * s
		_trunk.set_instance_transform(i, Transform3D(Basis.from_scale(ts), pos + Vector3(0, 1.4 * s, 0)))
		_crown.set_instance_transform(i, Transform3D(Basis.from_scale(cs), pos + Vector3(0, (3.2 + 1.6) * s, 0)))
		_crown.set_instance_color(i, t[2])
