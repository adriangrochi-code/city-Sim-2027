extends RefCounted
## Generador procedural de edificios coloniales (prototipo).
## Coordenadas locales: origen en el centro del solar a nivel del suelo; +Z mira a la calle.
## El alfa del color de vértice marca las ventanas (alfa 0), que se iluminan de noche.

const ADOBE := [Color(0.61, 0.48, 0.33), Color(0.66, 0.53, 0.37), Color(0.57, 0.45, 0.31)]
const THATCH := [Color(0.78, 0.66, 0.40), Color(0.71, 0.59, 0.35), Color(0.82, 0.71, 0.46)]
const CAL := [Color(0.94, 0.92, 0.86), Color(0.93, 0.89, 0.81), Color(0.91, 0.85, 0.72), Color(0.89, 0.82, 0.73), Color(0.86, 0.73, 0.67), Color(0.80, 0.83, 0.82)]
const SHUTTER := [Color(0.24, 0.35, 0.27), Color(0.33, 0.25, 0.18), Color(0.36, 0.41, 0.40), Color(0.20, 0.30, 0.36)]
const ROOF_FLAT := Color(0.60, 0.42, 0.32)
const TILE_ROOF := Color(0.62, 0.33, 0.24)
const WINDOW := Color(0.09, 0.07, 0.05, 0.0)
const IRON := Color(0.11, 0.11, 0.11)
const STONE := Color(0.62, 0.60, 0.56)
const GARDEN := Color(0.33, 0.42, 0.22)

static var _cache := {}
static var _material: ShaderMaterial


## Material compartido por todos los edificios (lo actualiza el ciclo día/noche).
static func material() -> ShaderMaterial:
	if _material == null:
		_material = ShaderMaterial.new()
		_material.shader = load("res://shaders/building.gdshader")
	return _material


static func get_mesh(level: int, variant: int, w: float, d: float) -> ArrayMesh:
	var key := "%d_%d" % [level, variant]
	if _cache.has(key):
		return _cache[key]
	var rng := RandomNumberGenerator.new()
	rng.seed = level * 7919 + variant * 104729 + 17
	var st := SurfaceTool.new()
	st.begin(Mesh.PRIMITIVE_TRIANGLES)
	match level:
		1:
			_rancho(st, rng, w, d)
		2:
			_azotea(st, rng, w, d)
		_:
			_altos(st, rng, w, d)
	var mesh := st.commit()
	_cache[key] = mesh
	return mesh


# --- Primitivas -------------------------------------------------------------

## Cuadrilátero con vértices en sentido horario vistos desde fuera.
static func quad(st: SurfaceTool, a: Vector3, b: Vector3, c: Vector3, d: Vector3, n: Vector3, col: Color, xf: Transform3D) -> void:
	var nn := (xf.basis * n).normalized()
	for p in [a, b, c, a, c, d]:
		st.set_color(col)
		st.set_normal(nn)
		st.add_vertex(xf * (p as Vector3))


static func tri(st: SurfaceTool, a: Vector3, b: Vector3, c: Vector3, n: Vector3, col: Color, xf: Transform3D) -> void:
	var nn := (xf.basis * n).normalized()
	for p in [a, b, c]:
		st.set_color(col)
		st.set_normal(nn)
		st.add_vertex(xf * (p as Vector3))


## Caja apoyada: `c` es el centro de la base. Sin cara inferior.
static func box(st: SurfaceTool, c: Vector3, s: Vector3, col: Color, xf := Transform3D.IDENTITY, top := Color(-1, 0, 0)) -> void:
	var x := s.x * 0.5
	var z := s.z * 0.5
	var y0 := c.y
	var y1 := c.y + s.y
	var tc := col if top.r < 0.0 else top
	var o := Vector3(c.x, 0, c.z)
	quad(st, o + Vector3(-x, y1, -z), o + Vector3(x, y1, -z), o + Vector3(x, y1, z), o + Vector3(-x, y1, z), Vector3.UP, tc, xf)
	quad(st, o + Vector3(-x, y1, z), o + Vector3(x, y1, z), o + Vector3(x, y0, z), o + Vector3(-x, y0, z), Vector3.BACK, col, xf)
	quad(st, o + Vector3(x, y1, -z), o + Vector3(-x, y1, -z), o + Vector3(-x, y0, -z), o + Vector3(x, y0, -z), Vector3.FORWARD, col, xf)
	quad(st, o + Vector3(x, y1, z), o + Vector3(x, y1, -z), o + Vector3(x, y0, -z), o + Vector3(x, y0, z), Vector3.RIGHT, col, xf)
	quad(st, o + Vector3(-x, y1, -z), o + Vector3(-x, y1, z), o + Vector3(-x, y0, z), o + Vector3(-x, y0, -z), Vector3.LEFT, col, xf)


