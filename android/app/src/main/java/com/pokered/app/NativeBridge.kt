package com.pokered.app

object NativeBridge {
    init {
        System.loadLibrary("pokered_android")
    }

    external fun pressButton(button: Int)
    external fun releaseButton(button: Int)
}
