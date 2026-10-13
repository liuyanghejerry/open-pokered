import Metal
import MetalKit
import DotzukiTouch
import UIKit

// MARK: - FFI Bridge

/// Opaque runner owned by dotzuki-mobile's shared C ABI.
struct DotzukiMobileRunner {}

@_silgen_name("dotzuki_mobile_abi_version")
func dotzuki_mobile_abi_version() -> UInt32

@_silgen_name("dotzuki_mobile_create")
func dotzuki_mobile_create(
    _ pack: UnsafePointer<UInt8>, _ packLen: Int,
    _ save: UnsafePointer<UInt8>?, _ saveLen: Int
) -> UnsafeMutablePointer<DotzukiMobileRunner>?

@_silgen_name("dotzuki_mobile_destroy")
func dotzuki_mobile_destroy(_ runner: UnsafeMutablePointer<DotzukiMobileRunner>)

@_silgen_name("dotzuki_mobile_width")
func dotzuki_mobile_width(_ runner: UnsafePointer<DotzukiMobileRunner>) -> UInt32

@_silgen_name("dotzuki_mobile_height")
func dotzuki_mobile_height(_ runner: UnsafePointer<DotzukiMobileRunner>) -> UInt32

@_silgen_name("dotzuki_mobile_frame_len")
func dotzuki_mobile_frame_len(_ runner: UnsafePointer<DotzukiMobileRunner>) -> Int

@_silgen_name("dotzuki_mobile_tick")
func dotzuki_mobile_tick(_ runner: UnsafeMutablePointer<DotzukiMobileRunner>, _ inputBits: UInt8) -> Bool

@_silgen_name("dotzuki_mobile_copy_frame")
func dotzuki_mobile_copy_frame(
    _ runner: UnsafePointer<DotzukiMobileRunner>, _ buffer: UnsafeMutablePointer<UInt8>, _ len: Int
) -> Int

@_silgen_name("dotzuki_mobile_export_save")
func dotzuki_mobile_export_save(
    _ runner: UnsafePointer<DotzukiMobileRunner>, _ buffer: UnsafeMutablePointer<UInt8>?, _ len: Int
) -> Int

// MARK: - Constants

private let SCREEN_WIDTH: Int    = 160
private let SCREEN_HEIGHT: Int   = 144
private let BYTES_PER_PIXEL: Int = 4
private let FRAME_BUFFER_SIZE: Int = SCREEN_WIDTH * SCREEN_HEIGHT * BYTES_PER_PIXEL
private let BYTES_PER_ROW: Int   = SCREEN_WIDTH * BYTES_PER_PIXEL
private let MOBILE_ABI_VERSION: UInt32 = 1
private let SAVE_KEY = "pokered-mobile-save-v1"
private let INITIALIZATION_PAYLOAD = Array("pokered:red:v1".utf8)

// MARK: - Inline MSL Source

private let metalShaderSource = """
#include <metal_stdlib>
using namespace metal;

struct VertexOut {
    float4 position [[position]];
    float2 texCoord;
};

vertex VertexOut vertex_main(uint vid [[vertex_id]]) {
    const float2 positions[4] = {
        float2(-1.0,  1.0),
        float2( 1.0,  1.0),
        float2(-1.0, -1.0),
        float2( 1.0, -1.0)
    };
    const float2 texCoords[4] = {
        float2(0.0, 0.0),
        float2(1.0, 0.0),
        float2(0.0, 1.0),
        float2(1.0, 1.0)
    };
    VertexOut out;
    out.position = float4(positions[vid], 0.0, 1.0);
    out.texCoord = texCoords[vid];
    return out;
}

fragment float4 fragment_main(
    VertexOut in         [[stage_in]],
    texture2d<float> tex [[texture(0)]]
) {
    constexpr sampler s(filter::nearest, address::clamp_to_edge);
    return tex.sample(s, in.texCoord);
}
"""

// MARK: - GameViewController

final class GameViewController: UIViewController {

    // MARK: - Properties

    private var mtkView: MTKView!
    private var device: MTLDevice!
    private var commandQueue: MTLCommandQueue!
    private var pipelineState: MTLRenderPipelineState!
    private var gameTexture: MTLTexture!
    private var displayLink: CADisplayLink!
    private var frameBuffer: [UInt8] = Array(repeating: 0, count: FRAME_BUFFER_SIZE)

    var ctxPtr: UnsafeMutablePointer<DotzukiMobileRunner>? = nil
    private var lastCommittedSave = ""