## Techo a dos aguas con la cumbrera a lo largo de X. `c` = centro a la altura del alero.
static func gable(st: SurfaceTool, c: Vector3, hx: float, hz: float, rh: float, roof: Color, wall: Color, wall_hx: float, wall_hz: float, xf := Transform3D.IDENTITY) -> void:
	var y0 := c.y
	var y1 := c.y + rh
	var o := Vector3(c.x, 0, c.z)
	var nf := Vector3(0, hz, rh).normalized()
	var nb := Vector3(0, hz, -rh).normalized()
	quad(st, o + Vector3(-hx, y1, 0), o + Vector3(hx, y1, 0), o + Vector3(hx, y0, hz), o + Vector3(-hx, y0, hz), nf, roof, xf)
	quad(st, o + Vector3(hx, y1, 0), o + Vector3(-hx, y1, 0), o + Vector3(-hx, y0, -hz), o + Vector3(hx, y0, -hz), nb, roof, xf)
	var ry := y0 + rh * (1.0 - wall_hz / hz)
	tri(st, o + Vector3(wall_hx, y1, 0), o + Vector3(wall_hx, y0, -wall_hz), o + Vector3(wall_hx, y0, wall_hz), Vector3.RIGHT, wall, xf)
	tri(st, o + Vector3(-wall_hx, y1, 0), o + Vector3(-wall_hx, y0, wall_hz), o + Vector3(-wall_hx, y0, -wall_hz), Vector3.LEFT, wall, xf)
	# Bajo el alero (para que no se vea hueco desde abajo).
	quad(st, o + Vector3(-hx, y0, hz), o + Vector3(hx, y0, hz), o + Vector3(hx, ry, 0), o + Vector3(-hx, ry, 0), Vector3.DOWN, roof.darkened(0.4), xf)


## Ventana con reja en la fachada frontal (plano z = fz).
static func window(st: SurfaceTool, x: float, y: float, fz: float, ww: float, wh: float, bars: bool) -> void:
	box(st, Vector3(x, y, fz + 0.03), Vector3(ww, wh, 0.12), WINDOW)
	if bars:
		var n := 4
		for i in n:
			var bx := x - ww * 0.5 + ww * (i + 0.5) / n
			box(st, Vector3(bx, y - 0.05, fz + 0.14), Vector3(0.05, wh + 0.1, 0.05), IRON)
		box(st, Vector3(x, y + wh * 0.5, fz + 0.14), Vector3(ww + 0.1, 0.05, 0.05), IRON)


# --- Niveles ---------------------------------------------------------------

## Nivel 1: rancho de barro y paja, algo retirado de la calle, con huerta.
static func _rancho(st: SurfaceTool, rng: RandomNumberGenerator, w: float, d: float) -> void:
	var bw := rng.randf_range(5.5, minf(7.5, w - 2.0))
	var bd := rng.randf_range(4.5, 5.6)
	var wh := rng.randf_range(2.3, 2.7)
	var front := d * 0.5 - rng.randf_range(2.0, 4.5)
	var cz := front - bd * 0.5
	var cx := rng.randf_range(-0.8, 0.8)
	var wall: Color = ADOBE[rng.randi() % ADOBE.size()]
	box(st, Vector3(cx, -0.6, cz), Vector3(bw, wh + 0.6, bd), wall)
	gable(st, Vector3(cx, wh, cz), bw * 0.5 + 0.35, bd * 0.5 + 0.45, rng.randf_range(1.6, 2.1), THATCH[rng.randi() % THATCH.size()], wall, bw * 0.5, bd * 0.5)
	box(st, Vector3(cx - bw * 0.2, 0.0, front + 0.03), Vector3(0.9, 1.9, 0.1), Color(0.26, 0.19, 0.13))
	box(st, Vector3(cx + bw * 0.25, 1.0, front + 0.03), Vector3(0.7, 0.7, 0.1), WINDOW)
	box(st, Vector3(rng.randf_range(-2.0, 2.0), -0.3, -d * 0.5 + 4.0), Vector3(5.0, 0.42, 5.0), GARDEN)


