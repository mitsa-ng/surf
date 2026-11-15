package com.surf.browser

import android.os.Bundle
import androidx.core.view.WindowCompat

// targetSdk 35+ enforces edge-to-edge regardless of the theme, which
// makes windowSoftInputMode="adjustResize" inert and lets the on-screen
// keyboard cover the omnibox and its suggestion dropdown. Opting back
// into decor fitting (the pre-35 default) lets the window — and with it
// the webview viewport — shrink while typing, lifting the bottom chrome
// above the keyboard; system bars are inset natively again.
class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    WindowCompat.setDecorFitsSystemWindows(window, true)
    super.onCreate(savedInstanceState)
  }
}
