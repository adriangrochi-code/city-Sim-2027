extends Node3D
## Iglesia matriz: el gran proyecto de la época colonial. Empieza como una capilla de adobe
## y se construye por fases a lo largo de los años; cada fase se ve en obra, con andamios,
## y al terminarla atrae a más pobladores.

const BuildingGen = preload("res://scripts/city/building_gen.gd")

signal event(text: String)

const PHASES := [
	{"name": "Capilla de adobe", "cost": 0, "months": 0},
	{"name": "Cimientos de piedra", "cost": 6000, "months": 6},
	{"name": "Muros", "cost": 10000, "months": 10},
	{"name": "Naves y techo de teja", "cost": 12000, "months": 10},
	{"name": "Cúpula", "cost": 9000, "months": 8},
	{"name": "Torres y campanario", "cost": 8000, "months": 8},
]
const MIN_POP := 1000

var phase := 0          # última fase terminada
var building := false   # hay una fase en obra
var months_left := 0
var _mesh: MeshInstance3D
var _scaffold: MeshInstance3D


func setup(xf: Transform3D) -> void:
	transform = xf
	_mesh = MeshInstance3D.new()
	_mesh.material_override = BuildingGen.material()
	add_child(_mesh)
	_scaffold = MeshInstance3D.new()
	_scaffold.material_override = BuildingGen.material()
	add_child(_scaffold)
	_refresh()


func finished() -> bool:
	return phase >= PHASES.size() - 1


func next_problem(pop: int, money: int) -> String:
	if finished():
		return "La iglesia matriz ya está terminada"
	if building:
		return "Ya hay una fase en obra (faltan %d meses)" % months_left
	if pop < MIN_POP:
		return "Hace falta una ciudad de %d habitantes para empezar la obra" % MIN_POP
	var cost: int = PHASES[phase + 1]["cost"]
	if cost > money:
		return "La fase «%s» cuesta %d pesos" % [PHASES[phase + 1]["name"], cost]
	return ""


## Empieza la fase siguiente. Devuelve su costo.
func start_next() -> int:
	building = true
	months_left = PHASES[phase + 1]["months"]
	_refresh()
	event.emit("Empieza la obra de la iglesia matriz: %s" % PHASES[phase + 1]["name"].to_lower())
	return PHASES[phase + 1]["cost"]


func month_tick() -> void:
	if not building:
		return
	months_left -= 1
	if months_left <= 0:
		building = false
		phase += 1
		_refresh()
		if finished():
			event.emit("¡Repican las campanas! La iglesia matriz está terminada")
		else:
			event.emit("Terminada la fase «%s» de la iglesia matriz" % PHASES[phase]["name"])


## Más pobladores por cada fase terminada: la obra da prestigio a la ciudad.
func attraction() -> float:
	return 1.0 + 0.08 * phase


func describe() -> String:
	var txt := "Iglesia matriz\nEstado: %s\n" % PHASES[phase]["name"]
	if building:
		txt += "En obra: %s (faltan %d meses)\n" % [PHASES[phase + 1]["name"], months_left]
	elif not finished():
		txt += "Siguiente fase: %s · %d pesos · %d meses\nPulsa G para financiarla.\n" % [PHASES[phase + 1]["name"], PHASES[phase + 1]["cost"], PHASES[phase + 1]["months"]]
	txt += "Atrae un %d %% más de pobladores" % int((attraction() - 1.0) * 100.0)
	return txt


func set_state(p: int, b: bool, m: int) -> void:
	phase = p
	building = b
	months_left = m
	_refresh()


func _refresh() -> void:
	_mesh.mesh = _phase_mesh(phase)
	_scaffold.visible = building
	if building:
		_scaffold.mesh = _scaffold_mesh(phase + 1)


# --- Mallas por fase (la fachada mira hacia +Z) --------------------------------------

