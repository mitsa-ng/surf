package com.surf.browser

import android.os.Bundle
import android.view.View
import android.view.ViewGroup
import android.webkit.WebView
import androidx.activity.OnBackPressedCallback
import androidx.core.view.WindowCompat

// targetSdk 35+ enforces edge-to-edge regardless of the theme, which
// makes windowSoftInputMode="adjustResize" inert and lets the on-screen
// keyboard cover the omnibox and its suggestion dropdown. Opting back
// into decor fitting (the pre-35 default) lets the window — and with it
// the webview viewport — shrink while typing, lifting the bottom chrome
// above the keyboard; system bars are inset natively again.
class MainActivity : TauriActivity() {
  // The chrome webview is the first one wry creates on this activity;
  // every later RustWebView is a content tab.
  private var chromeWebview: WebView? = null

  override fun onCreate(savedInstanceState: Bundle?) {
    WindowCompat.setDecorFitsSystemWindows(window, true)
    super.onCreate(savedInstanceState)
    // Back contract (TauriActivity disables wry's single-webview default,
    // which exited the app outright): overlays close first — the chrome
    // webview decides via __surfHandleBack — then the visible content
    // webview's history drives; only a page at its history root
    // backgrounds the app, Chrome-style.
    onBackPressedDispatcher.addCallback(
      this,
      object : OnBackPressedCallback(true) {
        override fun handleOnBackPressed() {
          val chrome = chromeWebview
          if (chrome == null) {
            backOrDefault()
            return
          }
          chrome.evaluateJavascript("window.__surfHandleBack && window.__surfHandleBack()") { result ->
            val consumed = result?.contains("true") == true
            if (!consumed) backOrDefault()
          }
        }
      },
    )
  }

  override fun onWebViewCreate(webView: WebView) {
    if (chromeWebview == null) chromeWebview = webView
  }

  private fun backOrDefault() {
    val active = activeContentWebview()
    if (active != null && active.canGoBack()) {
      active.goBack()
    } else {
      moveTaskToBack(true)
    }
  }

  private fun activeContentWebview(): WebView? {
    val frame = findViewById<ViewGroup>(android.R.id.content)
    // bringToFront puts the active tab last; the chrome webview and hidden
    // tabs never count as content.
    return (0 until frame.childCount)
      .map { frame.getChildAt(it) }
      .filterIsInstance<WebView>()
      .filter { it.visibility == View.VISIBLE && it !== chromeWebview }
      .lastOrNull()
  }
}
