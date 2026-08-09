import UIKit

// MARK: - FFI Bridge (save/load/lifecycle)

@_silgen_name("pokered_save")
func pokered_save(_ ctx: UnsafeMutableRawPointer, _ path: UnsafePointer<CChar>?) -> Bool

@_silgen_name("pokered_clear_cache")
func pokered_clear_cache(_ ctx: UnsafeMutableRawPointer)

@_silgen_name("pokered_destroy")
func pokered_destroy(_ ctx: UnsafeMutableRawPointer)

// MARK: - AppDelegate

@main
final class AppDelegate: UIResponder, UIApplicationDelegate {

    // MARK: - Properties

    var window: UIWindow?
    private var gameVC: GameViewController!

    // MARK: - UIApplicationDelegate

    func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
    ) -> Bool {
        // ── Audio session ──
        AudioEngine.configureAudioSession()

        // ── Window + root controller ──
        let vc = GameViewController()
        gameVC = vc

        let window = UIWindow(frame: UIScreen.main.bounds)
        window.rootViewController = vc
        window.makeKeyAndVisible()
        self.window = window

        return true
    }

    func applicationDidBecomeActive(_ application: UIApplication) {
        gameVC.resumeDisplayLink()
        do {
            try gameVC.audioEngine?.start()
        } catch {
            print("AudioEngine start failed in didBecomeActive: \(error)")
        }
    }

    func applicationWillResignActive(_ application: UIApplication) {
        // Auto-save before resigning. The Rust side skips saving during
        // battle (mid-turn state is not serializable), and the atomic
        // rename in Rust protects against SIGKILL during write.
        if let ctx = gameVC.ctxPtr {
            _ = pokered_save(UnsafeMutableRawPointer(ctx), nil)
        }
        gameVC.pauseDisplayLink()
        gameVC.audioEngine?.stop()
    }

    func applicationDidEnterBackground(_ application: UIApplication) {
        gameVC.releaseDrawables()
    }

    func applicationWillEnterForeground(_ application: UIApplication) {
        // didBecomeActive will start audio and resume display link.
    }

    func applicationDidReceiveMemoryWarning(_ application: UIApplication) {
        if let ctx = gameVC.ctxPtr {
            pokered_clear_cache(UnsafeMutableRawPointer(ctx))
        }
    }

    func applicationWillTerminate(_ application: UIApplication) {
        if let ctx = gameVC.ctxPtr {
            _ = pokered_save(UnsafeMutableRawPointer(ctx), nil)
            pokered_destroy(UnsafeMutableRawPointer(ctx))
            gameVC.ctxPtr = nil
        }
    }
}
