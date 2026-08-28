import { execFile as execFileCallback } from 'node:child_process'
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { promisify } from 'node:util'

import sharp from 'sharp'

const execFile = promisify(execFileCallback)
const root = path.resolve(import.meta.dirname, '..')
const source = path.join(root, 'public/flashlan-icon.svg')
const output = path.join(root, 'src-tauri/icons/icon.icns')
const canvasSize = 1024
const paddingRatio = 0.08
const contentSize = Math.round(canvasSize * (1 - paddingRatio * 2))
const padding = Math.floor((canvasSize - contentSize) / 2)

if (process.platform !== 'darwin') {
  throw new Error('macOS icon generation requires macOS iconutil')
}

const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'flashlan-macos-icon-'))
const iconset = path.join(temporaryRoot, 'FlashLAN.iconset')

try {
  await mkdir(iconset)

  const paddedIcon = await sharp(source, { density: 72 })
    .resize(contentSize, contentSize)
    .extend({
      top: padding,
      bottom: canvasSize - contentSize - padding,
      left: padding,
      right: canvasSize - contentSize - padding,
      background: { r: 0, g: 0, b: 0, alpha: 0 },
    })
    .png()
    .toBuffer()

  const writePng = (fileName, size) =>
    sharp(paddedIcon)
      .resize(size, size)
      .png()
      .toFile(path.join(root, `src-tauri/icons/${fileName}`))

  await writePng('icon.png', 512)
  await writePng('64x64.png', 64)
  await writePng('32x32.png', 32)
  await writePng('128x128.png', 128)
  await writePng('128x128@2x.png', 256)
  const trayIcon = await sharp(source, { density: 72 })
    .resize(32, 32)
    .ensureAlpha()
    .raw()
    .toBuffer()
  await writeFile(path.join(root, 'src-tauri/icons/tray-icon.rgba'), trayIcon)

  for (const size of [16, 32, 128, 256, 512]) {
    const regular = await sharp(paddedIcon).resize(size, size).png().toBuffer()
    const retina = await sharp(paddedIcon)
      .resize(size * 2, size * 2)
      .png()
      .toBuffer()
    await Promise.all([
      writeFile(path.join(iconset, `icon_${size}x${size}.png`), regular),
      writeFile(path.join(iconset, `icon_${size}x${size}@2x.png`), retina),
    ])
  }

  await execFile('iconutil', ['-c', 'icns', iconset, '-o', output])
  console.log(`Generated ${output} with ${paddingRatio * 100}% canvas padding.`)
} finally {
  await rm(temporaryRoot, { recursive: true, force: true })
}
