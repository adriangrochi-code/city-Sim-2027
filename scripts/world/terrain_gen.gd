extends RefCounted
## Geografía del mapa. Por omisión es la **geografía real de Montevideo**: la península de la
## Ciudad Vieja, la bahía, el Cerro, los arroyos y la costa del Río de la Plata, a partir de
## datos de elevación reales (ver docs/sistemas/mapa-real.md y tools/mapa_real/).
## La geografía inventada del primer prototipo sigue disponible con `--mapa=ficticio`.
##
## La altura se puede consultar en cualquier punto (malla del terreno, calles, solares,
## árboles y selección con el ratón). Coordenadas: x hacia el este, z hacia el sur, en metros,
## con (0, 0) en la esquina noroeste del mapa jugable.

const MAP_SIZE := 24000.0
const TILE_SIZE := 3000.0
const TILES := 8
const HASH_CELL := 200.0
const MAP_DIR := "res://assets/mapas/montevideo/"

## Mapa real: centro sugerido para la plaza mayor (Ciudad Vieja, junto a la Plaza Matriz) y
## cumbre del Cerro. Los mismos valores están en assets/mapas/montevideo/meta.json.
const FOUNDING_SITE := Vector2(11097.7, 14308.9)
const CERRO := Vector2(5923.7, 12269.8)

## Mapa ficticio del primer prototipo.
const FOUNDING_SITE_FICTICIO := Vector2(13500.0, 16300.0)
const CERRO_FICTICIO := Vector2(10000.0, 15100.0)
const COAST_Z := 15600.0        # costa base (al sur está el río)
const BAY_X := 11600.0          # centro de la bahía
const PENINSULA_X := 13500.0    # centro de la península

## Cauce de los arroyos del mapa real: se hunde un poco el terreno y el agua corre por encima.
const CAUCE_PROFUNDIDAD := 1.6
const CAUCE_ANCHO := 30.0

var real := true
var arroyos: Array[PackedVector2Array] = []
var _n_coast := FastNoiseLite.new()
var _n_hills := FastNoiseLite.new()
var _n_detail := FastNoiseLite.new()
var _seg_a := PackedVector2Array()
var _seg_b := PackedVector2Array()
var _seg_hash := {}
# Datos del mapa real (uint16: metros = valor / 200 − 50).
var _main := PackedByteArray()
var _main_n := 0
var _main_step := 20.0
var _ctx := PackedByteArray()
var _ctx_n := 0
var _ctx_step := 100.0
var _ctx_from := -6000.0


## ¿Se pidió el mapa ficticio por línea de comandos (`--mapa=ficticio`)?
static func ficticio_pedido() -> bool:
	return OS.get_cmdline_user_args().has("--mapa=ficticio") or OS.get_cmdline_args().has("--mapa=ficticio")


## Sitio de fundación según el mapa en uso (sin crear el generador).
static func founding_site_default() -> Vector2:
	return FOUNDING_SITE_FICTICIO if ficticio_pedido() else FOUNDING_SITE


func _init(seed_value: int = 1726) -> void:
	_n_coast.seed = seed_value
	_n_coast.frequency = 0.0007
	_n_hills.seed = seed_value + 1
	_n_hills.frequency = 0.00035
	_n_hills.fractal_octaves = 3
	_n_detail.seed = seed_value + 2
	_n_detail.frequency = 0.004
	real = not ficticio_pedido() and _load_real()
	if real:
		return
	# Mapa ficticio: arroyo que desemboca en la bahía y otro al este de la península.
	_add_arroyo(PackedVector2Array([
		Vector2(11600, 14400), Vector2(11450, 13300), Vector2(11800, 12100),
		Vector2(11200, 10800), Vector2(11500, 9200), Vector2(10900, 7600), Vector2(11200, 6000)]))
	_add_arroyo(PackedVector2Array([
		Vector2(16400, 15900), Vector2(16600, 14600), Vector2(16200, 13300),
		Vector2(16900, 11800), Vector2(16500, 10200)]))


