extends Node3D
## Malla del terreno dividida en trozos (uno por parcela). Las parcelas propias y las
## vecinas tienen más resolución; el resto del mapa y el entorno, menos.

const TerrainGen = preload("res://scripts/world/terrain_gen.gd")
const Parcels = preload("res://scripts/world/parcels.gd")
const TERRAIN_SHADER = preload("res://shaders/terrain.gdshader")
const WATER_SHADER = preload("res://shaders/water.gdshader")
const OUTER := 2   # anillo de parcelas decorativas alrededor del mapa
const SKIRT := 25.0

var gen: TerrainGen
var parcels: Parcels
var material: ShaderMaterial
var _chunks := {}
var _steps := {}


func setup(g: TerrainGen, p: Parcels) -> void:
	gen = g
	parcels = p
	material = ShaderMaterial.new()
	material.shader = TERRAIN_SHADER
	material.set_shader_parameter("map_size", TerrainGen.MAP_SIZE)
	material.set_shader_parameter("tile_size", TerrainGen.TILE_SIZE)
	_add_water()
	_add_streams()
	refresh()


func set_overlay(on: bool) -> void:
	material.set_shader_parameter("overlay", 1.0 if on else 0.0)


func set_hover_tile(t: Vector2i) -> void:
	material.set_shader_parameter("hover_tile", Vector2(t))


## Reconstruye los trozos cuya resolución cambió (al comprar parcelas).
func refresh() -> void:
	for ty in range(-OUTER, TerrainGen.TILES + OUTER):
		for tx in range(-OUTER, TerrainGen.TILES + OUTER):
			var t := Vector2i(tx, ty)
			var step := _desired_step(t)
			if is_equal_approx(float(_steps.get(t, -1.0)), step):
				continue
			if _chunks.has(t):
				(_chunks[t] as Node).queue_free()
			var mi := _build_chunk(t, step)
			add_child(mi)
			_chunks[t] = mi
			_steps[t] = step
	material.set_shader_parameter("owned_mask", parcels.mask_texture())


func _desired_step(t: Vector2i) -> float:
	if parcels.is_owned_tile(t):
		return 12.0
	if parcels.in_map(t) and parcels.is_near_owned(t):
		return 40.0
	if parcels.in_map(t):
		return 125.0
	return 375.0


func _build_chunk(t: Vector2i, step: float) -> MeshInstance3D:
	var n := int(round(TerrainGen.TILE_SIZE / step))
	var verts := n + 1
	var w := verts + 2
	var origin := Vector2(t.x, t.y) * TerrainGen.TILE_SIZE
	var hs := PackedFloat32Array()
	hs.resize(w * w)
	for j in w:
		for i in w:
			hs[j * w + i] = gen.height(origin.x + (i - 1) * step, origin.y + (j - 1) * step)

	var pv := PackedVector3Array()
	var pn := PackedVector3Array()
	pv.resize(verts * verts)
	pn.resize(verts * verts)
	for j in verts:
		for i in verts:
			var k := (j + 1) * w + i + 1
			pv[j * verts + i] = Vector3(origin.x + i * step, hs[k], origin.y + j * step)
			pn[j * verts + i] = Vector3(hs[k - 1] - hs[k + 1], 2.0 * step, hs[k - w] - hs[k + w]).normalized()
	var idx := PackedInt32Array()
	for j in n:
		for i in n:
			var a := j * verts + i
			var b := a + 1
			var c := a + verts
			var d := c + 1
			idx.append_array(PackedInt32Array([a, b, c, b, d, c]))

	# Faldones en los bordes para tapar grietas entre trozos de distinta resolución.
	var borders: Array[PackedInt32Array] = [PackedInt32Array(), PackedInt32Array(), PackedInt32Array(), PackedInt32Array()]
	for i in verts:
		borders[0].append(i)
		borders[1].append(n * verts + i)
		borders[2].append(i * verts)
		borders[3].append(i * verts + n)
	for border in borders:
		var base := pv.size()
		for vi in border:
			pv.append(pv[vi] - Vector3(0, SKIRT, 0))
			pn.append(pn[vi])
		for i in border.size() - 1:
			var a := border[i]
			var b := border[i + 1]
			var c := base + i
			var d := base + i + 1
			idx.append_array(PackedInt32Array([a, b, c, b, d, c]))

	var arrays := []
	arrays.resize(Mesh.ARRAY_MAX)
	arrays[Mesh.ARRAY_VERTEX] = pv
	arrays[Mesh.ARRAY_NORMAL] = pn
	arrays[Mesh.ARRAY_INDEX] = idx
	var mesh := ArrayMesh.new()
	mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
	var mi := MeshInstance3D.new()
	mi.mesh = mesh
	mi.material_override = material
	mi.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_ON if step <= 40.0 else GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	return mi


## Agua de los arroyos del mapa real: una cinta que sigue el cauce a la altura del terreno.
func _add_streams() -> void:
	if not gen.real or gen.arroyos.is_empty():
		return
	var st := SurfaceTool.new()
	st.begin(Mesh.PRIMITIVE_TRIANGLES)
	for path in gen.arroyos:
		var src: PackedVector2Array = path
		var pts := PackedVector2Array()
		for i in src.size() - 1:
			var n := maxi(1, int(src[i].distance_to(src[i + 1]) / 10.0))
			for k in n:
				pts.append(src[i].lerp(src[i + 1], float(k) / n))
		pts.append(src[src.size() - 1])
		if pts.size() < 2:
			continue
		# El ancho crece aguas abajo (el arroyo suma caudal).
		var total := float(pts.size())
		var prev_l := Vector3.ZERO
		var prev_r := Vector3.ZERO
		for i in pts.size():
			var tan: Vector2
			if i == 0:
				tan = pts[1] - pts[0]
			elif i == pts.size() - 1:
				tan = pts[i] - pts[i - 1]
			else:
				tan = pts[i + 1] - pts[i - 1]
			tan = tan.normalized()
			var w := lerpf(3.0, 8.0, float(i) / total)
			var nrm := Vector2(-tan.y, tan.x) * w
			var y := maxf(gen.arroyo_water_height(pts[i].x, pts[i].y), 0.05)
			var l := Vector3(pts[i].x + nrm.x, y, pts[i].y + nrm.y)
			var r := Vector3(pts[i].x - nrm.x, y, pts[i].y - nrm.y)
			if i > 0:
				for v in [prev_r, r, prev_l, r, l, prev_l]:
					st.set_normal(Vector3.UP)
					st.add_vertex(v)
			prev_l = l
			prev_r = r
	var mat := ShaderMaterial.new()
	mat.shader = WATER_SHADER
	mat.set_shader_parameter("water_color", Color(0.29, 0.27, 0.17))
	mat.set_shader_parameter("far_color", Color(0.29, 0.27, 0.17))
	mat.set_shader_parameter("specular_amount", 0.3)
	mat.set_shader_parameter("view_bias", 0.02)
	var mi := MeshInstance3D.new()
	mi.mesh = st.commit()
	mi.material_override = mat
	mi.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	add_child(mi)


func _add_water() -> void:
	var plane := PlaneMesh.new()
	plane.size = Vector2(120000, 120000)
	var mat := ShaderMaterial.new()
	mat.shader = WATER_SHADER
	var mi := MeshInstance3D.new()
	mi.mesh = plane
	mi.material_override = mat
	mi.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	mi.position = Vector3(TerrainGen.MAP_SIZE * 0.5, 0.0, TerrainGen.MAP_SIZE * 0.5)
	add_child(mi)
