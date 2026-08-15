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

import { Accessor, NodeIO } from '@gltf-transform/core';
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

    // FBX2glTF only writes channels for bones that move; every other bone
    // keeps the animation file's node transform (its pose for that clip).
    // Merge those static bones in as constant channels, otherwise they stay
    // at the character's T-pose (e.g. thumbs flat against the palm)
    // while the rest of the body plays the clip → misaligned pose.
    //
    // Constant channels must end at the clip's real motion length (not a
    // hardcoded 1.0), otherwise the clip duration is pinned to 1.0s and
    // shorter cycles (e.g. the 0.667s run) freeze for the remainder of
    // every loop while longer ones (crouchWalk, 1.333s) get truncated.
    const animated = new Set(
        sourceAnim
            .listChannels()
            .map((ch) => `${ch.getTargetNode().getName()}/${ch.getTargetPath()}`),
    );
    const constEnd = sourceAnim
        .listChannels()
        .map((ch) => ch.getSampler().getInput().getArray())
        .filter((a) => a.length > 2)
        .reduce((max, a) => Math.max(max, a[a.length - 1]), 0);
    const srcBuffer = doc.getRoot().listBuffers()[0] ?? doc.createBuffer();
    const paths = [
        { path: 'rotation', get: (n) => n.getRotation(), ident: [0, 0, 0, 1], type: Accessor.Type.VEC4 },
        { path: 'translation', get: (n) => n.getTranslation(), ident: [0, 0, 0], type: Accessor.Type.VEC3 },
        { path: 'scale', get: (n) => n.getScale(), ident: [1, 1, 1], type: Accessor.Type.VEC3 },
    ];
    let constants = 0;
    const sceneRoots = new Set(
        doc.getRoot().listScenes().map((s) => s.listChildren()),
    );
    for (const node of doc.getRoot().listNodes()) {
        const host = charNodes.get(node.getName());
        if (!host) continue;
        const isSceneRoot = sceneRoots.has(node);
        if (isSceneRoot) continue;
        for (const { path, get, ident, type } of paths) {
            if (animated.has(`${node.getName()}/${path}`)) continue;
            const value = get(node);
            const isIdent = value.every((v, i) => Math.abs(v - ident[i]) < 1e-6);
            if (isIdent) continue;
            const times = doc
                .createAccessor()
                .setType(Accessor.Type.SCALAR)
                .setArray(new Float32Array([0, constEnd || 1]))
                .setBuffer(srcBuffer);
            const out = doc
                .createAccessor()
                .setType(type)
                .setArray(new Float32Array([...value, ...value]))
                .setBuffer(srcBuffer);
            const sampler = doc
                .createAnimationSampler()
                .setInput(times)
                .setOutput(out)
                .setInterpolation('LINEAR');
            sourceAnim.addSampler(sampler).addChannel(
                doc
                    .createAnimationChannel()
                    .setTargetNode(node)
                    .setTargetPath(path)
                    .setSampler(sampler),
            );
            constants++;
        }
    }
    if (constants) console.log(`  ${file}: +${constants} constant channels for static bones`);

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

await character.transform(pruneAction({ keepAttributes: true }), dedupAction());
await io.write(resolve(outPath), character);
console.log(`\nWrote ${outPath}: ${merged} animations merged (${skipped} skipped).`);