## Nivel 2: casa de azotea encalada, sobre la línea de la calle, con patio y ala lateral.
static func _azotea(st: SurfaceTool, rng: RandomNumberGenerator, w: float, d: float) -> void:
	var h := rng.randf_range(4.2, 4.7)
	var fd := 7.0
	var front := d * 0.5 - 0.1
	var wall: Color = CAL[rng.randi() % CAL.size()]
	var shutter: Color = SHUTTER[rng.randi() % SHUTTER.size()]
	var bw := w - 0.1
	box(st, Vector3(0, -1.0, front - fd * 0.5), Vector3(bw, h + 1.0, fd), wall, Transform3D.IDENTITY, ROOF_FLAT)
	_parapet(st, bw, fd, front, h, wall)
	box(st, Vector3(0, h - 0.45, front + 0.06), Vector3(bw, 0.18, 0.12), wall.darkened(0.08))
	var side := 1.0 if rng.randf() < 0.5 else -1.0
	var door_x := -side * (bw * 0.5 - 1.6)
	box(st, Vector3(door_x, 0.0, front + 0.04), Vector3(1.3, 2.7, 0.12), shutter)
	window(st, door_x + side * 3.0, 0.9, front, 1.1, 1.9, true)
	window(st, door_x + side * 5.6, 0.9, front, 1.1, 1.9, true)
	# Ala lateral alrededor del patio.
	var wd := d - fd - 3.0
	var wx := side * (bw * 0.5 - 1.9)
	box(st, Vector3(wx, -1.0, front - fd - wd * 0.5), Vector3(3.8, h + 0.6, wd), wall, Transform3D.IDENTITY, ROOF_FLAT)
	for k in 2:
		var zz := front - fd - 2.0 - k * 4.0
		var xf := Transform3D(Basis(Vector3.UP, -side * PI * 0.5), Vector3(wx - side * 1.9, 0, zz))
		box(st, Vector3(0, 0.9, 0.03), Vector3(1.0, 1.8, 0.1), WINDOW, xf)


