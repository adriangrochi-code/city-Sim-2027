extends RefCounted
## Mallas sencillas para los agentes (+Z = hacia delante). El color de instancia de la
## MultiMesh tiñe las partes blancas (ropa, pelaje).

const BuildingGen = preload("res://scripts/city/building_gen.gd")


static func _mesh(build: Callable) -> ArrayMesh:
	var st := SurfaceTool.new()
	st.begin(Mesh.PRIMITIVE_TRIANGLES)
	build.call(st)
	var mesh := st.commit()
	var mat := StandardMaterial3D.new()
	mat.vertex_color_use_as_albedo = true
	mat.roughness = 0.9
	mesh.surface_set_material(0, mat)
	return mesh


static func walker() -> ArrayMesh:
	return _mesh(func(st: SurfaceTool):
		BuildingGen.box(st, Vector3(0, 0, 0), Vector3(0.36, 0.85, 0.26), Color(0.35, 0.30, 0.25))
		BuildingGen.box(st, Vector3(0, 0.85, 0), Vector3(0.5, 0.7, 0.32), Color(1, 1, 1))
		BuildingGen.box(st, Vector3(0, 1.55, 0), Vector3(0.24, 0.26, 0.24), Color(0.80, 0.62, 0.48))
		BuildingGen.box(st, Vector3(0, 1.79, 0), Vector3(0.34, 0.08, 0.34), Color(0.15, 0.12, 0.10)))


static func horse() -> ArrayMesh:
	return _mesh(func(st: SurfaceTool):
		BuildingGen.box(st, Vector3(0, 0.75, 0), Vector3(0.55, 0.65, 1.7), Color(1, 1, 1))
		for x in [-0.18, 0.18]:
			for z in [-0.65, 0.65]:
				BuildingGen.box(st, Vector3(x, 0, z), Vector3(0.14, 0.78, 0.14), Color(0.7, 0.7, 0.7))
		BuildingGen.box(st, Vector3(0, 1.2, 0.95), Vector3(0.3, 0.5, 0.35), Color(1, 1, 1))
		BuildingGen.box(st, Vector3(0, 1.55, 1.15), Vector3(0.24, 0.24, 0.5), Color(1, 1, 1)))


static func rider() -> ArrayMesh:
	return _mesh(func(st: SurfaceTool):
		BuildingGen.box(st, Vector3(0, 1.35, -0.05), Vector3(0.5, 0.75, 0.34), Color(1, 1, 1))
		BuildingGen.box(st, Vector3(0, 2.1, -0.05), Vector3(0.24, 0.26, 0.24), Color(0.80, 0.62, 0.48))
		BuildingGen.box(st, Vector3(0, 2.34, -0.05), Vector3(0.5, 0.07, 0.5), Color(0.15, 0.12, 0.10)))


static func cart() -> ArrayMesh:
	return _mesh(func(st: SurfaceTool):
		var wood := Color(0.42, 0.30, 0.19)
		var hide := Color(0.72, 0.62, 0.45)
		# Carreta de dos ruedas grandes con techo de cuero.
		BuildingGen.box(st, Vector3(0, 0.9, -1.6), Vector3(1.7, 0.5, 3.2), wood)
		BuildingGen.box(st, Vector3(0, 1.4, -1.6), Vector3(1.6, 0.9, 3.0), hide)
		for x in [-0.95, 0.95]:
			BuildingGen.box(st, Vector3(x, 0, -1.6), Vector3(0.15, 1.9, 1.9), Color(0.30, 0.22, 0.14))
		BuildingGen.box(st, Vector3(0, 0.95, 1.4), Vector3(0.12, 0.12, 3.0), wood)
		# Dos bueyes.
		for x in [-0.5, 0.5]:
			BuildingGen.box(st, Vector3(x, 0.55, 2.4), Vector3(0.7, 0.8, 1.8), Color(0.55, 0.42, 0.30))
			BuildingGen.box(st, Vector3(x, 0, 2.4), Vector3(0.5, 0.6, 1.4), Color(0.40, 0.30, 0.22))
			BuildingGen.box(st, Vector3(x, 0.95, 3.45), Vector3(0.45, 0.45, 0.5), Color(0.55, 0.42, 0.30)))