## Centro sugerido para la plaza mayor en el mapa en uso.
func founding_site() -> Vector2:
	return FOUNDING_SITE if real else FOUNDING_SITE_FICTICIO


func cerro() -> Vector2:
	return CERRO if real else CERRO_FICTICIO


# --- Mapa real ----------------------------------------------------------------------------

func _load_real() -> bool:
	var meta_txt := FileAccess.get_file_as_string(MAP_DIR + "meta.json")
	var meta = JSON.parse_string(meta_txt)
	if typeof(meta) != TYPE_DICTIONARY:
		push_warning("No se encontró el mapa real; se usa el mapa ficticio")
		return false
	_main = FileAccess.get_file_as_bytes(MAP_DIR + "altura.bin")
	_ctx = FileAccess.get_file_as_bytes(MAP_DIR + "contexto.bin")
	_main_n = int(meta["muestras"])
	_main_step = float(meta["paso_m"])
	_ctx_n = int(meta["contexto"]["muestras"])
	_ctx_step = float(meta["contexto"]["paso_m"])
	_ctx_from = float(meta["contexto"]["desde_m"])
	if _main.size() != _main_n * _main_n * 2 or _ctx.size() != _ctx_n * _ctx_n * 2:
		push_warning("El mapa real está incompleto; se usa el mapa ficticio")
		return false
	var arr = JSON.parse_string(FileAccess.get_file_as_string(MAP_DIR + "arroyos.json"))
	if typeof(arr) == TYPE_DICTIONARY:
		for line in arr["arroyos"]:
			var pts := PackedVector2Array()
			for p in line:
				pts.append(Vector2(float(p[0]), float(p[1])))
			_register_arroyo(pts)
	return true


static func _sample(bytes: PackedByteArray, n: int, step: float, from: float, x: float, z: float) -> float:
	var fx := clampf((x - from) / step, 0.0, n - 1.001)
	var fz := clampf((z - from) / step, 0.0, n - 1.001)
	var i := int(fz)
	var j := int(fx)
	var tx := fx - j
	var tz := fz - i
	var o := (i * n + j) * 2
	var a := bytes.decode_u16(o)
	var b := bytes.decode_u16(o + 2)
	var c := bytes.decode_u16(o + n * 2)
	var d := bytes.decode_u16(o + n * 2 + 2)
	var v := lerpf(lerpf(a, b, tx), lerpf(c, d, tx), tz)
	return v / 200.0 - 50.0


## Altura del relieve real, sin el cauce de los arroyos.
func raw_height(x: float, z: float) -> float:
	if x >= 0.0 and z >= 0.0 and x <= MAP_SIZE and z <= MAP_SIZE:
		return _sample(_main, _main_n, _main_step, 0.0, x, z)
	return _sample(_ctx, _ctx_n, _ctx_step, _ctx_from, x, z)


## Nivel del agua de un arroyo en ese punto (para dibujar el agua que corre por el cauce).
func arroyo_water_height(x: float, z: float) -> float:
	if not real:
		return 0.0
	return raw_height(x, z) - 0.2


# --- Consultas comunes ---------------------------------------------------------------------

func height(x: float, z: float) -> float:
	if not real:
		return _height_ficticio(x, z)
	var h := raw_height(x, z)
	if h > 0.3:
		var da := arroyo_distance(x, z)
		if da < CAUCE_ANCHO:
			h -= CAUCE_PROFUNDIDAD * (1.0 - smoothstep(8.0, CAUCE_ANCHO, da))
	return h


func is_land(x: float, z: float) -> bool:
	return height(x, z) > 0.4


## Agua del río o de un arroyo (para no construir encima).
func is_water(x: float, z: float) -> bool:
	return height(x, z) <= 0.4 or (real and arroyo_distance(x, z) < 7.0)


