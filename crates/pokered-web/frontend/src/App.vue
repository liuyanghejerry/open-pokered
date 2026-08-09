<template>
  <div class="container">
    <div class="game-wrapper">
      <div class="loading" id="loading">
        <div class="loading-spinner"></div>
        <p>Loading Pokémon Red...</p>
        <div class="progress-bar-container">
          <div class="progress-bar" id="progress-bar"></div>
        </div>
        <p class="progress-text" id="loading-status">Initializing...</p>
      </div>

      <div class="error-message hidden" id="error">
        <h3>Error Loading Game</h3>
        <p id="error-message"></p>
        <p class="error-hint">Make sure your browser supports WebGPU or WebGL2.</p>
      </div>

      <canvas id="game-canvas"></canvas>
      <div id="fps-counter" class="fps-counter"></div>
    </div>

    <div class="toolbar" id="toolbar" style="display: none;">
      <button class="volume-btn" id="volume-btn" title="Toggle mute">
        <svg id="volume-icon-on" class="volume-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"></polygon>
          <path d="M19.07 4.93a10 10 0 0 1 0 14.14"></path>
          <path d="M15.54 8.46a5 5 0 0 1 0 7.07"></path>
        </svg>
        <svg id="volume-icon-off" class="volume-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="display: none;">
          <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"></polygon>
          <line x1="23" y1="9" x2="17" y2="15"></line>
          <line x1="17" y1="9" x2="23" y2="15"></line>
        </svg>
      </button>
      <input type="range" id="volume-slider" class="volume-slider" min="0" max="100" value="100" title="Volume">
      <span class="volume-label" id="volume-label">100%</span>
    </div>

    <!-- Virtual gamepad (visible on touch devices) -->
    <div class="gamepad" id="gamepad">
      <div class="gamepad-row gamepad-row-main">
        <!-- D-Pad (left side) -->
        <div class="dpad">
          <button class="dpad-btn dpad-up" data-key="ArrowUp" aria-label="Up">&#9650;</button>
          <div class="dpad-middle-row">
            <button class="dpad-btn dpad-left" data-key="ArrowLeft" aria-label="Left">&#9664;</button>
            <div class="dpad-center"></div>
            <button class="dpad-btn dpad-right" data-key="ArrowRight" aria-label="Right">&#9654;</button>
          </div>
          <button class="dpad-btn dpad-down" data-key="ArrowDown" aria-label="Down">&#9660;</button>
        </div>

        <!-- A/B buttons (right side) -->
        <div class="ab-buttons">
          <button class="action-btn btn-b" data-key="KeyX" aria-label="B">B</button>
          <button class="action-btn btn-a" data-key="KeyZ" aria-label="A">A</button>
        </div>
      </div>

      <!-- Start / Select (center bottom) -->
      <div class="gamepad-row gamepad-row-meta">
        <button class="meta-btn" data-key="ShiftRight" aria-label="Select">SELECT</button>
        <button class="meta-btn" data-key="Space" aria-label="Start">START</button>
      </div>
    </div>

    <div class="controls-info" id="controls-info">
      <h3>Controls</h3>
      <div class="controls-grid">
        <div class="control-item">
          <span>D-Pad</span>
          <span class="key">Arrow Keys / WASD</span>
        </div>
        <div class="control-item">
          <span>A Button</span>
          <span class="key">Z / Enter</span>
        </div>
        <div class="control-item">
          <span>B Button</span>
          <span class="key">X / Backspace</span>
        </div>
        <div class="control-item">
          <span>Start</span>
          <span class="key">Space / Return</span>
        </div>
        <div class="control-item">
          <span>Select</span>
          <span class="key">Right Shift</span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted } from 'vue'
import wasmBinaryUrl from '@wasm/pokered_web_bg.wasm?url'

async function fetchWithProgress(
  url: string,
  onProgress: (loaded: number, total: number) => void
): Promise<ArrayBuffer> {
  const response = await fetch(url)

  const isCompressed = !!response.headers.get('Content-Encoding')
  const contentLength = response.headers.get('Content-Length')
  const total = (!isCompressed && contentLength) ? parseInt(contentLength, 10) : 0

  if (!response.body) {
    return response.arrayBuffer()
  }

  const reader = response.body.getReader()
  const chunks: Uint8Array[] = []
  let loaded = 0

  for (;;) {
    const { done, value } = await reader.read()
    if (done) break
    chunks.push(value)
    loaded += value.byteLength
    onProgress(loaded, total)
  }

  const result = new Uint8Array(loaded)
  let offset = 0
  for (const chunk of chunks) {
    result.set(chunk, offset)
    offset += chunk.byteLength
  }
  return result.buffer
}

