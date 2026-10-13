import XCTest
import UIKit
import DotzukiTouch

@MainActor
final class TouchOverlayTests: XCTestCase {
    func testLayoutAndHostInputContract() {
        let defaults = UserDefaults.standard
        let saved = defaults.dictionary(forKey: "dotzuki-touch-v1")
        defaults.removeObject(forKey: "dotzuki-touch-v1")
        defer { defaults.set(saved, forKey: "dotzuki-touch-v1") }
        let overlay = DTZTouchOverlay(frame: CGRect(x: 0, y: 0, width: 800, height: 400))
        overlay.layoutIfNeeded()
        XCTAssertTrue(overlay.point(inside: CGPoint(x: 92, y: 240), with: nil))
        XCTAssertFalse(overlay.point(inside: CGPoint(x: 400, y: 200), with: nil))
        var released = false
        overlay.onInputChanged = { bits in released = bits == 0 }
        overlay.controlsEnabled = false
        XCTAssertTrue(released)
        XCTAssertEqual(overlay.consumeInput(), 0)
        XCTAssertFalse(overlay.isUserInteractionEnabled)
        overlay.controlsEnabled = true
        released = false
        overlay.frame.size.width = 700
        overlay.layoutIfNeeded()
        XCTAssertTrue(released)
        let image = UIGraphicsImageRenderer(size: overlay.bounds.size).image { context in
            overlay.layer.render(in: context.cgContext)
        }
        let attachment = XCTAttachment(image: image)
        attachment.name = "Landscape touch overlay"
        attachment.lifetime = .keepAlways
        add(attachment)
        overlay.frame.size = CGSize(width: 200, height: 100)
        overlay.layoutIfNeeded()
        XCTAssertFalse(overlay.point(inside: CGPoint(x: 92, y: 50), with: nil))
    }

    func testMirroredLayoutAndSettingsPersistence() {
        let defaults = UserDefaults.standard
        let saved = defaults.dictionary(forKey: "dotzuki-touch-v1")
        defer { defaults.set(saved, forKey: "dotzuki-touch-v1") }
        defaults.set(["mirrored": true, "scale": 1.0], forKey: "dotzuki-touch-v1")
        let overlay = DTZTouchOverlay(frame: CGRect(x: 0, y: 0, width: 800, height: 400))
        overlay.layoutIfNeeded()
        XCTAssertTrue(overlay.point(inside: CGPoint(x: 708, y: 240), with: nil))
        XCTAssertFalse(overlay.point(inside: CGPoint(x: 400, y: 200), with: nil))
    }
}