    private var touchOverlay: DTZTouchOverlay!
    private var touchInputBits: UInt8 = 0
    private var applicationIsActive = true
    private var touchSettingsVisible = false

    /// Loading screen state — 3-frame deferred init to avoid SpringBoard watchdog.
    private var isLoading = true
    private var loadingTick: UInt32 = 0

    /// Audio engine for real-time playback. Owned by this controller so the
    /// game loop can push samples to the ring buffer; AppDelegate coordinates
    /// start/stop across lifecycle transitions.
    private(set) var audioEngine: AudioEngine?

    // MARK: - Lifecycle

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = UIColor(red: 0.102, green: 0.102, blue: 0.180, alpha: 1)

        guard let device = MTLCreateSystemDefaultDevice() else {
            fatalError("Metal is not supported on this device")
        }
        self.device = device

        guard let queue = device.makeCommandQueue() else {
            fatalError("Failed to create MTLCommandQueue")
        }
        commandQueue = queue

        setupMTKView()
        setupTouchOverlay()
        setupFPS()
        setupTexture()
        setupPipeline()
        setupDisplayLink()
    }

    override func viewDidDisappear(_ animated: Bool) {
        super.viewDidDisappear(animated)
        touchOverlay?.releaseAll()
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        mtkView.frame = view.bounds
        fpsLabel.frame.origin = CGPoint(x: 6, y: view.safeAreaInsets.top + 6)
    }

    // MARK: - Setup

    private func setupMTKView() {
        mtkView = MTKView(frame: .zero, device: device)
        mtkView.clearColor       = MTLClearColor(red: 0.102, green: 0.102, blue: 0.180, alpha: 1)
        mtkView.colorPixelFormat = .bgra8Unorm
        mtkView.isPaused              = false
        mtkView.enableSetNeedsDisplay = false
        mtkView.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        view.addSubview(mtkView)
    }

    private func setupTexture() {
        let desc = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: .rgba8Unorm,
            width:       SCREEN_WIDTH,
            height:      SCREEN_HEIGHT,
            mipmapped:   false
        )
        desc.usage       = [.shaderRead]
        desc.storageMode = .shared
        guard let tex = device.makeTexture(descriptor: desc) else {
            fatalError("Failed to create 160x144 game texture")
        }
        gameTexture = tex
    }

    private func setupPipeline() {
        guard let library = try? device.makeLibrary(source: metalShaderSource, options: nil) else {
            fatalError("Failed to compile inline Metal shaders")
        }
        guard let vertFn = library.makeFunction(name: "vertex_main"),
              let fragFn = library.makeFunction(name: "fragment_main") else {
            fatalError("Metal shader functions not found")
        }
        let desc = MTLRenderPipelineDescriptor()
        desc.vertexFunction                  = vertFn
        desc.fragmentFunction                = fragFn
        desc.colorAttachments[0].pixelFormat = mtkView.colorPixelFormat
        guard let pipeline = try? device.makeRenderPipelineState(descriptor: desc) else {
            fatalError("Failed to create Metal render pipeline")
        }
        pipelineState = pipeline
    }

    private func setupDisplayLink() {
        displayLink = CADisplayLink(target: self, selector: #selector(gameLoop))
        displayLink.add(to: .main, forMode: .common)
    }

    private func setupFPS() {
        fpsLabel = UILabel()
        fpsLabel.font = .monospacedSystemFont(ofSize: 11, weight: .regular)
        fpsLabel.textColor = UIColor(white: 0.75, alpha: 1)
        fpsLabel.text = "FPS: --"
        fpsLabel.sizeToFit()
        fpsLabel.frame.origin = CGPoint(x: 6, y: view.safeAreaInsets.top + 6)
        view.addSubview(fpsLabel)
    }

    private func setupTouchOverlay() {
        let overlay = DTZTouchOverlay(frame: .zero)
        overlay.translatesAutoresizingMaskIntoConstraints = false
        overlay.onInputChanged = { [weak self] bits in
            self?.touchInputBits = bits
        }
        overlay.onSettingsVisibilityChanged = { [weak self] visible in
            guard let self else { return }
            self.touchSettingsVisible = visible
            self.updateDisplayLinkState()
        }
        view.addSubview(overlay)
        NSLayoutConstraint.activate([
            overlay.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            overlay.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            overlay.topAnchor.constraint(equalTo: view.topAnchor),
            overlay.bottomAnchor.constraint(equalTo: view.bottomAnchor),
        ])
        touchOverlay = overlay
    }

    private var frameCount: UInt64 = 0
    private var droppedFrames: UInt64 = 0
    private var fpsStartTime: CFTimeInterval = CACurrentMediaTime()
    private var fpsLabel: UILabel!

    // MARK: - Game Loop

    @objc private func gameLoop() {
        if isLoading {
            if loadingTick < 2 {
                drawLoadingScreen(tick: loadingTick)
            } else if loadingTick == 2 {
                createMobileRunner()
                isLoading = false
            }
            loadingTick += 1
        }

        if let ctx = ctxPtr {
            let inputBits = touchOverlay.consumeInput() | touchInputBits
            guard dotzuki_mobile_tick(ctx, inputBits) else {
                destroyRunner()
                return
            }
            let written = dotzuki_mobile_copy_frame(ctx, &frameBuffer, FRAME_BUFFER_SIZE)
            guard written == FRAME_BUFFER_SIZE else {
                destroyRunner()
                return
            }
        }

        frameCount += 1
        let elapsed = CACurrentMediaTime() - fpsStartTime
        if elapsed >= 1.0 {
            let fps = Double(frameCount) / elapsed
            DispatchQueue.main.async { self.fpsLabel.text = String(format: "FPS: %.0f", fps) }
            frameCount = 0
            fpsStartTime = CACurrentMediaTime()
        }

        gameTexture.replace(
            region:      MTLRegionMake2D(0, 0, SCREEN_WIDTH, SCREEN_HEIGHT),
            mipmapLevel: 0,
            withBytes:   frameBuffer,
            bytesPerRow: BYTES_PER_ROW
        )

        renderFrame()
    }

    // MARK: - Rendering

    private func renderFrame() {
        guard let drawable       = mtkView.currentDrawable,
              let renderPassDesc = mtkView.currentRenderPassDescriptor,
              let cmdBuf         = commandQueue.makeCommandBuffer(),
              let encoder        = cmdBuf.makeRenderCommandEncoder(descriptor: renderPassDesc)
        else { droppedFrames += 1; return }

        encoder.setRenderPipelineState(pipelineState)
        encoder.setViewport(letterboxViewport(drawableSize: drawable.texture.size))
        encoder.setFragmentTexture(gameTexture, index: 0)
        encoder.drawPrimitives(type: .triangleStrip, vertexStart: 0, vertexCount: 4)
        encoder.endEncoding()
        cmdBuf.present(drawable)
        cmdBuf.commit()
    }

    // MARK: - Loading Screen

    private func drawLoadingScreen(tick: UInt32) {
        let darkAccent: [UInt8] = [40, 40, 50, 255]
        let accent:     [UInt8] = [255, 80, 80, 255]
        let barW = 80
        let barX = (SCREEN_WIDTH - barW) / 2
        let barY = SCREEN_HEIGHT / 2 + 20
        let barH = 4

        var i = 0
        while i < FRAME_BUFFER_SIZE {
            frameBuffer[i]     = darkAccent[0]
            frameBuffer[i + 1] = darkAccent[1]
            frameBuffer[i + 2] = darkAccent[2]
            frameBuffer[i + 3] = darkAccent[3]
            i += 4
        }

        let progress = min(Int(tick) * (80 / 3), 80)
        guard progress > 0 else { return }

        let pulse = sin(Float(tick) * 0.1) * 0.3 + 0.7
        let pulseColor: [UInt8] = [
            UInt8(Float(accent[0]) * pulse),
            UInt8(Float(accent[1]) * pulse),
            UInt8(Float(accent[2]) * pulse),
            255
        ]

        for py in barY..<(barY + barH) {
            for px in barX..<(barX + progress) {
                let idx = (py * SCREEN_WIDTH + px) * 4
                frameBuffer[idx]     = pulseColor[0]
                frameBuffer[idx + 1] = pulseColor[1]
                frameBuffer[idx + 2] = pulseColor[2]
                frameBuffer[idx + 3] = pulseColor[3]
            }
        }
    }

    // MARK: - Letterbox

    private func letterboxViewport(drawableSize size: MTLSize) -> MTLViewport {
        let targetAspect = Float(SCREEN_WIDTH) / Float(SCREEN_HEIGHT)
        let screenAspect = Float(size.width)   / Float(size.height)

        let vpW: Float
        let vpH: Float
        if screenAspect > targetAspect {
            vpH = Float(size.height)
            vpW = vpH * targetAspect
        } else {
            vpW = Float(size.width)
            vpH = vpW / targetAspect
        }

        let vpX = (Float(size.width)  - vpW) * 0.5
        let vpY = (Float(size.height) - vpH) * 0.5
        return MTLViewport(
            originX: Double(vpX), originY: Double(vpY),
            width:   Double(vpW), height:  Double(vpH),
            znear:   0.0,         zfar:    1.0
        )
    }

    // MARK: - Lifecycle Hooks (called by AppDelegate)

    /// Creates the `AudioEngine`, wires its `ctxPtr`, sets up the source
    /// node, and registers interruption observers. Must be called after
    /// the shared mobile runner is created.
    func setupAudioEngine() {
        guard let ctx = ctxPtr else { return }
        let audio = AudioEngine()
        audio.ctxPtr = UnsafeMutableRawPointer(ctx)
        audio.setupSourceNode()
        audio.observeInterruptions()
        self.audioEngine = audio
    }

    func pauseDisplayLink() {
        applicationIsActive = false
        touchOverlay?.releaseAll()
        updateDisplayLinkState()
    }

    func resumeDisplayLink() {
        applicationIsActive = true
        updateDisplayLinkState()
    }

    private func updateDisplayLinkState() {
        displayLink?.isPaused = !applicationIsActive || touchSettingsVisible
    }

    func releaseDrawables() {
        // On iOS 17+, Metal drawables are released automatically when the
        // app enters background; explicit release is a no-op for safety.
    }

    /// Persist only committed JSON saves from the shared mobile runtime.
    /// A zero-length export means the game has no committed save yet.
    @discardableResult
    func persistSave() -> Bool {
        guard let ctx = ctxPtr else { return false }
        let length = dotzuki_mobile_export_save(ctx, nil, 0)
        guard length > 0 else { return false }
        var bytes = [UInt8](repeating: 0, count: length)
        let written = bytes.withUnsafeMutableBufferPointer {
            dotzuki_mobile_export_save(ctx, $0.baseAddress, $0.count)
        }
        guard written == length, let save = String(bytes: bytes, encoding: .utf8) else {
            return false
        }
        if save != lastCommittedSave {
            UserDefaults.standard.set(save, forKey: SAVE_KEY)
            lastCommittedSave = save
        }
        return true
    }

    /// Stop Core Audio before releasing the FFI runner, as required by ABI v1.
    func destroyRunner() {
        touchOverlay?.releaseAll()
        audioEngine?.stop()
        audioEngine = nil
        guard let ctx = ctxPtr else { return }
        _ = persistSave()
        dotzuki_mobile_destroy(ctx)
        ctxPtr = nil
    }

    private func createMobileRunner() {
        guard dotzuki_mobile_abi_version() == MOBILE_ABI_VERSION else {
            fatalError("Unsupported dotzuki mobile ABI")
        }
        let saved = UserDefaults.standard.string(forKey: SAVE_KEY)
        let runner: UnsafeMutablePointer<DotzukiMobileRunner>? = INITIALIZATION_PAYLOAD.withUnsafeBufferPointer { pack in
            guard let packPtr = pack.baseAddress else { return nil }
            guard let saved else {
                return dotzuki_mobile_create(packPtr, pack.count, nil, 0)
            }
            let bytes = Array(saved.utf8)
            return bytes.withUnsafeBufferPointer {
                dotzuki_mobile_create(packPtr, pack.count, $0.baseAddress, $0.count)
            }
        }
        guard let runner else {
            fatalError("Unable to create dotzuki mobile runner")
        }
        guard dotzuki_mobile_width(runner) == UInt32(SCREEN_WIDTH),
              dotzuki_mobile_height(runner) == UInt32(SCREEN_HEIGHT),
              dotzuki_mobile_frame_len(runner) == FRAME_BUFFER_SIZE
        else {
            dotzuki_mobile_destroy(runner)
            fatalError("Unexpected dotzuki mobile framebuffer contract")
        }
        ctxPtr = runner
        lastCommittedSave = saved ?? ""
        setupAudioEngine()
        do {
            try audioEngine?.start()
        } catch {
            print("AudioEngine start failed after runner creation: \(error)")
        }
    }
}

// MARK: - MTLTexture Size Helper

private extension MTLTexture {
    var size: MTLSize { MTLSize(width: width, height: height, depth: depth) }
}
