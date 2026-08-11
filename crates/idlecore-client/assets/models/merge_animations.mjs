#!/usr/bin/env node
/**
 * Merge per-file FBX2glTF animation GLBs into a character GLB.
 *
 * Pipeline for Kenney "Animated Characters" style packs
 * (character FBX + one FBX per animation, all sharing one skeleton):
 *
 *   1.  brew install node && npm i -g @gltf-transform/core @gltf-transform/functions
 *       (or npm i in this folder)
 *   2.  FBX2glTF -b -k position -k normal -k color -k uv0 characterLargeMale.fbx -o characterLargeMale
 *       FBX2glTF -b idle.fbx -o idle   (repeat for every animation FBX)
 *       NOTE: `-k uv0` is required — FBX2glTF drops UVs when the FBX
 *       material references no texture, which breaks skin textures later.
 *   3.  node merge_animations.mjs characterLargeMale.glb ./dir-of-anims characterLargeMale_out.glb
 *   4.  inspect_glb.py characterLargeMale_out.glb   # optional sanity check
 *
 * All files are FBX2glTF output, so the bone hierarchy (and node names) is
 * identical everywhere; animation channels are retargeted onto the character
 * by node name. The "Root|0.Targeting Pose" tracks are dropped.
 * UVs are kept via prune({ keepAttributes: true }).
 *
 * Requires: node 18+, @gltf-transform/core, @gltf-transform/functions.
 */

import { NodeIO } from '@gltf-transform/core';
import {
    dedup as dedupAction,
    mergeDocuments,
    prune as pruneAction,
} from '@gltf-transform/functions';
import { readdir } from 'node:fs/promises';
import { extname, join, resolve } from 'node:path';

const io = new NodeIO();

const [charPath, animDir, outPath] = process.argv.slice(2);
if (!charPath || !animDir || !outPath) {
    console.error('Usage: node merge_animations.mjs <character.glb> <animDir> <out.glb>');
    process.exit(1);
}

const character = await io.read(resolve(charPath));
const root = character.getRoot();

// Node lookup for the character scene, by name.
const charNodes = new Map();
for (const scene of root.listScenes()) {
    scene.traverse((node) => {
        if (!charNodes.has(node.getName())) charNodes.set(node.getName(), node);
    });
}
console.log(`Character: ${root.listNodes().length} nodes, ${root.listScenes().length} scene(s)`);

// The real clip inside each animation file is the one not named "*Targeting*".
const files = (await readdir(animDir)).filter((f) => extname(f).toLowerCase() === '.glb');
let merged = 0;
let skipped = 0;

for (const file of files.sort()) {
    const doc = await io.read(join(resolve(animDir), file));
    const sourceAnim = doc.getRoot().listAnimations().find((a) => !a.getName().includes('Targeting'));
    if (!sourceAnim) {
        console.log(`SKIP ${file}: no non-Targeting animation found`);
        skipped++;
        continue;
    }

    const clipName = sourceAnim.getName().replace(/^Root\|/, '');
    doc.getRoot().setDefaultScene(null);
    mergeDocuments(character, doc);

    // mergeDocuments deep-copies into `character`; fetch the copy from there.
    const anim = root.listAnimations().find((a) => a.getName() === sourceAnim.getName());
    anim.setName(clipName.replace(/^./, (c) => c.toLowerCase()));

    let bound = 0;
    for (const channel of anim.listChannels()) {
        const target = channel.getTargetNode();
        const host = target && charNodes.get(target.getName());
        if (!host) {
            console.warn(`  ${file}: no matching node for "${target?.getName() ?? '?'}"`);
            continue;
        }
        channel.setTargetNode(host);
        bound++;
    }
    console.log(`+ ${clipName} (${bound}/${anim.listChannels().length} channels bound)`);
    merged++;
}

root.listAnimations().forEach((a) => {
    if (a.getName().includes('Targeting')) a.dispose();
});

// v4: accessors link straight to buffers; move every accessor onto one
// buffer so the GLB writer doesn't complain about multiple buffers.
const single = character.createBuffer();
for (const acc of root.listAccessors()) acc.setBuffer(single);
for (const b of root.listBuffers()) if (b !== single) b.dispose();

await character.transform(pruneAction(), dedupAction());
await io.write(resolve(outPath), character);
console.log(`\nWrote ${outPath}: ${merged} animations merged (${skipped} skipped).`);
