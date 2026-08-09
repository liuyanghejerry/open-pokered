import AVFoundation

// MARK: - Rust FFI Import

/// Bridge to `pokered_audio_fill` in `crates/pokered-ios/src/lib.rs`.
///
/// Pulls interleaved stereo `f32` samples from the lock-free ring buffer.
/// - Parameter ctx: Raw pointer to the Rust `GameContext`.
/// - Parameter buffer: Destination for interleaved stereo samples (L,R,L,R,…).
/// - Parameter frames: Requested number of stereo frame pairs.
/// - Returns: Number of frames actually written (may be less than `frames`).
@_silgen_name("pokered_audio_fill")
func pokered_audio_fill(
    _ ctx: UnsafeMutableRawPointer,
    _ buffer: UnsafeMutablePointer<Float>,
    _ frames: UInt32
) -> UInt32

// MARK: - AudioEngine

/// Real-time audio playback engine for the Pokémon Red/Blue iOS app.
///
/// Wraps the system `AVAudioEngine` with an `AVAudioSourceNode` whose
/// render callback runs on a high-priority real-time thread and pulls
/// samples from the Rust lock-free ring buffer via C FFI.
///
/// ## Thread Safety
///
/// The render callback is invoked by Core Audio on a **real-time I/O thread**.
/// It must never allocate heap memory, take locks, call blocking APIs, or
/// invoke any Objective-C / Swift runtime that may `retain`/`release`.
/// All buffers are pre-allocated in `init()`.
final class AudioEngine {

    // MARK: - Audio Format

    /// Standard game-audio format: 48 kHz, 2-channel stereo, float32 PCM,
    /// non-interleaved (two `AudioBuffer` entries in the output `AudioBufferList`).
    private let stereoFormat = AVAudioFormat(
        commonFormat: .pcmFormatFloat32,
        sampleRate: 48000,
        channels: 2,
        interleaved: false
    )!

    // MARK: - Hardware

    private let engine = AVAudioEngine()
    private var sourceNode: AVAudioSourceNode?

    /// Raw pointer to the Rust `GameContext` — the single owner of all game state.
    /// Set by the caller (e.g. `GameViewController`) after `pokered_init()`.
    var ctxPtr: UnsafeMutableRawPointer?

    // MARK: - Pre-allocated Scratch Buffer

    /// Scratch buffer for interleaved stereo samples pulled from Rust.
    ///
    /// **Allocated once in `init()`** and reused across every render callback
    /// so that the real-time path performs zero heap allocations.
    ///
    /// Capacity = `maxRenderFrames * 2` floats (L,R per frame).
    private let scratchBuffer: UnsafeMutablePointer<Float>
    private let scratchCapacity: Int
    private static let maxRenderFrames = 4096

    // MARK: - Lifecycle

    init() {
        scratchCapacity = Self.maxRenderFrames * 2
        scratchBuffer = UnsafeMutablePointer<Float>.allocate(capacity: scratchCapacity)
        scratchBuffer.initialize(repeating: 0, count: scratchCapacity)
    }

    deinit {
        scratchBuffer.deinitialize(count: scratchCapacity)
        scratchBuffer.deallocate()
    }

    // MARK: - Audio Session

    /// Configures the shared `AVAudioSession` for gameplay.
    ///
    /// Uses the `.playback` category so audio ignores the hardware silent switch,
    /// and activates the session immediately. Call this once at app launch.
    static func configureAudioSession() {
        let session = AVAudioSession.sharedInstance()
        do {
            try session.setCategory(.playback, mode: .default)
            try session.setActive(true)
        } catch {
            print("AudioSession configuration failed: \(error)")
        }
    }

    // MARK: - Source Node Setup

