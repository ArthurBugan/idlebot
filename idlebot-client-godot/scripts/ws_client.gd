extends Node
## Low-level WebSocket client for the `idlebot-godot-bridge`.
##
## The bridge protocol is JSON over WebSocket: every message is an object
## with a `type` field (snake_case). This node wraps the raw socket, parses
## the JSON, and emits typed signals so callers never touch the wire format.
##
## Requires Godot 4.3+ (uses the `WebSocket2` class).
##
## Client -> bridge: connect, login, update_profile, move, heartbeat,
## plant, harvest, clean, claim_idle, teleport.
## Bridge -> client: connected, error, row, reducer, teleport.

signal socket_connected()
signal connected(identity: String)
signal error(message: String)
signal row(table: String, op: String, data: Variant)
signal reducer(name: String, ok: bool, message: String)
signal teleported(q: int, r: int)

const DEFAULT_URL := "ws://127.0.0.1:9400"

var ws: WebSocket2 = null
var _json: JSON = null

## Open a WebSocket connection to the bridge. Returns false on failure.
func open(url: String = DEFAULT_URL) -> bool:
	if ws != null:
		return true
	if _json == null:
		_json = JSON.new()
	ws = WebSocket2.new()
	var err := ws.connect_to_url(url)
	if err != 0:
		push_error("failed to connect to " + url + " (error " + str(err) + ")")
		ws.free()
		ws = null
		return false
	ws.connected.connect(_on_socket_connected)
	ws.data_received.connect(_on_data)
	ws.error.connect(_on_error)
	ws.connection_closed.connect(_on_closed)
	return true

## Is the underlying socket connected? (Not necessarily to the bridge; see
## the `connected` signal for the bridge-level round-trip.)
func get_is_connected() -> bool:
	return ws != null and ws.get_is_connected()

## Send a JSON object to the bridge. Returns true if the frame was queued.
func send_message(dict: Dictionary) -> bool:
	if ws == null or _json == null:
		return false
	var err := ws.send_text(_json.stringify(dict))
	return err == 0

func _on_socket_connected() -> void:
	emit_signal("socket_connected")
	# Tell the bridge we are open; it replies with {"type":"connected"}.
	send_message({"type": "connect"})

func _on_data(data: PackedByteArray, is_text: bool, is_compressed: bool) -> void:
	if not is_text or _json == null:
		return
	var parsed: Variant = _json.parse(data.get_string_from_utf8())
	if not (parsed is Dictionary):
		push_warning("bridge sent non-JSON data")
		return
	var msg: Dictionary = parsed
	match msg.get("type", ""):
		"connected":
			emit_signal("connected", msg.get("identity", ""))
		"error":
			emit_signal("error", msg.get("message", ""))
		"row":
			emit_signal("row", msg.get("table", ""), msg.get("op", ""), msg.get("data", null))
		"reducer":
			emit_signal("reducer", msg.get("name", ""), msg.get("ok", false), msg.get("message", ""))
		"teleport":
			emit_signal("teleported", msg.get("q", 0), msg.get("r", 0))
		_:
			push_warning("unknown bridge message type: " + str(msg.get("type", "")))

func _on_error(err: int) -> void:
	emit_signal("error", "websocket error: " + str(err))

func _on_closed(error_code: int, reason: String) -> void:
	emit_signal("error", "websocket closed (" + str(error_code) + "): " + reason)
