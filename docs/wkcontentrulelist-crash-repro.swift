// Minimal reproducer for the WebKit regression documented in NOTES.md:
// `-[WKUserContentController addContentRuleList:]` segfaults with a null
// WebUserContentControllerProxy on macOS 27.2 (WebKit 27-era), even from
// plain Swift with a fully loaded page. Run with:
//   swift docs/wkcontentrulelist-crash-repro.swift
// It prints "compiled ok" then crashes inside addContentRuleList when the
// bug is present; prints "ADDED OK" when WebKit is fixed.
import Cocoa
import WebKit

let app = NSApplication.shared
let ctrl = WKUserContentController()
let cfg = WKWebViewConfiguration()
cfg.userContentController = ctrl
let wv = WKWebView(frame: NSRect(x: 0, y: 0, width: 200, height: 200), configuration: cfg)
let json = #" [{"trigger":{"url-filter":"example"},"action":{"type":"block"}}] "#
WKContentRuleListStore.default().compileContentRuleList(
    forIdentifier: nil, encodedContentRuleList: json)
{ rules, err in
    guard let rules = rules else { print("compile err:", err as Any); app.terminate(nil); return }
    print("compiled ok")
    DispatchQueue.main.async {
        ctrl.add(rules)
        print("ADDED OK")
        app.terminate(nil)
    }
}
wv.load(URL(string: "https://example.org/")!)
app.run()
