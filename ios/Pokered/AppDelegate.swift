import UIKit

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
        // Export the most recent committed shared-mobile save before resigning.
        _ = gameVC.persistSave()
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
        // The shared runtime has no host-side cache to clear.
    }

    func applicationWillTerminate(_ application: UIApplication) {
        gameVC.destroyRunner()
    }
}
