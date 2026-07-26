#!/usr/bin/env python3
"""
IdleBot Asset Downloader
Downloads free 3D low-poly assets from OpenGameArt and other sources
for the IdleBot game project.

License: CC0 - Public Domain
"""

import os
import sys
import subprocess
import shutil
import zipfile
import tarfile
from pathlib import Path
from typing import List, Dict, Optional
import urllib.request
import urllib.error
import ssl

# Disable SSL verification for some sites (OGA uses old certs)
ssl._create_default_https_context = ssl._create_unverified_context


class AssetDownloader:
    """Download game assets from OpenGameArt and other sources"""
    
    def __init__(self, output_dir: Path):
        self.output_dir = output_dir
        self.temp_dir = output_dir / "temp"
        self.downloaded = []
        
    def setup(self):
        """Create directory structure"""
        self.output_dir.mkdir(parents=True, exist_ok=True)
        self.temp_dir.mkdir(parents=True, exist_ok=True)
        print(f"✓ Setup complete - downloading to: {self.output_dir}")
    
    def download_file(self, url: str, filename: str) -> Optional[Path]:
        """Download a file from URL to temp directory"""
        filepath = self.temp_dir / filename
        try:
            print(f"  Downloading: {filename}...")
            urllib.request.urlretrieve(url, filepath)
            
            # Verify file size > 0
            if filepath.stat().st_size == 0:
                print(f"  ✗ Failed: File is empty")
                filepath.unlink()
                return None
                
            print(f"  ✓ Downloaded: {filepath.name} ({filepath.stat().st_size / 1024 / 1024:.1f} MB)")
            self.downloaded.append(filepath)
            return filepath
            
        except urllib.error.URLError as e:
            print(f"  ✗ Failed: {e}")
            return None
        except Exception as e:
            print(f"  ✗ Error: {e}")
            return None
    
    def extract_archive(self, archive_path: Path, extract_to: Path) -> bool:
        """Extract zip or tar archive"""
        try:
            if archive_path.suffix == '.zip':
                with zipfile.ZipFile(archive_path, 'r') as zip_ref:
                    zip_ref.extractall(extract_to)
            elif archive_path.suffix in ['.tar.gz', '.tgz']:
                with tarfile.open(archive_path, 'r:gz') as tar_ref:
                    tar_ref.extractall(extract_to)
            else:
                print(f"  ✗ Unsupported format: {archive_path.suffix}")
                return False
                
            print(f"  ✓ Extracted: {archive_path.name}")
            return True
            
        except Exception as e:
            print(f"  ✗ Extraction failed: {e}")
            return False
    
    def organize_files(self, source_dir: Path, target_subdir: str):
        """Move extracted files to proper location"""
        target_dir = self.output_dir / target_subdir
        
        # Find all model files (FBX, OBJ, GLB, blend)
        model_extensions = {'.fbx', '.obj', '.glb', '.blend', '.gltf', '.dae'}
        texture_extensions = {'.png', '.jpg', '.jpeg', '.tga', '.dds'}
        
        extracted = list(source_dir.rglob("*"))
        
        models = [f for f in extracted if f.suffix.lower() in model_extensions]
        textures = [f for f in extracted if f.suffix.lower() in texture_extensions]
        
        if not models:
            print(f"  ✗ No 3D models found in {source_dir.name}")
            return
            
        print(f"  Found {len(models)} models, {len(textures)} textures")
        
        # Create target structure
        (target_dir / "models").mkdir(parents=True, exist_ok=True)
        (target_dir / "textures").mkdir(parents=True, exist_ok=True)
        
        # Copy models
        for model in models[:50]:  # Limit to 50 models per pack
            if model.is_file():
                dest = target_dir / "models" / model.name
                shutil.copy2(model, dest)
                
        # Copy textures
        for texture in textures[:100]:  # Limit to 100 textures
            if texture.is_file():
                dest = target_dir / "textures" / texture.name
                shutil.copy2(texture, dest)
        
        print(f"  ✓ Organized: {len(models)} models, {len(textures)} textures")
    
    def cleanup(self):
        """Remove temp files"""
        if self.temp_dir.exists():
            shutil.rmtree(self.temp_dir)
            print(f"✓ Cleanup complete")
    
    def run(self):
        """Execute all downloads"""
        print("\n" + "="*60)
        print("IDLEBOT ASSET DOWNLOADER")
        print("="*60 + "\n")
        
        # OpenGameArt Assets (CC0 - Public Domain)
        oga_assets = [
            # Low Poly Terrain
            {
                "name": "Low Poly Nature Pack",
                "url": "https://opengameart.org/sites/default/files/low-poly-nature-pack.zip",
                "category": "terrain"
            },
            # Trees
            {
                "name": "Low Poly Trees",
                "url": "https://opengameart.org/sites/default/files/low-poly-trees.zip",
                "category": "nature"
            },
            # Vehicles
            {
                "name": "Low Poly Vehicles",
                "url": "https://opengameart.org/sites/default/files/low-poly-vehicles-set.zip",
                "category": "vehicles"
            },
            # Farming
            {
                "name": "Farming Tools",
                "url": "https://opengameart.org/sites/default/files/farming-tools.zip",
                "category": "farming"
            },
        ]
        
        # Additional sources
        extra_assets = [
            {
                "name": "Sketchfab Free Low Poly",
                "url": "https://sketchfab.com/feed/search?features=downloadable&query=low+poly",
                "category": "misc"
            },
        ]
        
        print("📦 Downloading OpenGameArt Assets (CC0)...")
        for asset in oga_assets:
            print(f"\n📁 {asset['name']}")
            filepath = self.download_file(asset['url'], f"{asset['name'].lower().replace(' ', '-')}.zip")
            
            if filepath:
                # Extract
                extract_dir = self.temp_dir / asset['name'].lower().replace(' ', '-')
                if self.extract_archive(filepath, extract_dir):
                    # Organize
                    self.organize_files(extract_dir, asset['category'])
                
                # Cleanup archive
                filepath.unlink()
        
        # Extra assets (manual download recommended)
        print("\n\n🎨 Additional Sources (Manual Download Recommended):")
        print("-" * 60)
        for asset in extra_assets:
            print(f"  • {asset['name']}: {asset['url']}")
        
        print("\n" + "="*60)
        print("⚠️  IMPORTANT NOTES:")
        print("="*60)
        print("  1. Some OGA assets may require manual download")
        print("  2. Always check individual license terms")
        print("  3. Assets are CC0 (Public Domain) unless stated otherwise")
        print("  4. Verify file integrity before using in production")
        print("\n" + "="*60)
        print(f"✓ Download complete! Files saved to: {self.output_dir}")
        print("="*60 + "\n")
        
        self.cleanup()


def main():
    """Main entry point"""
    output_dir = Path(__file__).parent.parent / "idlebot" / "assets" / "downloaded"
    
    downloader = AssetDownloader(output_dir)
    downloader.setup()
    downloader.run()


if __name__ == "__main__":
    main()
