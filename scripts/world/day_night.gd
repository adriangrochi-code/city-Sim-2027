extends Node
## Ciclo de día y noche con la luz rioplatense. En el hemisferio sur el sol pasa por el norte.
## De noche la ciudad colonial queda casi a oscuras: solo brillan velas y faroles.

var time := 0.32           # 0 = medianoche, 0.25 = 6:00, 0.5 = mediodía
var day_length := 480.0    # segundos reales por día a velocidad 1
var day_index := 0
const WEEKDAYS := ["lunes", "martes", "miércoles", "jueves", "viernes", "sábado", "domingo"]
var speed := 1.0
var sun: DirectionalLight3D
var env: Environment
var sky: ProceduralSkyMaterial
var night_materials: Array[ShaderMaterial] = []   # materiales con `night_factor`


func setup(world: Node3D) -> void:
	sky = ProceduralSkyMaterial.new()
	sky.sun_angle_max = 18.0
	var s := Sky.new()
	s.sky_material = sky
	env = Environment.new()
	env.background_mode = Environment.BG_SKY
	env.sky = s
	env.ambient_light_source = Environment.AMBIENT_SOURCE_SKY
	env.tonemap_mode = Environment.TONE_MAPPER_FILMIC
	env.tonemap_white = 6.0
	env.tonemap_exposure = 0.95
	env.fog_enabled = true
	env.fog_density = 0.00002
	env.fog_aerial_perspective = 0.3
	env.glow_enabled = true
	env.glow_intensity = 0.6
	env.glow_bloom = 0.05
	env.ssao_enabled = true
	var we := WorldEnvironment.new()
	we.environment = env
	world.add_child(we)
	sun = DirectionalLight3D.new()
	sun.shadow_enabled = true
	sun.directional_shadow_max_distance = 900.0
	world.add_child(sun)
	apply()


func _process(delta: float) -> void:
	advance(delta * speed / day_length)
	apply()


func advance(fraction: float) -> void:
	time += fraction
	while time >= 1.0:
		time -= 1.0
		day_index += 1


func weekday() -> int:
	return day_index % 7


func hour() -> float:
	return time * 24.0


func is_night() -> bool:
	return elevation() < 0.0


func elevation() -> float:
	return sin((time - 0.25) * TAU)


func clock_text() -> String:
	var minutes := int(time * 24.0 * 60.0)
	return "%02d:%02d" % [minutes / 60, minutes % 60]


func apply() -> void:
	var ang := (time - 0.25) * TAU
	var elev := sin(ang)
	var day := smoothstep(-0.1, 0.3, elev)
	var sunset := clampf(1.0 - absf(elev) / 0.28, 0.0, 1.0)
	var sun_dir := Vector3(cos(ang), maxf(elev, 0.02) * 0.9, -0.42).normalized()
	if elev >= 0.0:
		sun.basis = Basis.looking_at(-sun_dir, Vector3.UP)
		sun.light_energy = 1.25 * smoothstep(0.0, 0.18, elev)
		sun.light_color = Color(1.0, 0.62, 0.38).lerp(Color(1.0, 0.96, 0.9), smoothstep(0.05, 0.5, elev))
	else:
		# Luz de luna.
		var moon_dir := Vector3(-cos(ang), maxf(-elev, 0.05) * 0.8, -0.3).normalized()
		sun.basis = Basis.looking_at(-moon_dir, Vector3.UP)
		sun.light_energy = 0.14 * smoothstep(0.0, 0.2, -elev)
		sun.light_color = Color(0.55, 0.65, 0.95)
	var top := Color(0.025, 0.035, 0.08).lerp(Color(0.30, 0.48, 0.76), day)
	var horizon := Color(0.05, 0.06, 0.11).lerp(Color(0.74, 0.79, 0.85), day)
	horizon = horizon.lerp(Color(0.98, 0.62, 0.40), sunset * 0.55)
	sky.sky_top_color = top
	sky.sky_horizon_color = horizon
	sky.ground_horizon_color = horizon
	sky.ground_bottom_color = top.darkened(0.3)
	env.ambient_light_energy = lerpf(0.2, 0.55, day)
	env.fog_light_color = horizon
	var nf := 1.0 - smoothstep(-0.06, 0.1, elev)
	for m in night_materials:
		m.set_shader_parameter("night_factor", nf)
