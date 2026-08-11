import bpy
import os
import sys

# ============================================================
# CONFIG
# ============================================================

# Overridable via CLI:  blender --background --python convert_fbx_to_glb.py -- \
#     --base path/to/model.fbx --out path/to/out.glb --anim-dir path/to/animations
def config_from_args(argv):
    base_fbx = "/Users/arthur/Documents/idlebot/crates/idlecore-client/assets/models/characterLargeMale.fbx"
    anim_dir = "/Users/arthur/Documents/idlebot/crates/idlecore-client/assets/models/"
    output_glb = "/Users/arthur/Documents/idlebot/crates/idlecore-client/assets/models/characterLargeMale.glb"

    args = argv[argv.index("--") + 1:] if "--" in argv else []
    i = 0
    while i < len(args):
        arg, i = args[i], i + 1
        if arg == "--base" and i < len(args):
            base_fbx, i = args[i], i + 1
        elif arg == "--anim-dir" and i < len(args):
            anim_dir, i = args[i], i + 1
        elif arg == "--out" and i < len(args):
            output_glb, i = args[i], i + 1
        else:
            print(f"Unknown argument: {arg}")

    return base_fbx, anim_dir, output_glb

BASE_FBX, ANIMATIONS_DIR, OUTPUT_GLB = config_from_args(sys.argv)

# ============================================================
# HELPERS
# ============================================================

def clear_scene():
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete(use_global=False)

    # Remove orphan data
    for datablocks in (
        bpy.data.meshes,
        bpy.data.curves,
        bpy.data.materials,
        bpy.data.cameras,
        bpy.data.lights,
    ):
        for block in list(datablocks):
            if block.users == 0:
                datablocks.remove(block)


def import_fbx(path):
    print(f"Importing: {path}")

    before = set(bpy.data.objects)

    bpy.ops.import_scene.fbx(
        filepath=path,
        use_anim=True,
    )

    after = set(bpy.data.objects)

    return list(after - before)


def find_armature(objects):
    for obj in objects:
        if obj.type == "ARMATURE":
            return obj

    return None


def find_meshes(objects):
    return [
        obj for obj in objects
        if obj.type == "MESH"
    ]


def remove_objects(objects):
    bpy.ops.object.select_all(action="DESELECT")

    for obj in objects:
        obj.select_set(True)

    bpy.context.view_layer.objects.active = None

    bpy.ops.object.delete(use_global=False)


def get_animation_action(armature):
    """
    Returns the Action created when importing an animation FBX.
    """
    if armature.animation_data and armature.animation_data.action:
        return armature.animation_data.action

    return None


def copy_action_to_armature(source_armature, target_armature, name):
    source_action = get_animation_action(source_armature)

    if not source_action:
        print(f"WARNING: No animation found for {name}")
        return None

    # Copy the Action so deleting the imported armature
    # doesn't destroy the animation.
    action = source_action.copy()
    action.name = name

    # Make sure target has animation data.
    if not target_armature.animation_data:
        target_armature.animation_data_create()

    # Assign temporarily.
    target_armature.animation_data.action = action

    return action


def add_action_to_nla(armature, action, name):
    """
    Adds an Action to an NLA track.

    Blender's glTF exporter can use NLA track names
    as animation names.
    """

    if not armature.animation_data:
        armature.animation_data_create()

    # Clear active action.
    armature.animation_data.action = None

    track = armature.animation_data.nla_tracks.new()
    track.name = name

    frame_start = int(action.frame_range[0])
    frame_end = int(action.frame_range[1])

    strip = track.strips.new(
        name=name,
        start=frame_start,
        action=action,
    )

    strip.action_frame_start = action.frame_range[0]
    strip.action_frame_end = action.frame_range[1]

    strip.frame_start = frame_start
    strip.frame_end = frame_end

    strip.extrapolation = "NOTHING"

    return track


# ============================================================
# MAIN
# ============================================================