    /// Creates the `AVAudioSourceNode` and wires it into the engine graph.
    ///
    /// The render block captures **raw pointers only** — no `self`, no ARC
    /// objects — so the real-time thread never triggers retain / release.
    ///
    /// Call this after `ctxPtr` has been set and before `start()`.
    func setupSourceNode() {
        // Captured once; the Rust GameContext is a stable singleton.
        let capturedCtx = ctxPtr
        let scratch = scratchBuffer
        let maxFrames = Self.maxRenderFrames

        let node = AVAudioSourceNode(format: stereoFormat) { (
            isSilence: UnsafeMutablePointer<ObjCBool>,
            _ timestamp: UnsafePointer<AudioTimeStamp>,
            frameCount: AVAudioFrameCount,
            outputData: UnsafeMutablePointer<AudioBufferList>
        ) -> OSStatus in

            guard let ctx = capturedCtx else {
                isSilence.pointee = true
                return noErr
            }

            let fc = Int(frameCount)
            let requested = UInt32(min(fc, maxFrames))

            // === Pull interleaved stereo from the Rust ring buffer ===
            let filled = pokered_audio_fill(ctx, scratch, requested)

            // === Access the two non-interleaved output channel buffers ===
            let outputBuffers = UnsafeMutableAudioBufferListPointer(outputData)
            guard outputBuffers.count >= 2,
                  let leftPtr = outputBuffers[0].mData?.assumingMemoryBound(to: Float.self),
                  let rightPtr = outputBuffers[1].mData?.assumingMemoryBound(to: Float.self)
            else {
                isSilence.pointee = true
                return noErr
            }

            if filled == 0 {
                // Ring buffer empty → output silence.
                isSilence.pointee = true
            } else {
                isSilence.pointee = false

                let filledInt = Int(filled)

                // De-interleave scratch (L₀,R₀,L₁,R₁,…) → separate L / R arrays.
                var si = scratch
                for i in 0..<filledInt {
                    leftPtr[i] = si.pointee;   si = si.successor()
                    rightPtr[i] = si.pointee;  si = si.successor()
                }

                // Zero-pad any remaining frames (partial underrun).
                if filledInt < fc {
                    let remaining = fc - filledInt
                    (leftPtr + filledInt).initialize(repeating: 0, count: remaining)
                    (rightPtr + filledInt).initialize(repeating: 0, count: remaining)
                }
            }

            return noErr
        }

        self.sourceNode = node
        engine.attach(node)
        engine.connect(node, to: engine.mainMixerNode, format: stereoFormat)
    }

    // MARK: - Start / Stop

    /// Starts the audio engine.
    ///
    /// `setupSourceNode()` must be called first, and `AVAUDiOSSession`
    /// must be configured. Throws if the engine cannot start.
    func start() throws {
        do {
            try engine.start()
        } catch {
            print("AudioEngine start failed: \(error)")
            throw error
        }
    }

    /// Stops the audio engine immediately.
    func stop() {
        engine.stop()
    }

    // MARK: - Interruption Handling

    /// Registers an observer for `AVAudioSession.interruptionNotification`.
    ///
    /// - `.began` — pauses the engine (phone call, Siri, alarm).
    /// - `.ended` — reactivates the session and restarts the engine when
    ///   the system's `shouldResume` flag is set.
    func observeInterruptions() {
        NotificationCenter.default.addObserver(
            forName: AVAudioSession.interruptionNotification,
            object: nil,
            queue: .main
        ) { [weak self] notification in
            guard let self = self,
                  let userInfo = notification.userInfo,
                  let rawType = userInfo[AVAudioSessionInterruptionTypeKey] as? UInt,
                  let type = AVAudioSession.InterruptionType(rawValue: rawType)
            else {
                return
            }

            switch type {
            case .began:
                self.stop()

            case .ended:
                guard let rawOptions =
                    userInfo[AVAudioSessionInterruptionOptionKey] as? UInt
                else {
                    return
                }
                let options = AVAudioSession.InterruptionOptions(rawValue: rawOptions)

                // Reactivate the shared audio session.
                do {
                    try AVAudioSession.sharedInstance().setActive(true)
                } catch {
                    print("AudioSession reactivation failed: \(error)")
                    return
                }

                if options.contains(.shouldResume) {
                    do {
                        try self.engine.start()
                    } catch {
                        print("AudioEngine restart after interruption failed: \(error)")
                    }
                }

            @unknown default:
                break
            }
        }
    }
}
