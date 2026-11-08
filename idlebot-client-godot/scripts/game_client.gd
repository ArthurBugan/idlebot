extends Node2D
## IdleBot Godot client: connects to the idlebot-godot-bridge over WebSocket,
## keeps a local copy of the authoritative `player` table, sends input
## commands (move / heartbeat), and renders every player as a circle.
##
## Controls: WASD or arrow keys to move. The server is authoritative —
## circles just mirror the positions that arrive in `row` messages.

const MOVE_STEP_S := 0.1       # send a move at most every 100 ms
const HEARTBEAT_S := 10.0      # spec FR6: heartbeat every 10 s
const SPEED := 1.0            # world units/s at intended_speed 1.0
const LOGIN_NAME := "godot-client"

const WS_CLIENT_SCRIPT := preload("res://scripts/ws_client.gd")

var ws: Node = null   # an instance of WS_CLIENT_SCRIPT
var my_identity: String = ""
var players: Dictionary = {}   # identity -> player snapshot (Dictionary)
var sprites: Dictionary = {}   # identity -> Sprite2D

var _hb_acc := 0.0
var _move_acc := 0.0

signal player_added(identity: String)
signal player_updated(identity: String)
signal player_removed(identity: String)
signal reducer_result(name: String, ok: bool, message: String)
signal teleported(q: int, r: int)

func _ready() -> void:
	ws = WS_CLIENT_SCRIPT.new()
	add_child(ws)
	if not ws.open():
		push_warning("bridge not reachable; will keep trying in the background")
	ws.connected.connect(_on_connected)
	ws.error.connect(_on_error)
	ws.row.connect(_on_row)
	ws.reducer.connect(_on_reducer)
	ws.teleported.connect(_on_teleported)

func _on_connected(identity: String) -> void:
	my_identity = identity
	print("connected to bridge as ", identity)
	ws.send_message({"type": "login", "name": LOGIN_NAME})

func _on_error(message: String) -> void:
	printerr("bridge error: ", message)

func _on_row(table: String, op: String, data: Variant) -> void:
	if table != "player" or not (data is Dictionary):
		return
	var snap: Dictionary = data
	var pid: String = snap.get("identity", "")
	if pid == "":
		return
	if op == "insert":
		players[pid] = snap
		_add_sprite(pid)
		emit_signal("player_added", pid)
	elif op == "update":
		players[pid] = snap
		_move_sprite(pid)
		emit_signal("player_updated", pid)
	else: # "delete"
		players.erase(pid)
		_remove_sprite(pid)
		emit_signal("player_removed", pid)

func _on_reducer(name: String, ok: bool, message: String) -> void:
	print("reducer [", name, "] ok=", ok, " ", message)
	emit_signal("reducer_result", name, ok, message)

func _on_teleported(q: int, r: int) -> void:
	# Server confirmed our teleport; the following `row` update carries the
	# new position, so there is nothing else to do here.
	print("teleported to hex (", q, ", ", r, ")")
	emit_signal("teleported", q, r)

func _process(delta: float) -> void:
	if ws == null or not ws.get_is_connected():
		return
	_hb_acc += delta
	if _hb_acc >= HEARTBEAT_S:
		_hb_acc = 0.0
		ws.send_message({"type": "heartbeat"})
	var dir := _input_dir()
	_move_acc += delta
	if dir != Vector2.ZERO and _move_acc >= MOVE_STEP_S:
		_move_acc = 0.0
		var unit := dir.normalized()
		var pos := _my_pos()
		ws.send_message({
			"type": "move",
			"dir_x": unit.x,
			"dir_y": unit.y,
			"intended_speed": 1.0,
			"dt": MOVE_STEP_S,
			"to_x": pos.x + unit.x * SPEED * MOVE_STEP_S,
			"to_y": pos.y + unit.y * SPEED * MOVE_STEP_S,
		})

func _input_dir() -> Vector2:
	var dir := Vector2(0.0, 0.0)
	if Input.is_key_pressed(KEY_UP) or Input.is_key_pressed(KEY_W):
		dir.y -= 1.0
	if Input.is_key_pressed(KEY_DOWN) or Input.is_key_pressed(KEY_S):
		dir.y += 1.0
	if Input.is_key_pressed(KEY_LEFT) or Input.is_key_pressed(KEY_A):
		dir.x -= 1.0
	if Input.is_key_pressed(KEY_RIGHT) or Input.is_key_pressed(KEY_D):
		dir.x += 1.0
	return dir

func _my_pos() -> Vector2:
	var me: Variant = players.get(my_identity)
	if me is Dictionary:
		var d: Dictionary = me
		return Vector2(float(d.get("position_x", 0.0)), float(d.get("position_y", 0.0)))
	return Vector2.ZERO

# --- visuals (no asset dependencies; circles are generated at runtime) ---

func _sprite_color(pid: String) -> Color:
	if pid == my_identity:
		return Color(0.9, 0.2, 0.2) # me
	return Color(0.2, 0.6, 0.9)   # other players

func _make_circle(color: Color) -> Sprite2D:
	var img: Image = Image.create(16, 16, false, 2)
	img.fill(color)
	var s: Sprite2D = Sprite2D.new()
	s.texture = img.get_texture_from_image()
	s.centered = true
	s.z_index = 1
	return s

func _add_sprite(pid: String) -> void:
	var s: Sprite2D = _make_circle(_sprite_color(pid))
	add_child(s)
	sprites[pid] = s
	_move_sprite(pid)

func _move_sprite(pid: String) -> void:
	var s: Sprite2D = sprites.get(pid)
	if s == null:
		return
	var me: Variant = players.get(pid)
	if me is Dictionary:
		var d: Dictionary = me
		s.position = Vector2(float(d.get("position_x", 0.0)), float(d.get("position_y", 0.0)))
		s.visible = true
	else:
		s.visible = false

func _remove_sprite(pid: String) -> void:
	var s: Sprite2D = sprites.get(pid)
	if s != null:
		s.queue_free()
	sprites.erase(pid)
