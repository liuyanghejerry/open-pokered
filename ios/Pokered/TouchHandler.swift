import UIKit

@MainActor
final class TouchHandler {

    // MARK: - Bit constants

    private static let bitA:      UInt8 = 1 << 0
    private static let bitB:      UInt8 = 1 << 1
    private static let bitSelect: UInt8 = 1 << 2
    private static let bitStart:  UInt8 = 1 << 3
    private static let bitRight:  UInt8 = 1 << 4
    private static let bitLeft:   UInt8 = 1 << 5
    private static let bitUp:     UInt8 = 1 << 6
    private static let bitDown:   UInt8 = 1 << 7

    // MARK: - State

    private var activeTouches: [ObjectIdentifier: UInt8] = [:]

    // MARK: - Public API

    func currentInputBits() -> UInt8 {
        return activeTouches.values.reduce(0, |)
    }

    func touchesBegan(_ touches: Set<UITouch>, in view: UIView) {
        for touch in touches {
            activeTouches[ObjectIdentifier(touch)] = bits(for: touch, in: view)
        }
    }

    func touchesMoved(_ touches: Set<UITouch>, in view: UIView) {
        for touch in touches {
            activeTouches[ObjectIdentifier(touch)] = bits(for: touch, in: view)
        }
    }

    func touchesEnded(_ touches: Set<UITouch>, in view: UIView) {
        for touch in touches {
            activeTouches.removeValue(forKey: ObjectIdentifier(touch))
        }
    }

    func touchesCancelled(_ touches: Set<UITouch>, in view: UIView) {
        for touch in touches {
            activeTouches.removeValue(forKey: ObjectIdentifier(touch))
        }
    }

    func drawOverlay(in context: CGContext, size: CGSize) {
        let w     = size.width
        let h     = size.height
        let btnH  = h * 0.25
        let gameH = h - btnH

        context.setFillColor(red: 40.0/255, green: 40.0/255, blue: 50.0/255, alpha: 80.0/255)
        context.fill(CGRect(x: 0, y: gameH, width: w, height: btnH))

        let btnMidY = gameH + btnH / 2.0
        let radius  = btnH / 3.0

        fillCircle(in: context,
                   cx: w / 6.0, cy: btnMidY, r: radius,
                   red: 255.0/255, green: 80.0/255, blue: 80.0/255, alpha: 80.0/255)

        fillCircle(in: context,
                   cx: w * 5.0 / 6.0, cy: btnMidY, r: radius,
                   red: 80.0/255, green: 160.0/255, blue: 255.0/255, alpha: 80.0/255)

        fillCircle(in: context,
                   cx: w / 2.0, cy: gameH + btnH / 4.0, r: radius / 2.0,
                   red: 1.0, green: 1.0, blue: 1.0, alpha: 64.0/255)

        fillCircle(in: context,
                   cx: w / 2.0, cy: gameH + btnH * 3.0 / 4.0, r: radius / 2.0,
                   red: 1.0, green: 1.0, blue: 1.0, alpha: 64.0/255)

        let padCX    = w / 2.0
        let padCY    = gameH / 2.0
        let padR     = gameH / 6.0
        let arrowR   = padR / 3.0
        let dpadAlpha: CGFloat = 40.0 / 255.0

        fillCircle(in: context, cx: padCX,        cy: padCY - padR, r: arrowR, red: 1, green: 1, blue: 1, alpha: dpadAlpha)
        fillCircle(in: context, cx: padCX,        cy: padCY + padR, r: arrowR, red: 1, green: 1, blue: 1, alpha: dpadAlpha)
        fillCircle(in: context, cx: padCX - padR, cy: padCY,        r: arrowR, red: 1, green: 1, blue: 1, alpha: dpadAlpha)
        fillCircle(in: context, cx: padCX + padR, cy: padCY,        r: arrowR, red: 1, green: 1, blue: 1, alpha: dpadAlpha)
    }

    // MARK: - Private helpers

    private func bits(for touch: UITouch, in view: UIView) -> UInt8 {
        let insets = view.safeAreaInsets
        let raw    = touch.location(in: view)
        let lx     = raw.x - insets.left
        let ly     = raw.y - insets.top
        let w      = view.bounds.width  - insets.left - insets.right
        let h      = view.bounds.height - insets.top  - insets.bottom

        guard w > 0, h > 0 else { return 0 }

        let btnH  = h * 0.25
        let gameH = h - btnH

        if ly < gameH {
            let col = Int(lx / (w / 3.0))
            let row = Int(ly / (gameH / 3.0))
            switch (col, row) {
            case (1, 0): return TouchHandler.bitUp
            case (0, 1): return TouchHandler.bitLeft
            case (2, 1): return TouchHandler.bitRight
            case (1, 2): return TouchHandler.bitDown
            default:     return 0
            }
        } else {
            let rx = lx
            let ry = ly - gameH

            if rx < w / 3.0 {
                return TouchHandler.bitA
            } else if rx > w * 2.0 / 3.0 {
                return TouchHandler.bitB
            } else if ry < btnH / 2.0 {
                return TouchHandler.bitStart
            } else {
                return TouchHandler.bitSelect
            }
        }
    }

    private func fillCircle(in context: CGContext,
                            cx: CGFloat, cy: CGFloat, r: CGFloat,
                            red: CGFloat, green: CGFloat, blue: CGFloat, alpha: CGFloat) {
        context.setFillColor(red: red, green: green, blue: blue, alpha: alpha)
        context.fillEllipse(in: CGRect(x: cx - r, y: cy - r, width: r * 2, height: r * 2))
    }
}
