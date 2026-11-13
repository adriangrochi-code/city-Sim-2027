extends Node3D
## Cámara de constructor de ciudades: WASD para mover, Q/E para girar, rueda para el zoom,
## botón central para orbitar y botón derecho para arrastrar el mapa.

var cam: Camera3D
var gen   # TerrainGen
var bounds := Rect2(0, 0, 24000, 24000)
var yaw := deg_to_rad(20.0)
var pitch := deg_to_rad(-48.0)
var distance := 900.0
var target_distance := 900.0
const MIN_DIST := 15.0
const MAX_DIST := 16000.0
var _rotating := false
var _panning := false


func _ready() -> void:
	cam = Camera3D.new()
	cam.fov = 50.0
	cam.far = 90000.0
	add_child(cam)
	_apply()


func _process(delta: float) -> void:
	var move := Vector2.ZERO
	if Input.is_key_pressed(KEY_W) or Input.is_key_pressed(KEY_UP):
		move.y -= 1.0
	if Input.is_key_pressed(KEY_S) or Input.is_key_pressed(KEY_DOWN):
		move.y += 1.0
	if Input.is_key_pressed(KEY_A) or Input.is_key_pressed(KEY_LEFT):
		move.x -= 1.0
	if Input.is_key_pressed(KEY_D) or Input.is_key_pressed(KEY_RIGHT):
		move.x += 1.0
	if move != Vector2.ZERO:
		var fwd := Vector3(-sin(yaw), 0, -cos(yaw))
		var right := Vector3(cos(yaw), 0, -sin(yaw))
		position += (right * move.x - fwd * move.y) * distance * 0.9 * delta
	if Input.is_key_pressed(KEY_Q):
		yaw += 1.4 * delta
	if Input.is_key_pressed(KEY_E):
		yaw -= 1.4 * delta
	distance = lerpf(distance, target_distance, clampf(delta * 9.0, 0.0, 1.0))
	position.x = clampf(position.x, bounds.position.x, bounds.end.x)
	position.z = clampf(position.z, bounds.position.y, bounds.end.y)
	if gen != null:
		var ground := maxf(gen.height(position.x, position.z), 0.0)
		position.y = lerpf(position.y, ground, clampf(delta * 5.0, 0.0, 1.0))
	_apply()


func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventMouseButton:
		var mb := event as InputEventMouseButton
		if mb.button_index == MOUSE_BUTTON_WHEEL_UP and mb.pressed:
			target_distance = clampf(target_distance * 0.86, MIN_DIST, MAX_DIST)
		elif mb.button_index == MOUSE_BUTTON_WHEEL_DOWN and mb.pressed:
			target_distance = clampf(target_distance * 1.16, MIN_DIST, MAX_DIST)
		elif mb.button_index == MOUSE_BUTTON_MIDDLE:
			_rotating = mb.pressed
		elif mb.button_index == MOUSE_BUTTON_RIGHT:
			_panning = mb.pressed
	elif event is InputEventMouseMotion:
		var mm := event as InputEventMouseMotion
		if _rotating:
			yaw -= mm.relative.x * 0.005
			pitch = clampf(pitch - mm.relative.y * 0.004, deg_to_rad(-88.0), deg_to_rad(-8.0))
		if _panning:
			var fwd := Vector3(-sin(yaw), 0, -cos(yaw))
			var right := Vector3(cos(yaw), 0, -sin(yaw))
			position += (-right * mm.relative.x + fwd * mm.relative.y) * distance * 0.0015


func look_at_point(p: Vector3, dist: float) -> void:
	position = p
	distance = dist
	target_distance = dist
	_apply()


func _apply() -> void:
	rotation = Vector3(pitch, yaw, 0)
	cam.position = Vector3(0, 0, distance)
	cam.near = clampf(distance * 0.004, 0.3, 30.0)