/**
 * Monkey-patch AudioContext constructor to insert a GainNode between
 * the ScriptProcessorNode and destination. Must run BEFORE importing
 * the wasm-bindgen JS glue (it captures AudioContext at module load).
 * Stores the GainNode on window.__pokered_gain_node for volume control.
 */
function installAudioHook() {
  const OrigAudioCtx = window.AudioContext || (window as any).webkitAudioContext
  if (!OrigAudioCtx) return

  const origConnect = AudioNode.prototype.connect as (
    destination: AudioNode,
    output?: number,
    input?: number
  ) => AudioNode

  const Patched = function (this: any, options?: AudioContextOptions) {
    const ctx: AudioContext = options
      ? new OrigAudioCtx(options)
      : new OrigAudioCtx()

    const gain = ctx.createGain()
    origConnect.call(gain, ctx.destination)

    ;(window as any).__pokered_audio_ctx = ctx
    ;(window as any).__pokered_gain_node = gain

    AudioNode.prototype.connect = function (
      this: AudioNode,
      dest: AudioNode,
      output?: number,
      input?: number
    ): AudioNode {
      if (dest === ctx.destination) {
        return origConnect.call(this, gain, output, input)
      }
      return origConnect.call(this, dest, output, input)
    } as any

    return ctx
  } as any

  Patched.prototype = OrigAudioCtx.prototype
  ;(window as any).AudioContext = Patched
  if ((window as any).webkitAudioContext) {
    ;(window as any).webkitAudioContext = Patched
  }
}

function setupGamepad() {
  const gamepad = document.getElementById('gamepad')
  const controlsInfo = document.getElementById('controls-info')
  if (!gamepad) return

  const isTouchDevice = 'ontouchstart' in window || navigator.maxTouchPoints > 0
  if (isTouchDevice) {
    gamepad.style.display = 'block'
    if (controlsInfo) controlsInfo.style.display = 'none'
  } else {
    gamepad.style.display = 'none'
    if (controlsInfo) controlsInfo.style.display = ''
    return
  }

  function sendKey(code: string, type: 'keydown' | 'keyup') {
    const canvas = document.getElementById('game-canvas')
    if (!canvas) return
    const event = new KeyboardEvent(type, {
      code,
      key: code,
      bubbles: true,
      cancelable: true,
    })
    canvas.dispatchEvent(event)
  }

  // Track active touches per button to handle multi-touch correctly
  const activeTouches = new Map<number, string>()

  gamepad.addEventListener('touchstart', (e: TouchEvent) => {
    e.preventDefault()
    for (let i = 0; i < e.changedTouches.length; i++) {
      const touch = e.changedTouches[i]
      const target = document.elementFromPoint(touch.clientX, touch.clientY) as HTMLElement | null
      const btn = target?.closest('[data-key]') as HTMLElement | null
      if (btn) {
        const code = btn.dataset.key!
        activeTouches.set(touch.identifier, code)
        btn.classList.add('pressed')
        sendKey(code, 'keydown')
      }
    }
  }, { passive: false })

  gamepad.addEventListener('touchmove', (e: TouchEvent) => {
    e.preventDefault()
    for (let i = 0; i < e.changedTouches.length; i++) {
      const touch = e.changedTouches[i]
      const target = document.elementFromPoint(touch.clientX, touch.clientY) as HTMLElement | null
      const btn = target?.closest('[data-key]') as HTMLElement | null
      const newCode = btn?.dataset.key ?? null
      const oldCode = activeTouches.get(touch.identifier) ?? null

      if (oldCode !== newCode) {
        // Finger moved off one button onto another
        if (oldCode) {
          sendKey(oldCode, 'keyup')
          // Remove pressed class from old button
          const oldBtn = gamepad.querySelector(`[data-key="${oldCode}"]`) as HTMLElement | null
          oldBtn?.classList.remove('pressed')
        }
        if (newCode && btn) {
          activeTouches.set(touch.identifier, newCode)
          btn.classList.add('pressed')
          sendKey(newCode, 'keydown')
        } else {
          activeTouches.delete(touch.identifier)
        }
      }
    }
  }, { passive: false })

  function handleTouchEnd(e: TouchEvent) {
    e.preventDefault()
    for (let i = 0; i < e.changedTouches.length; i++) {
      const touch = e.changedTouches[i]
      const code = activeTouches.get(touch.identifier)
      if (code) {
        sendKey(code, 'keyup')
        const btn = gamepad!.querySelector(`[data-key="${code}"]`) as HTMLElement | null
        btn?.classList.remove('pressed')
        activeTouches.delete(touch.identifier)
      }
    }
  }

  gamepad.addEventListener('touchend', handleTouchEnd, { passive: false })
  gamepad.addEventListener('touchcancel', handleTouchEnd, { passive: false })
}

