extends RefCounted
## Geografía del mapa (prototipo 1): península, bahía, cerro, arroyos y cuchillas.
## La altura sale de una función determinista, así se puede consultar en cualquier punto
## (malla del terreno, calles, solares, árboles y selección con el ratón).

const MAP_SIZE := 24000.0
const TILE_SIZE := 3000.0
const TILES := 8
const COAST_Z := 15600.0        # costa base (al sur está el río)
const BAY_X := 11600.0          # centro de la bahía
const PENINSULA_X := 13500.0    # centro de la península
const CERRO := Vector2(10000.0, 15100.0)
const HASH_CELL := 200.0

var arroyos: Array[PackedVector2Array] = []
var _n_coast := FastNoiseLite.new()
var _n_hills := FastNoiseLite.new()
var _n_detail := FastNoiseLite.new()
var _seg_a := PackedVector2Array()
var _seg_b := PackedVector2Array()
var _seg_hash := {}


func _init(seed_value: int = 1726) -> void:
	_n_coast.seed = seed_value
	_n_coast.frequency = 0.0007
	_n_hills.seed = seed_value + 1
	_n_hills.frequency = 0.00035
	_n_hills.fractal_octaves = 3
	_n_detail.seed = seed_value + 2
	_n_detail.frequency = 0.004
	# Arroyo que desemboca en la bahía y otro al este de la península.
	_add_arroyo(PackedVector2Array([
		Vector2(11600, 14400), Vector2(11450, 13300), Vector2(11800, 12100),
		Vector2(11200, 10800), Vector2(11500, 9200), Vector2(10900, 7600), Vector2(11200, 6000)]))
	_add_arroyo(PackedVector2Array([
		Vector2(16400, 15900), Vector2(16600, 14600), Vector2(16200, 13300),
		Vector2(16900, 11800), Vector2(16500, 10200)]))


## Línea de costa: para cada x, la z donde empieza el agua.
func coast_z(x: float) -> float:
	var z := COAST_Z + _n_coast.get_noise_1d(x) * 380.0
	z -= 1500.0 * exp(-pow((x - BAY_X) / 1100.0, 2.0))
	z += 1700.0 * exp(-pow((x - PENINSULA_X) / 520.0, 6.0))
	return z


func height(x: float, z: float) -> float:
	var d := coast_z(x) - z
	var h: float
	if d < 0.0:
		h = maxf(d * 0.015, -14.0)
	else:
		var inland := clampf(d / 3000.0, 0.0, 1.0)
		h = 0.25 + minf(d * 0.035, 7.0)
		h += (_n_hills.get_noise_2d(x, z) * 0.5 + 0.5) * 48.0 * inland
		h += _n_detail.get_noise_2d(x, z) * 1.2 * clampf(d / 200.0, 0.0, 1.0)
	var dc := Vector2(x, z).distance_squared_to(CERRO)
	h += 135.0 * exp(-dc / (2.0 * 380.0 * 380.0))
	var da := arroyo_distance(x, z)
	if da < 160.0:
		h = lerpf(minf(h, -1.2), h, smoothstep(10.0, 160.0, da))
	return h


func is_land(x: float, z: float) -> bool:
	return height(x, z) > 0.4


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


func _add_arroyo(raw: PackedVector2Array) -> void:
	var pts := _smooth_path(raw, 40.0)
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