func arroyo_distance(x: float, z: float) -> float:
	var p := Vector2(x, z)
	var c := Vector2i(floori(x / HASH_CELL), floori(z / HASH_CELL))
	var best := 1e9
	for dy in range(-1, 2):
		for dx in range(-1, 2):
			var k := c + Vector2i(dx, dy)
			if not _seg_hash.has(k):
				continue
			var arr: PackedInt32Array = _seg_hash[k]
			for si in arr:
				var q := Geometry2D.get_closest_point_to_segment(p, _seg_a[si], _seg_b[si])
				best = minf(best, p.distance_to(q))
	return best


func _register_arroyo(pts: PackedVector2Array) -> void:
	arroyos.append(pts)
	for i in pts.size() - 1:
		var a := pts[i]
		var b := pts[i + 1]
		var si := _seg_a.size()
		_seg_a.append(a)
		_seg_b.append(b)
		var mn := Vector2i(floori(minf(a.x, b.x) / HASH_CELL), floori(minf(a.y, b.y) / HASH_CELL))
		var mx := Vector2i(floori(maxf(a.x, b.x) / HASH_CELL), floori(maxf(a.y, b.y) / HASH_CELL))
		for cy in range(mn.y, mx.y + 1):
			for cx in range(mn.x, mx.x + 1):
				var k := Vector2i(cx, cy)
				var arr: PackedInt32Array = _seg_hash.get(k, PackedInt32Array())
				arr.append(si)
				_seg_hash[k] = arr


# --- Mapa ficticio (primer prototipo) -------------------------------------------------------

## Línea de costa ficticia: para cada x, la z donde empieza el agua.
func coast_z(x: float) -> float:
	var z := COAST_Z + _n_coast.get_noise_1d(x) * 380.0
	z -= 1500.0 * exp(-pow((x - BAY_X) / 1100.0, 2.0))
	z += 1700.0 * exp(-pow((x - PENINSULA_X) / 520.0, 6.0))
	return z


func _height_ficticio(x: float, z: float) -> float:
	var d := coast_z(x) - z
	var h: float
	if d < 0.0:
		h = maxf(d * 0.015, -14.0)
	else:
		var inland := clampf(d / 3000.0, 0.0, 1.0)
		h = 0.25 + minf(d * 0.035, 7.0)
		h += (_n_hills.get_noise_2d(x, z) * 0.5 + 0.5) * 48.0 * inland
		h += _n_detail.get_noise_2d(x, z) * 1.2 * clampf(d / 200.0, 0.0, 1.0)
	var dc := Vector2(x, z).distance_squared_to(CERRO_FICTICIO)
	h += 135.0 * exp(-dc / (2.0 * 380.0 * 380.0))
	var da := arroyo_distance(x, z)
	if da < 160.0:
		h = lerpf(minf(h, -1.2), h, smoothstep(10.0, 160.0, da))
	return h


func _add_arroyo(raw: PackedVector2Array) -> void:
	_register_arroyo(_smooth_path(raw, 40.0))


## Suaviza el trazado con Catmull-Rom y le agrega meandros con ruido.
func _smooth_path(raw: PackedVector2Array, step: float) -> PackedVector2Array:
	var out := PackedVector2Array()
	var last := raw.size() - 1
	for i in last:
		var p0 := raw[maxi(i - 1, 0)]
		var p1 := raw[i]
		var p2 := raw[i + 1]
		var p3 := raw[mini(i + 2, last)]
		var n := maxi(2, int(p1.distance_to(p2) / step))
		var tan := (p2 - p1).normalized()
		var perp := Vector2(-tan.y, tan.x)
		for k in n:
			var t := float(k) / n
			var t2 := t * t
			var t3 := t2 * t
			var q := 0.5 * ((2.0 * p1) + (p2 - p0) * t + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2 + (3.0 * p1 - p0 - 3.0 * p2 + p3) * t3)
			# Sin meandro en la desembocadura para que conecte con el agua.
			var fade := 0.0 if i == 0 and k < n / 2 else 1.0
			q += perp * _n_detail.get_noise_2d(q.x * 0.05, q.y * 0.05) * 70.0 * fade
			out.append(q)
	out.append(raw[last])
	return out