function setupVolumeControls() {
  const toolbar = document.getElementById('toolbar')
  const volumeBtn = document.getElementById('volume-btn')
  const volumeSlider = document.getElementById('volume-slider') as HTMLInputElement | null
  const volumeLabel = document.getElementById('volume-label')
  const iconOn = document.getElementById('volume-icon-on')
  const iconOff = document.getElementById('volume-icon-off')

  const gainNode: GainNode | null = (window as any).__pokered_gain_node ?? null

  if (!toolbar || !volumeBtn || !volumeSlider || !volumeLabel || !iconOn || !iconOff || !gainNode) {
    if (toolbar) toolbar.style.display = 'none'
    return
  }

  toolbar.style.display = ''

  let isMuted = false
  let lastVolume = 1.0

  function updateUI() {
    if (!volumeSlider || !volumeLabel || !iconOn || !iconOff) return
    if (isMuted) {
      iconOn.style.display = 'none'
      iconOff.style.display = ''
      volumeSlider.value = '0'
      volumeLabel.textContent = 'Muted'
    } else {
      iconOn.style.display = ''
      iconOff.style.display = 'none'
      const pct = Math.round(lastVolume * 100)
      volumeSlider.value = String(pct)
      volumeLabel.textContent = `${pct}%`
    }
  }

  volumeBtn.addEventListener('click', () => {
    isMuted = !isMuted
    gainNode.gain.value = isMuted ? 0 : lastVolume
    updateUI()
  })

  volumeSlider.addEventListener('input', () => {
    const val = parseInt(volumeSlider.value, 10) / 100
    lastVolume = val
    isMuted = val === 0
    gainNode.gain.value = val
    updateUI()
  })

  updateUI()
}

onMounted(async () => {
  const loadingStatus = document.getElementById('loading-status')
  const progressBar = document.getElementById('progress-bar')
  const errorMessage = document.getElementById('error-message')
  const loadingDiv = document.getElementById('loading')
  const errorDiv = document.getElementById('error')

  // Set up the on-screen gamepad immediately so it is visible on touch
  // devices during the WASM download and even if loading fails.
  setupGamepad()

  try {
    installAudioHook()

    if (loadingStatus) loadingStatus.textContent = 'Loading module...'
    const wasmModule = await import('@wasm/pokered_web.js')

    if (loadingStatus) loadingStatus.textContent = 'Downloading game data...'

    const wasmBytes = await fetchWithProgress(wasmBinaryUrl, (loaded, total) => {
      if (total > 0) {
        const pct = Math.min(100, Math.round((loaded / total) * 100))
        if (progressBar) progressBar.style.width = `${pct}%`
        if (loadingStatus) {
          const loadedMB = (loaded / 1024 / 1024).toFixed(1)
          const totalMB = (total / 1024 / 1024).toFixed(1)
          loadingStatus.textContent = `Downloading... ${loadedMB}/${totalMB} MB (${pct}%)`
        }
      } else {
        if (loadingStatus) {
          const loadedMB = (loaded / 1024 / 1024).toFixed(1)
          loadingStatus.textContent = `Downloading... ${loadedMB} MB`
        }
      }
    })

    if (loadingStatus) loadingStatus.textContent = 'Compiling...'
    if (progressBar) progressBar.style.width = '100%'

    await wasmModule.default(wasmBytes)

    if (loadingStatus) loadingStatus.textContent = 'Game initialized!'
    setTimeout(() => {
      if (loadingDiv) loadingDiv.classList.add('hidden')
    }, 500)

    setupVolumeControls()
  } catch (e) {
    console.error('Failed to load game:', e)
    if (loadingDiv) loadingDiv.classList.add('hidden')
    if (errorDiv) errorDiv.classList.remove('hidden')
    if (errorMessage) {
      errorMessage.textContent = e instanceof Error ? e.message : 'Unknown error occurred'
    }
  }
})
</script>
