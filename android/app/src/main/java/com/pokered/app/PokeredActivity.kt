package com.pokered.app

import android.app.NativeActivity
import android.graphics.PixelFormat
import android.os.Build
import android.os.Bundle
import android.view.Gravity
import android.view.WindowInsets
import android.view.WindowManager

class PokeredActivity : NativeActivity() {
    private var gamepadView: GamepadView? = null
    private var overlayAdded = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        // NativeActivity calls Window.takeSurface() AND Window.takeInputQueue() in its
        // onCreate, so any View living in the activity's own content window neither draws
        // (the native wgpu renderer owns the window surface) nor receives touch (input is
        // routed straight to the native input queue). Hosting the gamepad in a SEPARATE
        // panel window gives it its own input channel — registered independently with the
        // system InputDispatcher — and composites it above the native game surface.
        gamepadView = GamepadView(this).apply {
            isFocusable = false
            onButtonEvent = { buttonCode, pressed ->
                if (pressed) {
                    NativeBridge.pressButton(buttonCode)
                } else {
                    NativeBridge.releaseButton(buttonCode)
                }
            }
            setOnApplyWindowInsetsListener { _, insets ->
                val bottomInset = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                    insets.getInsets(WindowInsets.Type.systemBars()).bottom
                } else {
                    @Suppress("DEPRECATION")
                    insets.systemWindowInsetBottom
                }
                setBottomInset(bottomInset)
                insets
            }
        }

        // decorView.windowToken is only valid once the activity window is attached,
        // which happens after onCreate returns — defer until then.
        window.decorView.post { addOverlayWindow() }
    }

    private fun addOverlayWindow() {
        if (overlayAdded) return
        val winToken = window.decorView.windowToken ?: return

        val params = WindowManager.LayoutParams(
            WindowManager.LayoutParams.MATCH_PARENT,
            WindowManager.LayoutParams.WRAP_CONTENT,
            // Child window of this activity (uses its token); no special permission needed.
            WindowManager.LayoutParams.TYPE_APPLICATION_PANEL,
            // Touchable (NOT focusable, so we never steal key/IME focus from the game).
            WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE,
            PixelFormat.TRANSLUCENT,
        ).apply {
            token = winToken
            gravity = Gravity.BOTTOM
        }

        windowManager.addView(gamepadView, params)
        overlayAdded = true
        gamepadView?.requestApplyInsets()
    }

    override fun onDestroy() {
        if (overlayAdded) {
            gamepadView?.let { windowManager.removeViewImmediate(it) }
            overlayAdded = false
        }
        super.onDestroy()
    }
}