def main():

    print("======================================")
    print("FBX → GLB Animation Combiner")
    print("======================================")

    clear_scene()

    # --------------------------------------------------------
    # 1. IMPORT BASE CHARACTER
    # --------------------------------------------------------

    base_objects = import_fbx(BASE_FBX)

    base_armature = find_armature(base_objects)

    if not base_armature:
        raise RuntimeError(
            "Could not find armature in base FBX."
        )

    print(f"Base armature: {base_armature.name}")

    # Keep the base character meshes.
    base_meshes = find_meshes(base_objects)

    # --------------------------------------------------------
    # 2. REMOVE BASE ANIMATION
    # --------------------------------------------------------

    if base_armature.animation_data:
        base_armature.animation_data.action = None

        # Remove existing NLA tracks
        for track in list(base_armature.animation_data.nla_tracks):
            base_armature.animation_data.nla_tracks.remove(track)

    # --------------------------------------------------------
    # 3. PROCESS ANIMATION FBXS
    # --------------------------------------------------------

    animation_files = [
        f
        for f in os.listdir(ANIMATIONS_DIR)
        if f.lower().endswith(".fbx")
    ]

    animation_files.sort()

    if not animation_files:
        raise RuntimeError(
            f"No FBX files found in {ANIMATIONS_DIR}"
        )

    print(f"Found {len(animation_files)} animations.")

    for filename in animation_files:

        path = os.path.join(
            ANIMATIONS_DIR,
            filename
        )

        animation_name = os.path.splitext(filename)[0]

        print()
        print("--------------------------------------")
        print(f"Processing: {animation_name}")
        print("--------------------------------------")

        imported_objects = import_fbx(path)

        source_armature = find_armature(imported_objects)

        if not source_armature:
            print(
                f"WARNING: No armature found in {filename}"
            )
            remove_objects(imported_objects)
            continue

        source_action = get_animation_action(
            source_armature
        )

        if not source_action:
            print(
                f"WARNING: No animation found in {filename}"
            )
            remove_objects(imported_objects)
            continue

        # Copy animation to base armature.
        action = source_action.copy()
        action.name = animation_name

        # Add to NLA.
        add_action_to_nla(
            base_armature,
            action,
            animation_name
        )

        print(
            f"Added animation: {animation_name}"
        )

        # Delete imported animation character.
        remove_objects(imported_objects)

    # --------------------------------------------------------
    # 4. SELECT BASE CHARACTER
    # --------------------------------------------------------

    bpy.ops.object.select_all(action="DESELECT")

    for obj in base_meshes:
        obj.select_set(True)

    base_armature.select_set(True)

    bpy.context.view_layer.objects.active = base_armature

    # --------------------------------------------------------
    # 5. EXPORT GLB
    # --------------------------------------------------------

    print()
    print("======================================")
    print("Exporting GLB...")
    print("======================================")

    os.makedirs(
        os.path.dirname(OUTPUT_GLB),
        exist_ok=True
    )

    bpy.ops.export_scene.gltf(
        filepath=OUTPUT_GLB,

        # GLB instead of separate .gltf + assets
        export_format="GLB",

        # Include animations
        export_animations=True,

        # Use NLA tracks as animations
        export_animation_mode="NLA_TRACKS",

        # Export only selected objects
        use_selection=True,

        # Don't export cameras/lights
        export_cameras=False,
        export_lights=False,

        # Apply transforms
        export_apply=True,

        # Keep materials
        export_materials="EXPORT",

        # Compression
        export_image_format="AUTO",
    )

    print()
    print("======================================")
    print("DONE")
    print("======================================")
    print(f"Output: {OUTPUT_GLB}")

    print()
    print("Animations:")

    for track in base_armature.animation_data.nla_tracks:
        strip = track.strips[0] if track.strips else None
        if strip:
            duration = strip.frame_end - strip.frame_start
            print(f"  - {track.name} (frames {strip.frame_start:.0f}-{strip.frame_end:.0f}, duration={duration:.1f}f)")
        else:
            print(f"  - {track.name} (NO STRIP)")


if __name__ == "__main__":
    main()
