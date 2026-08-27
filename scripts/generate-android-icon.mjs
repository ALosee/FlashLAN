/**
 * One-off generator for the Android adaptive-icon foreground PNGs.
 *
 * Usage: node scripts/generate-android-icon.mjs
 * Source: src-tauri/icons/android-adaptive-foreground.svg
 * Output: ic_launcher_foreground.png in every mipmap-* density folder.
 */
import { readFile, writeFile } from 'node:fs/promises'
import sharp from 'sharp'

const SOURCE = 'src-tauri/icons/android-adaptive-foreground.svg'
const RES = 'src-tauri/gen/android/app/src/main/res'

const DENSITIES = {
  'mipmap-mdpi': 108,
  'mipmap-hdpi': 162,
  'mipmap-xhdpi': 216,
  'mipmap-xxhdpi': 324,
  'mipmap-xxxhdpi': 432,
}

const svg = await readFile(SOURCE, 'utf-8')

for (const [folder, size] of Object.entries(DENSITIES)) {
  const png = await sharp(Buffer.from(svg), { density: 72 }).resize(size, size).png().toBuffer()
  const target = `${RES}/${folder}/ic_launcher_foreground.png`
  await writeFile(target, png)
  console.log(`wrote ${target} (${size}x${size})`)
}
