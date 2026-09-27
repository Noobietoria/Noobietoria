# Headless scene validation: checks that every .tscn in the project only
# references existing resources, then loads and instantiates each scene.
# Exits 1 on any failure. Run from CI or locally:
#   godot --headless --path . --script res://tools/validate_scenes.gd
extends SceneTree

var _ext_resource_path := RegEx.create_from_string("path=\"([^\"]+)\"")


func _init() -> void:
	var scenes := _list_scenes("res://")
	var failures := 0
	for path in scenes:
		# PackedScene.load() silently tolerates missing dependencies (it only
		# logs an ERROR), so check every ext_resource path exists on disk first.
		for res_path in _ext_resources(path):
			if not FileAccess.file_exists(res_path):
				push_error("%s: missing ext_resource: %s" % [path, res_path])
				failures += 1

		var packed: PackedScene = load(path)
		if packed == null:
			push_error("Cannot load scene: " + path)
			failures += 1
			continue
		var node := packed.instantiate()
		if node == null:
			push_error("Cannot instantiate scene: " + path)
			failures += 1
		else:
			node.free()

	if failures > 0:
		printerr("VALIDATION FAILED: %d problem(s) across %d scene(s)." % [failures, scenes.size()])
		quit(1)
	else:
		print("VALIDATION OK: %d scene(s) checked." % scenes.size())
		quit(0)


func _ext_resources(scene_path: String) -> PackedStringArray:
	var result := PackedStringArray()
	var file := FileAccess.open(scene_path, FileAccess.READ)
	if file == null:
		return result
	while not file.eof_reached():
		var line := file.get_line()
		if line.begins_with("[ext_resource"):
			for m in _ext_resource_path.search_all(line):
				result.append(m.get_string(1))
	return result


func _list_scenes(dir_path: String) -> PackedStringArray:
	var result := PackedStringArray()
	var dir := DirAccess.open(dir_path)
	if dir == null:
		return result
	dir.list_dir_begin()
	var entry := dir.get_next()
	while entry != "":
		if dir.current_is_dir() and not entry.begins_with("."):
			result.append_array(_list_scenes(dir_path.path_join(entry)))
		elif entry.ends_with(".tscn"):
			result.append(dir_path.path_join(entry))
		entry = dir.get_next()
	dir.list_dir_end()
	return result
