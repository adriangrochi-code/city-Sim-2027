extends RefCounted
## Parcelas del mapa: cuadrícula de 8 × 8 parcelas de 3 × 3 km.
## Se empieza con una y las demás se desbloquean con la población y se compran.

signal changed

const TILES := 8
const TILE_SIZE := 3000.0
const BASE_PRICE := 6000

var owned := {}
var start_tile := Vector2i(4, 5)
var max_owned := 1


func _init() -> void:
	owned[start_tile] = true


func in_map(t: Vector2i) -> bool:
	return t.x >= 0 and t.y >= 0 and t.x < TILES and t.y < TILES


func is_owned_tile(t: Vector2i) -> bool:
	return owned.has(t)


func tile_of(p: Vector2) -> Vector2i:
	return Vector2i(floori(p.x / TILE_SIZE), floori(p.y / TILE_SIZE))


func is_owned_pos(p: Vector2) -> bool:
	return owned.has(tile_of(p))


func is_near_owned(t: Vector2i) -> bool:
	for dy in range(-1, 2):
		for dx in range(-1, 2):
			if owned.has(t + Vector2i(dx, dy)):
				return true
	return false


func is_adjacent_owned(t: Vector2i) -> bool:
	for d in [Vector2i(1, 0), Vector2i(-1, 0), Vector2i(0, 1), Vector2i(0, -1)]:
		if owned.has(t + d):
			return true
	return false


## Devuelve "" si se puede comprar, o el motivo si no.
func buy_problem(t: Vector2i) -> String:
	if not in_map(t):
		return "Fuera del mapa"
	if owned.has(t):
		return "Esta parcela ya es tuya"
	if not is_adjacent_owned(t):
		return "Debe estar junto a una parcela tuya"
	if owned.size() >= max_owned:
		return "Necesitas más población para recibir otra merced de tierras"
	return ""


func price() -> int:
	return BASE_PRICE * owned.size()


func buy(t: Vector2i) -> void:
	owned[t] = true
	changed.emit()


func tile_rect(t: Vector2i) -> Rect2:
	return Rect2(Vector2(t) * TILE_SIZE, Vector2(TILE_SIZE, TILE_SIZE))


func mask_texture() -> ImageTexture:
	var img := Image.create_empty(TILES, TILES, false, Image.FORMAT_R8)
	for y in TILES:
		for x in TILES:
			img.set_pixel(x, y, Color(1, 0, 0) if owned.has(Vector2i(x, y)) else Color(0, 0, 0))
	return ImageTexture.create_from_image(img)
