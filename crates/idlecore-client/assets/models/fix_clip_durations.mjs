#!/usr/bin/env node
/**
 * Fix per-clip durations in characterLargeMale.glb.
 *
 * merge_animations.mjs keys static-bone constant channels at [0, 1], which
 * pins every clip's duration to 1.0s: the run (0.667s cycle) then freezes for
 * the remaining third of every loop, and crouchWalk (1.333s) gets truncated.
 *
 * This rewrites each constant channel's second key from 1.0 to the clip's real
 * motion length (the max key time of its non-constant channels), so every
 * clip loops at its true cycle length.
 *
 * Note: the merge pipeline's `dedup` shares one input accessor across all
 * constant channels, so each fixed sampler gets a freshly created accessor.
 */
import { Accessor, NodeIO } from '@gltf-transform/core';

const io = new NodeIO();
const doc = await io.read('characterLargeMale.glb');
const root = doc.getRoot();

let fixed = 0;
for (const anim of root.listAnimations()) {
    let motionEnd = 0;
    for (const ch of anim.listChannels()) {
        const input = ch.getSampler().getInput().getArray();
        if (input.length > 2) {
            motionEnd = Math.max(motionEnd, input[input.length - 1]);
        }
    }
    if (motionEnd <= 0) continue;
    for (const ch of anim.listChannels()) {
        const sampler = ch.getSampler();
        const input = sampler.getInput().getArray();
        const output = sampler.getOutput().getArray();
        if (input.length !== 2) continue;
        const comps = sampler.getOutput().getType().components;
        let constant = true;
        for (let i = 0; i < comps; i++) {
            if (output[i] !== output[comps + i]) { constant = false; break; }
        }
        if (!constant) continue;
        const srcBuffer = sampler.getInput().getBuffer();
        const acc = doc
            .createAccessor()
            .setType(Accessor.Type.SCALAR)
            .setArray(new Float32Array([0, motionEnd]))
            .setBuffer(srcBuffer);
        sampler.setInput(acc);
        fixed++;
    }
}

await io.write('characterLargeMale.glb', doc);
console.log(`Fixed ${fixed} constant channels across ${root.listAnimations().length} clips`);