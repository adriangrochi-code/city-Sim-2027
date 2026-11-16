extends Node3D
## Incendios. No son castigos al azar: dependen de cómo está construida la ciudad.
## Los ranchos de paja se prenden con facilidad y el fuego salta a los vecinos, sobre todo
## a favor del viento. Si hay agua cerca, los vecinos forman una cadena de baldes y lo apagan;
## si no, la casa se quema y queda en ruinas hasta que alguien la reconstruye.

const Lots = preload("res://scripts/city/lots.gd")
const Services = preload("res://scripts/city/services.gd")
const TerrainGen = preload("res://scripts/world/terrain_gen.gd")

signal event(text: String)

const IGNITE := [0.0, 0.00007, 0.00001, 0.000005]    # probabilidad por paso y por nivel
const CATCH := [0.0, 1.0, 0.18, 0.1]                  # facilidad para que salte el fuego
const SPREAD_DIST := 20.0
const BURN_OUT := 12                                  # pasos a máxima intensidad antes de derrumbarse

var lots: Lots
var services: Services
var gen: TerrainGen
var spawn_helpers: Callable    # func(from: Vector2, to: Vector2, n: int)
var wind := Vector2(0.7, -0.7)   # pampero: del sudoeste
var wind_name := "pampero"
var thatch_ban_center := Vector2(INF, INF)   # ordenanza: sin paja cerca de la plaza
var burning := {}   # id de solar -> {"i": intensidad, "full": pasos, "fx": Node3D, "fought": bool}
var smoking := []   # [Node3D, pasos restantes]
var destroyed_total := 0
var put_out_total := 0
var _rng := RandomNumberGenerator.new()


func setup(l: Lots, s: Services, g: TerrainGen, helpers: Callable) -> void:
	lots = l
	services = s
	gen = g
	spawn_helpers = helpers
	_rng.seed = 1806


func set_wind(dir: Vector2, name: String) -> void:
	wind = dir.normalized()
	wind_name = name


## Un paso de simulación (se llama con el crecimiento, dos veces por segundo de juego).
func step() -> void:
	# Igniciones.
	for id in lots.lots:
		var lot: Dictionary = lots.lots[id]
		var lvl: int = lot["level"]
		var p_ign: float = IGNITE[lvl]
		if lvl == 1 and thatch_ban_center.x != INF and (lot["c"] as Vector2).distance_to(thatch_ban_center) < 400.0:
			p_ign *= 0.3
		if lvl > 0 and not burning.has(id) and _rng.randf() < p_ign:
			ignite(id)
	var lost := []
	for id in burning.keys():
		if not lots.lots.has(id):
			_stop(id)
			continue
		var b: Dictionary = burning[id]
		var lot: Dictionary = lots.lots[id]
		var src := services.fire_source(lot["c"])
		if src != -1:
			if not b["fought"]:
				b["fought"] = true
				if spawn_helpers.is_valid():
					spawn_helpers.call(services.items[src]["c"], lots.front_point(lot), 6)
			b["i"] = float(b["i"]) - 0.09
		else:
			b["i"] = minf(1.0, float(b["i"]) + 0.12)
		if float(b["i"]) <= 0.0:
			put_out_total += 1
			_stop(id)
			event.emit("Los vecinos apagaron el fuego con una cadena de baldes")
			continue
		if float(b["i"]) >= 1.0:
			b["full"] = int(b["full"]) + 1
			if int(b["full"]) >= BURN_OUT:
				lost.append(id)
				continue
		_update_fx(b)
		# Propagación a los vecinos, más fácil a favor del viento.
		if float(b["i"]) > 0.5:
			for oid in lots._nearby(lot["c"]):
				if oid == id or burning.has(oid):
					continue
				var other: Dictionary = lots.lots[oid]
				if other["level"] == 0:
					continue
				var d: Vector2 = other["c"] - lot["c"]
				if d.length() > SPREAD_DIST:
					continue
				var w := 1.0 + 0.8 * d.normalized().dot(wind)
				if _rng.randf() < 0.035 * float(CATCH[other["level"]]) * w * float(b["i"]):
					ignite(oid)
	for id in lost:
		destroyed_total += 1
		var lot: Dictionary = lots.lots[id]
		var fx: Node3D = burning[id]["fx"]
		_stop(id, false)
		smoking.append([fx, 40])
		_set_smoke_only(fx)
		lots.burn_down(id)
	if lost.size() > 0:
		event.emit("El fuego destruyó %d %s" % [lost.size(), "casa" if lost.size() == 1 else "casas"])
	for i in range(smoking.size() - 1, -1, -1):
		smoking[i][1] = int(smoking[i][1]) - 1
		if int(smoking[i][1]) <= 0:
			(smoking[i][0] as Node).queue_free()
			smoking.remove_at(i)


