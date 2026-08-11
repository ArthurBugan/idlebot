#!/usr/bin/env python3
"""
FBX to GLB conversion script for Blender.
Converts all FBX files in the models directory to GLB format.
"""

import bpy
import os
import sys

# Configuration
MODELS_DIR = "/Users/arthur/Documents/idlebot/crates/idlecore-client/assets/models"

def main():
    """Process all FBX files and convert to GLB."""
    
    # Find all FBX files
    fbx_files = [f for f in os.listdir(MODELS_DIR) if f.lower().endswith('.fbx')]
    
    if not fbx_files:
        print("No FBX files found!")
        return
    
    print(f"Found {len(fbx_files)} FBX files to convert")
    
    for filename in fbx_files:
        input_path = os.path.join(MODELS_DIR, filename)
        output_path = os.path.join(MODELS_DIR, filename.replace('.fbx', '.glb'))
        
        print(f"\n{'='*60}")
        print(f"Converting: {filename}")
        print(f"{'='*60}")
        
        # Clear scene
        bpy.ops.object.select_all(action='SELECT')
        bpy.ops.object.delete(use_global=False)
        
        # Import FBX
        print(f"Importing: {input_path}")
        bpy.ops.import_scene.fbx(
            filepath=input_path,
            use_anim=False
        )
        
        # Check what was imported
        objects = bpy.data.objects
        print(f"Imported {len(objects)} objects")
        
        # Find armature if it exists
        armature = None
        for obj in objects:
            if obj.type == 'ARMATURE':
                armature = obj
                print(f"Found armature: {armature.name}")
                break
        
        # Select all objects for export
        bpy.ops.object.select_all(action='DESELECT')
        for obj in objects:
            obj.select_set(True)
        
        # Export as GLB
        print(f"Exporting to: {output_path}")
        bpy.ops.export_scene.gltf(
            filepath=output_path,
            export_format='GLB',
            use_selection=True,
            export_animations=False,
            export_apply=True
        )
        
        # Check file size
        if os.path.exists(output_path):
            size_mb = os.path.getsize(output_path) / (1024 * 1024)
            print(f"✓ Exported: {size_mb:.2f} MB")
        else:
            print("✗ Export failed")
        
        # Clean up
        bpy.ops.object.select_all(action='SELECT')
        bpy.ops.object.delete(use_global=False)
        
        # Remove orphan data
        for datablock in bpy.data.meshes:
            bpy.data.meshes.remove(datablock)
        for datablock in bpy.data.materials:
            bpy.data.materials.remove(datablock)
    
    print(f"\n{'='*60}")
    print(f"Conversion complete!")
    print(f"{'='*60}")

if __name__ == '__main__':
    main()
