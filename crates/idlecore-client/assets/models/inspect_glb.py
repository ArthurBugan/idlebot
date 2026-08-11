#!/usr/bin/env python3
"""Inspect a GLB file for animation data."""

import json
import struct
import sys

def inspect_glb(filepath):
    print(f"Inspecting: {filepath}")
    
    with open(filepath, 'rb') as f:
        # Read GLB header
        magic = f.read(4)
        if magic != b'glTF':
            print(f"ERROR: Not a valid GLB file (magic: {magic})")
            return
        
        version = struct.unpack('<I', f.read(4))[0]
        length = struct.unpack('<I', f.read(4))[0]
        
        print(f"Version: {version}")
        print(f"Length: {length} bytes")
        
        # Read chunks
        while f.tell() < length:
            chunk_len = struct.unpack('<I', f.read(4))[0]
            chunk_type = f.read(4)
            
            if chunk_type == b'JSON':
                print("\n=== JSON CHUNK ===")
                data = f.read(chunk_len)
                
                # Find the start of JSON
                json_start = data.find(b'{')
                if json_start < 0:
                    print("ERROR: Could not find JSON object")
                    continue
                
                json_bytes = data[json_start:]
                
                # Find the end of JSON by matching braces.
                # GLB pads the JSON chunk with 0x20 (space) up to 4-byte
                # alignment, so strip trailing padding first.
                json_bytes = json_bytes.rstrip(b' \t\r\n\0')
                
                depth = 0
                end = 0
                for i, b in enumerate(json_bytes):
                    if b == ord('{'):
                        depth += 1
                    elif b == ord('}'):
                        depth -= 1
                        if depth == 0:
                            end = i + 1
                            break
                
                if end == 0:
                    print("ERROR: Could not find end of JSON")
                    continue
                
                json_str = json_bytes[:end].decode('utf-8', errors='ignore')
                
                try:
                    obj = json.loads(json_str)
                except json.JSONDecodeError as e:
                    print(f"ERROR: Invalid JSON: {e}")
                    continue
                
                # Display structure
                print(f"Top-level keys: {list(obj.keys())}")
                
                # Check for animations
                if 'animations' in obj:
                    anims = obj['animations']
                    print(f"\n=== ANIMATIONS: {len(anims)} ===")
                    for i, a in enumerate(anims):
                        name = a.get('name', 'unnamed')
                        channels = a.get('channels', [])
                        samplers = a.get('samplers', [])
                        print(f"\n  [{i}] {name}")
                        print(f"      Channels: {len(channels)}, Samplers: {len(samplers)}")
                        if channels:
                            for ch in channels[:3]:  # Show first 3 channels
                                tgt = ch.get('target', {})
                                print(f"      -> {tgt.get('name', '?')} ({tgt.get('path', '?')})")
                else:
                    print("\nNO ANIMATIONS in GLB!")
                
                # Display basic counts
                print(f"\n=== STRUCTURE ===")
                print(f"Meshes: {len(obj.get('meshes', []))}")
                print(f"Nodes: {len(obj.get('nodes', []))}")
                print(f"Accessors: {len(obj.get('accessors', []))}")
                print(f"Samplers: {len(obj.get('samplers', []))}")
                
            else:
                # Skip binary chunks (chunkLength bytes of data)
                f.read(chunk_len)
                if chunk_type == b'BIN\0':
                    print(f"\nBinary chunk: {chunk_len - 8} bytes")

if __name__ == '__main__':
    if len(sys.argv) > 1:
        inspect_glb(sys.argv[1])
    else:
        print("Usage: inspect_glb.py <file.glb>")
