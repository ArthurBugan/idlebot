extends SceneTree
## Headless end-to-end test for the Godot-side WS client.
##
## Run from the project root with the bridge running on 127.0.0.1:9400:
##   godot --headless --script res://tests/headless_ws_test.gd
##
## Exits with code 0 on PASS (bridge round-trip worked), non-zero on failure.

var ws = null               # instance of res://scripts/ws_client.gd
var state := "idle"          # idle | connected | failed
var login_done := false
var quit_code := 0

const TIMEOUT_S := 20.0

var _elapsed := 0.0

func _initialize() -> void:
	print("== godot ws headless test ==")
	ws = load("res://scripts/ws_client.gd").new()
	ws.connected.connect(_on_connected)
	ws.error.connect(_on_error)
	ws.row.connect(_on_row)
	ws.reducer.connect(_on_reducer)
	ws.teleported.connect(_on_teleported)
	if not ws.open():
		print("FAIL: could not open WebSocket")
		state = "failed"

func _process(delta: float) -> bool:
	_elapsed += delta
	if state == "idle" and _elapsed > TIMEOUT_S:
		print("FAIL: timeout waiting for bridge")
		state = "failed"
	if state == "failed":
		quit_code = 1
		return true
	if state == "connected" and login_done:
		print("== PASS ==")
		quit_code = 0
		return true
	return false

func _on_connected(identity: String) -> void:
	state = "connected"
	print("connected to bridge as ", identity)
	ws.send_message({"type": "login", "name": "headless-test"})

func _on_error(message: String) -> void:
	print("FAIL: bridge error: ", message)
	state = "failed"

func _on_row(table: String, op: String, data: Variant) -> void:
	print("row ", table, " op=", op, " data=", data)

func _on_reducer(name: String, ok: bool, message: String) -> void:
	print("reducer [", name, "] ok=", ok, " ", message)
	if name == "login":
		login_done = true

func _on_teleported(q: int, r: int) -> void:
	print("teleported to hex (", q, ", ", r, ")")
