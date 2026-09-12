import Metal
import MetalKit
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

    private let touchHandler = TouchHandler()

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
        setupGamepad()
        setupFPS()
        setupTexture()
        setupPipeline()
        setupDisplayLink()
    }

    override func viewDidDisappear(_ animated: Bool) {
        super.viewDidDisappear(animated)
        displayLink.invalidate()
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        let safeTop = view.safeAreaInsets.top
        let w = view.bounds.width
        let h = view.bounds.height - safeTop
        let gameH = min(w * 144.0 / 160.0, h * 0.55)
        mtkView.frame = CGRect(x: 0, y: safeTop, width: w, height: gameH)
        if let gp = view.subviews.last(where: { $0 is VirtualGamepad }) {
            gp.frame = CGRect(x: 0, y: safeTop + gameH, width: w, height: h - gameH)
        }
    }

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        touchHandler.touchesBegan(touches, in: view)
    }

    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        touchHandler.touchesMoved(touches, in: view)
    }

    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        touchHandler.touchesEnded(touches, in: view)
    }

    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        touchHandler.touchesCancelled(touches, in: view)
    }

    // MARK: - Setup

    private func setupMTKView() {
        mtkView = MTKView(frame: .zero, device: device)
        mtkView.clearColor       = MTLClearColor(red: 0, green: 0, blue: 0, alpha: 1)
        mtkView.colorPixelFormat = .bgra8Unorm
        mtkView.isPaused              = false
        mtkView.enableSetNeedsDisplay = false
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

    private func setupGamepad() {
        let gamepad = VirtualGamepad(frame: .zero)
        gamepad.backgroundColor = .clear
        gamepad.onButtonStateChange = { [weak self] bits in
            self?.currentInputBits = bits
        }
        view.addSubview(gamepad)
    }

    private var currentInputBits: UInt8 = 0
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
            guard dotzuki_mobile_tick(ctx, currentInputBits) else {
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
        encoder.setViewport(MTLViewport(originX: 0, originY: 0,
                                         width: Double(drawable.texture.width),
                                         height: Double(drawable.texture.height),
                                         znear: 0, zfar: 1))
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
        displayLink.isPaused = true
    }

    func resumeDisplayLink() {
        displayLink.isPaused = false
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

private class VirtualGamepad: UIView {
    var onButtonStateChange: ((UInt8) -> Void)?

    private let btnA = UIButton()
    private let btnB = UIButton()
    private let btnUp = UIButton()
    private let btnDown = UIButton()
    private let btnLeft = UIButton()
    private let btnRight = UIButton()
    private let btnStart = UIButton()
    private let btnSelect = UIButton()
    private let dpadCenter = UIView()

    private var activeBits: UInt8 = 0
    private var touchToButton: [UITouch: UIButton] = [:]

    override init(frame: CGRect) {
        super.init(frame: frame)
        setupButtons()
    }

    required init?(coder: NSCoder) { fatalError() }

    private func setupButtons() {
        let dpadColor    = UIColor(red: 0.165, green: 0.165, blue: 0.243, alpha: 1)
        let dpadBorder   = UIColor(red: 0.227, green: 0.227, blue: 0.369, alpha: 1)
        let abColor      = UIColor(red: 0.800, green: 0.133, blue: 0.133, alpha: 1)
        let abHighlight  = UIColor(red: 1.0, green: 0.267, blue: 0.267, alpha: 1)
        let metaColor    = UIColor(red: 0.200, green: 0.200, blue: 0.314, alpha: 1)
        let metaBorder   = dpadBorder
        let textColor    = UIColor(white: 0.667, alpha: 1)

        func makeBtn(_ btn: UIButton, title: String, bg: UIColor, border: UIColor, cornerRadius: CGFloat? = nil) {
            btn.setTitle(title, for: .normal)
            btn.setTitleColor(.white, for: .normal)
            btn.titleLabel?.font = .monospacedSystemFont(ofSize: 16, weight: .bold)
            btn.backgroundColor = bg
            btn.layer.borderColor = border.cgColor
            btn.layer.borderWidth = 2
            if let r = cornerRadius { btn.layer.cornerRadius = r }
            btn.addTarget(self, action: #selector(buttonTouchDown(_:)), for: .touchDown)
            btn.addTarget(self, action: #selector(buttonTouchUp(_:)), for: [.touchUpInside, .touchUpOutside, .touchCancel])
            addSubview(btn)
        }

        // D-pad buttons
        for (btn, title, corners) in [(btnUp, "▲", CACornerMask([.layerMinXMinYCorner, .layerMaxXMinYCorner])),
                                       (btnDown, "▼", CACornerMask([.layerMinXMaxYCorner, .layerMaxXMaxYCorner])),
                                       (btnLeft, "◀", CACornerMask([.layerMinXMinYCorner, .layerMinXMaxYCorner])),
                                       (btnRight, "▶", CACornerMask([.layerMaxXMinYCorner, .layerMaxXMaxYCorner]))] {
            makeBtn(btn, title: title, bg: dpadColor, border: dpadBorder)
            btn.layer.cornerRadius = 0
            btn.layer.maskedCorners = corners
            btn.layer.cornerRadius = 8
        }
        dpadCenter.backgroundColor = UIColor(red: 0.133, green: 0.133, blue: 0.220, alpha: 1)
        dpadCenter.layer.borderColor = dpadBorder.cgColor
        dpadCenter.layer.borderWidth = 2
        addSubview(dpadCenter)

        // A / B
        makeBtn(btnA, title: "A", bg: abColor, border: abColor, cornerRadius: 31)
        makeBtn(btnB, title: "B", bg: abColor, border: abColor, cornerRadius: 31)
        btnB.titleLabel?.font = .monospacedSystemFont(ofSize: 14, weight: .bold)

        // Start / Select
        for (btn, title) in [(btnSelect, "SELECT"), (btnStart, "START")] {
            makeBtn(btn, title: title, bg: metaColor, border: metaBorder, cornerRadius: 14)
            btn.setTitleColor(textColor, for: .normal)
            btn.titleLabel?.font = .monospacedSystemFont(ofSize: 10, weight: .bold)
        }
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        let w = bounds.width
        let h = bounds.height
        let dpadSize: CGFloat = 48
        let abSize: CGFloat = 56

        let dpadX: CGFloat = 28
        let dpadCY = h / 2
        btnUp.frame    = CGRect(x: dpadX + dpadSize, y: dpadCY - dpadSize * 1.5, width: dpadSize, height: dpadSize)
        btnLeft.frame  = CGRect(x: dpadX, y: dpadCY - dpadSize * 0.5, width: dpadSize, height: dpadSize)
        dpadCenter.frame = CGRect(x: dpadX + dpadSize, y: dpadCY - dpadSize * 0.5, width: dpadSize, height: dpadSize)
        btnRight.frame = CGRect(x: dpadX + dpadSize * 2, y: dpadCY - dpadSize * 0.5, width: dpadSize, height: dpadSize)
        btnDown.frame  = CGRect(x: dpadX + dpadSize, y: dpadCY + dpadSize * 0.5, width: dpadSize, height: dpadSize)

        let abX = w - 28 - abSize * 2 - 12
        let abCY = h / 2
        btnB.frame = CGRect(x: abX, y: abCY - abSize / 2, width: abSize, height: abSize)
        btnA.frame = CGRect(x: abX + abSize + 12, y: abCY - abSize / 2 - 10, width: abSize, height: abSize)

        let metaW: CGFloat = 80
        let metaH: CGFloat = 32
        let metaY = h - metaH - 16
        btnSelect.frame = CGRect(x: w / 2 - metaW - 12, y: metaY, width: metaW, height: metaH)
        btnStart.frame  = CGRect(x: w / 2 + 12, y: metaY, width: metaW, height: metaH)
    }

    @objc private func buttonTouchDown(_ sender: UIButton) {
        let bit = buttonBit(sender)
        activeBits |= bit
        onButtonStateChange?(activeBits)
        if sender == btnA || sender == btnB {
            sender.backgroundColor = UIColor(red: 1, green: 0.267, blue: 0.267, alpha: 1)
        } else {
            sender.backgroundColor = UIColor(red: 1, green: 0.267, blue: 0.267, alpha: 1)
        }
    }

    @objc private func buttonTouchUp(_ sender: UIButton) {
        let bit = buttonBit(sender)
        activeBits &= ~bit
        onButtonStateChange?(activeBits)
        if sender == btnA || sender == btnB {
            sender.backgroundColor = UIColor(red: 0.8, green: 0.133, blue: 0.133, alpha: 1)
        } else if sender == btnStart || sender == btnSelect {
            sender.backgroundColor = UIColor(red: 0.2, green: 0.2, blue: 0.314, alpha: 1)
        } else {
            sender.backgroundColor = UIColor(red: 0.165, green: 0.165, blue: 0.243, alpha: 1)
        }
    }

    private func buttonBit(_ btn: UIButton) -> UInt8 {
        switch btn {
        case btnA:      return 1 << 0
        case btnB:      return 1 << 1
        case btnSelect: return 1 << 2
        case btnStart:  return 1 << 3
        case btnRight:  return 1 << 4
        case btnLeft:   return 1 << 5
        case btnUp:     return 1 << 6
        case btnDown:   return 1 << 7
        default:        return 0
        }
    }
}

// MARK: - MTLTexture Size Helper

private extension MTLTexture {
    var size: MTLSize { MTLSize(width: width, height: height, depth: depth) }
}