## Nivel 3: casa de altos con balcones de hierro y mirador en la azotea.
static func _altos(st: SurfaceTool, rng: RandomNumberGenerator, w: float, d: float) -> void:
	var h := rng.randf_range(8.0, 8.8)
	var fd := 9.0
	var front := d * 0.5 - 0.1
	var wall: Color = CAL[rng.randi() % CAL.size()]
	var shutter: Color = SHUTTER[rng.randi() % SHUTTER.size()]
	var bw := w - 0.1
	box(st, Vector3(0, -1.0, front - fd * 0.5), Vector3(bw, h + 1.0, fd), wall, Transform3D.IDENTITY, ROOF_FLAT)
	_parapet(st, bw, fd, front, h, wall)
	box(st, Vector3(0, 4.1, front + 0.07), Vector3(bw, 0.22, 0.14), wall.darkened(0.08))
	box(st, Vector3(0, h - 0.5, front + 0.07), Vector3(bw, 0.22, 0.14), wall.darkened(0.08))
	box(st, Vector3(0, -0.2, front + 0.05), Vector3(bw, 0.6, 0.1), STONE)
	var side := 1.0 if rng.randf() < 0.5 else -1.0
	var door_x := -side * (bw * 0.5 - 1.6)
	box(st, Vector3(door_x, 0.0, front + 0.04), Vector3(1.4, 2.9, 0.12), shutter)
	window(st, door_x + side * 3.1, 0.9, front, 1.1, 2.0, true)
	window(st, door_x + side * 5.8, 0.9, front, 1.1, 2.0, true)
	# Planta alta: puertas-ventana con balcón.
	for i in 3:
		var x := -bw * 0.5 + bw * (i + 0.5) / 3.0
		box(st, Vector3(x, 4.6, front + 0.03), Vector3(1.1, 2.4, 0.12), WINDOW)
		box(st, Vector3(x, 4.35, front + 0.4), Vector3(1.7, 0.12, 0.75), STONE.darkened(0.1))
		box(st, Vector3(x, 4.47, front + 0.76), Vector3(1.7, 0.95, 0.04), IRON)
	# Mirador.
	var mx := side * (bw * 0.5 - 1.9)
	var mz := front - 2.4
	box(st, Vector3(mx, h, mz), Vector3(3.2, 3.2, 3.2), wall, Transform3D.IDENTITY, ROOF_FLAT)
	box(st, Vector3(mx, h + 3.2, mz), Vector3(3.5, 0.35, 3.5), wall.darkened(0.06))
	for k in 4:
		var xf := Transform3D(Basis(Vector3.UP, PI * 0.5 * k), Vector3(mx, h, mz))
		box(st, Vector3(0, 1.0, 1.62), Vector3(1.1, 1.6, 0.06), WINDOW, xf)
	# Ala lateral de dos plantas.
	var wd := d - fd - 3.0
	var wx := side * (bw * 0.5 - 1.9)
	box(st, Vector3(wx, -1.0, front - fd - wd * 0.5), Vector3(3.8, h - 0.4, wd), wall, Transform3D.IDENTITY, ROOF_FLAT)


static func _parapet(st: SurfaceTool, bw: float, fd: float, front: float, h: float, wall: Color) -> void:
	var t := 0.25
	box(st, Vector3(0, h, front - t * 0.5), Vector3(bw, 0.55, t), wall)
	box(st, Vector3(0, h, front - fd + t * 0.5), Vector3(bw, 0.55, t), wall)
	box(st, Vector3(bw * 0.5 - t * 0.5, h, front - fd * 0.5), Vector3(t, 0.55, fd), wall)
	box(st, Vector3(-bw * 0.5 + t * 0.5, h, front - fd * 0.5), Vector3(t, 0.55, fd), wall)


## Iglesia matriz (versión inicial del gran proyecto): nave, techo de teja y campanario.
## La fachada mira hacia +Z.
static func church_mesh() -> ArrayMesh:
	var st := SurfaceTool.new()
	st.begin(Mesh.PRIMITIVE_TRIANGLES)
	var wall := Color(0.93, 0.91, 0.85)
	var rot := Transform3D(Basis(Vector3.UP, PI * 0.5), Vector3.ZERO)
	box(st, Vector3(0, -1.5, -2.0), Vector3(13.0, 12.5, 30.0), wall)
	gable(st, Vector3(0, 11.0, -2.0), 15.5, 7.0, 4.5, TILE_ROOF, wall, 15.0, 6.5, rot)
	box(st, Vector3(0, 0, 13.05), Vector3(2.6, 4.6, 0.15), Color(0.30, 0.22, 0.15))
	box(st, Vector3(0, 6.5, 13.05), Vector3(1.6, 2.2, 0.15), WINDOW)
	for sx in [-1.0, 1.0]:
		var tx: float = sx * 5.2
		box(st, Vector3(tx, -1.5, 11.0), Vector3(4.6, 21.5, 4.6), wall)
		box(st, Vector3(tx, 20.0, 11.0), Vector3(5.0, 0.5, 5.0), wall.darkened(0.08))
		box(st, Vector3(tx, 20.5, 11.0), Vector3(3.6, 3.8, 3.6), wall)
		for k in 4:
			var xf := Transform3D(Basis(Vector3.UP, PI * 0.5 * k), Vector3(tx, 20.5, 11.0))
			box(st, Vector3(0, 0.8, 1.81), Vector3(1.4, 2.4, 0.06), WINDOW, xf)
		gable(st, Vector3(tx, 24.3, 11.0), 2.0, 2.0, 2.2, TILE_ROOF, wall, 1.8, 1.8)
	return st.commit()