static func _phase_mesh(p: int) -> ArrayMesh:
	var st := SurfaceTool.new()
	st.begin(Mesh.PRIMITIVE_TRIANGLES)
	var cal := Color(0.93, 0.91, 0.85)
	var stone := Color(0.62, 0.59, 0.54)
	var rot := Transform3D(Basis(Vector3.UP, PI * 0.5), Vector3.ZERO)
	if p == 0:
		var adobe: Color = BuildingGen.ADOBE[0]
		BuildingGen.box(st, Vector3(0, -1.0, 2.0), Vector3(7.0, 5.0, 14.0), adobe)
		BuildingGen.gable(st, Vector3(-2.0, 4.0, 0), 7.5, 4.0, 2.4, BuildingGen.THATCH[0], adobe, 7.0, 3.5, rot)
		BuildingGen.box(st, Vector3(0, 4.0, 8.95), Vector3(2.6, 3.0, 0.5), adobe)
		BuildingGen.box(st, Vector3(0, 5.2, 9.0), Vector3(0.9, 1.1, 0.6), BuildingGen.WINDOW)
		BuildingGen.box(st, Vector3(0, 0, 9.05), Vector3(1.6, 2.6, 0.12), Color(0.30, 0.22, 0.15))
		return st.commit()
	# Cimientos: el contorno completo en piedra.
	BuildingGen.box(st, Vector3(0, -1.5, -2.0), Vector3(14.0, 2.3, 31.0), stone)
	for sx in [-1.0, 1.0]:
		BuildingGen.box(st, Vector3(sx * 5.2, -1.5, 11.0), Vector3(5.4, 2.3, 5.4), stone)
	if p == 1:
		return st.commit()
	# Muros (sin techo hasta la fase 3).
	var wall_h := 11.0
	BuildingGen.box(st, Vector3(-6.2, 0.8, -2.0), Vector3(0.9, wall_h - 0.8, 30.0), cal)
	BuildingGen.box(st, Vector3(6.2, 0.8, -2.0), Vector3(0.9, wall_h - 0.8, 30.0), cal)
	BuildingGen.box(st, Vector3(0, 0.8, -16.6), Vector3(13.0, wall_h - 0.8, 0.9), cal)
	BuildingGen.box(st, Vector3(0, 0.8, 12.6), Vector3(13.0, wall_h - 0.8, 0.9), cal)
	BuildingGen.box(st, Vector3(0, 0, 13.1), Vector3(2.6, 4.6, 0.15), Color(0.30, 0.22, 0.15))
	BuildingGen.box(st, Vector3(0, 6.5, 13.1), Vector3(1.6, 2.2, 0.15), BuildingGen.WINDOW)
	if p >= 3:
		BuildingGen.gable(st, Vector3(2.0, wall_h, 0), 15.5, 7.0, 4.5, BuildingGen.TILE_ROOF, cal, 15.0, 6.5, rot)
	if p >= 4:
		# Cúpula escalonada sobre el crucero, con linterna.
		var y := wall_h + 3.0
		BuildingGen.box(st, Vector3(0, y - 3.0, -8.0), Vector3(8.0, 5.0, 8.0), cal)
		var sizes := [7.2, 6.4, 5.4, 4.2, 2.8]
		for i in sizes.size():
			var sz: float = sizes[i]
			BuildingGen.box(st, Vector3(0, y + 2.0 + i * 0.9, -8.0), Vector3(sz, 0.9, sz), Color(0.80, 0.78, 0.74) if i < 4 else cal)
		BuildingGen.box(st, Vector3(0, y + 6.5, -8.0), Vector3(1.4, 1.8, 1.4), cal)
		BuildingGen.box(st, Vector3(0, y + 8.3, -8.0), Vector3(0.2, 1.6, 0.2), Color(0.15, 0.15, 0.15))
	if p >= 5:
		for sx in [-1.0, 1.0]:
			var tx: float = sx * 5.2
			BuildingGen.box(st, Vector3(tx, 0.8, 11.0), Vector3(4.6, 19.2, 4.6), cal)
			BuildingGen.box(st, Vector3(tx, 20.0, 11.0), Vector3(5.0, 0.5, 5.0), cal.darkened(0.08))
			BuildingGen.box(st, Vector3(tx, 20.5, 11.0), Vector3(3.6, 3.8, 3.6), cal)
			for k in 4:
				var xf := Transform3D(Basis(Vector3.UP, PI * 0.5 * k), Vector3(tx, 20.5, 11.0))
				BuildingGen.box(st, Vector3(0, 0.8, 1.81), Vector3(1.4, 2.4, 0.06), BuildingGen.WINDOW, xf)
			BuildingGen.gable(st, Vector3(tx, 24.3, 11.0), 2.0, 2.0, 2.2, BuildingGen.TILE_ROOF, cal, 1.8, 1.8)
	return st.commit()


## Andamios de madera alrededor de la parte que se está construyendo.
static func _scaffold_mesh(p: int) -> ArrayMesh:
	var st := SurfaceTool.new()
	st.begin(Mesh.PRIMITIVE_TRIANGLES)
	var wood := Color(0.55, 0.42, 0.28)
	var h: float = [0.0, 3.0, 12.0, 15.0, 22.0, 26.0][p]
	var xs := [-7.6, 7.6]
	var z := -17.0
	while z <= 14.0:
		for x in xs:
			BuildingGen.box(st, Vector3(x, 0, z), Vector3(0.25, h, 0.25), wood)
		z += 4.0
	var y := 2.5
	while y < h:
		for x in xs:
			BuildingGen.box(st, Vector3(x, y, -1.5), Vector3(1.2, 0.15, 31.0), wood.darkened(0.15))
		y += 3.0
	if p >= 4:
		for x in [-2.0, 2.0]:
			BuildingGen.box(st, Vector3(x, 11.0, -8.0), Vector3(0.25, 12.0, 0.25), wood)
	# Una grúa de madera junto a la obra.
	BuildingGen.box(st, Vector3(-11.0, 0, 4.0), Vector3(0.6, h + 4.0, 0.6), wood)
	BuildingGen.box(st, Vector3(-8.0, h + 3.5, 4.0), Vector3(7.0, 0.4, 0.4), wood)
	return st.commit()