func ignite(id: int) -> void:
	if burning.has(id) or not lots.lots.has(id):
		return
	var lot: Dictionary = lots.lots[id]
	var c: Vector2 = lot["c"]
	var fx := _make_fx(Vector3(c.x, gen.height(c.x, c.y), c.y), int(lot["level"]))
	add_child(fx)
	burning[id] = {"i": 0.2, "full": 0, "fx": fx, "fought": false}
	if burning.size() == 1 or _rng.randf() < 0.3:
		event.emit("¡Fuego! Se incendia %s" % Lots.LEVEL_NAMES[lot["level"]].to_lower())


func is_burning(id: int) -> bool:
	return burning.has(id)


func _stop(id: int, free_fx := true) -> void:
	if free_fx:
		(burning[id]["fx"] as Node).queue_free()
	burning.erase(id)


# --- Efectos visuales ------------------------------------------------------------

func _make_fx(pos: Vector3, level: int) -> Node3D:
	var root := Node3D.new()
	root.position = pos
	var h := 3.0 if level == 1 else (5.0 if level == 2 else 9.0)
	var flames := CPUParticles3D.new()
	flames.name = "Llamas"
	flames.amount = 48
	flames.lifetime = 1.1
	flames.position = Vector3(0, h * 0.6, 0)
	flames.emission_shape = CPUParticles3D.EMISSION_SHAPE_BOX
	flames.emission_box_extents = Vector3(4.0, h * 0.4, 5.0)
	flames.direction = Vector3.UP
	flames.spread = 12.0
	flames.gravity = Vector3(wind.x * 2.0, 3.0, wind.y * 2.0)
	flames.initial_velocity_min = 2.0
	flames.initial_velocity_max = 5.0
	flames.scale_amount_min = 1.0
	flames.scale_amount_max = 2.2
	flames.color_ramp = _ramp([Color(1.0, 0.85, 0.4, 0.9), Color(1.0, 0.45, 0.1, 0.8), Color(0.6, 0.1, 0.05, 0.0)])
	flames.mesh = _quad(true)
	root.add_child(flames)
	var smoke := CPUParticles3D.new()
	smoke.name = "Humo"
	smoke.amount = 36
	smoke.lifetime = 7.0
	smoke.position = Vector3(0, h, 0)
	smoke.emission_shape = CPUParticles3D.EMISSION_SHAPE_SPHERE
	smoke.emission_sphere_radius = 3.0
	smoke.direction = Vector3.UP
	smoke.spread = 20.0
	smoke.gravity = Vector3(wind.x * 3.0, 1.2, wind.y * 3.0)
	smoke.initial_velocity_min = 2.0
	smoke.initial_velocity_max = 4.0
	smoke.scale_amount_min = 1.0
	smoke.scale_amount_max = 2.5
	smoke.color_ramp = _ramp([Color(0.75, 0.5, 0.35, 0.0), Color(0.42, 0.40, 0.38, 0.32), Color(0.55, 0.55, 0.55, 0.0)])
	smoke.mesh = _quad(false)
	root.add_child(smoke)
	var light := OmniLight3D.new()
	light.name = "Luz"
	light.light_color = Color(1.0, 0.55, 0.25)
	light.omni_range = 35.0
	light.light_energy = 2.0
	light.position = Vector3(0, h, 0)
	root.add_child(light)
	return root


func _update_fx(b: Dictionary) -> void:
	var fx: Node3D = b["fx"]
	var i: float = b["i"]
	var flames: CPUParticles3D = fx.get_node("Llamas")
	flames.amount = maxi(4, int(48 * i))
	(fx.get_node("Luz") as OmniLight3D).light_energy = 0.6 + 3.5 * i


func _set_smoke_only(fx: Node3D) -> void:
	(fx.get_node("Llamas") as CPUParticles3D).emitting = false
	(fx.get_node("Luz") as OmniLight3D).visible = false


static var _dot: GradientTexture2D


## Textura circular de bordes suaves para llamas y humo.
static func _soft_dot() -> GradientTexture2D:
	if _dot == null:
		var g := Gradient.new()
		g.offsets = PackedFloat32Array([0.0, 0.55, 1.0])
		g.colors = PackedColorArray([Color(1, 1, 1, 1), Color(1, 1, 1, 0.5), Color(1, 1, 1, 0)])
		_dot = GradientTexture2D.new()
		_dot.gradient = g
		_dot.fill = GradientTexture2D.FILL_RADIAL
		_dot.fill_from = Vector2(0.5, 0.5)
		_dot.fill_to = Vector2(1.0, 0.5)
		_dot.width = 64
		_dot.height = 64
	return _dot


func _ramp(cols: Array) -> Gradient:
	var g := Gradient.new()
	g.offsets = PackedFloat32Array([0.0, 0.5, 1.0])
	g.colors = PackedColorArray(cols)
	return g


func _quad(additive: bool) -> QuadMesh:
	var q := QuadMesh.new()
	q.size = Vector2(1.6, 2.0) if additive else Vector2(5, 5)
	var m := StandardMaterial3D.new()
	m.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	m.billboard_mode = BaseMaterial3D.BILLBOARD_PARTICLES
	m.vertex_color_use_as_albedo = true
	m.albedo_texture = _soft_dot()
	m.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	if additive:
		m.blend_mode = BaseMaterial3D.BLEND_MODE_ADD
	q.material = m
	return q
