// Native startup and mouse-held tug on macOS when Accessibility permission is available.
import Foundation
import AppKit
import CoreGraphics
import ApplicationServices

let args = CommandLine.arguments
guard args.count == 3 else { fatalError("Usage: swift smoke-macos.swift APPLICATION OUTPUT_DIRECTORY") }
let directory = URL(fileURLWithPath: args[2], isDirectory: true)
try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
let app = Process()
app.executableURL = URL(fileURLWithPath: args[1]).standardizedFileURL
let output = directory.appendingPathComponent("application.log")
FileManager.default.createFile(atPath: output.path, contents: nil)
let handle = try FileHandle(forWritingTo: output)
app.standardOutput = handle; app.standardError = handle
var report: [String: Any] = ["platform":"macOS", "scope":"native startup and visible window only", "passed":[String](), "interaction":"not run", "multi_display":"not run"]
var passed = [String]()
var failure: String? = nil
func ownWindows() -> [[String: Any]] {
    let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly,.excludeDesktopElements],kCGNullWindowID) as? [[String: Any]] ?? []
    return windows.filter { ($0[kCGWindowOwnerPID as String] as? Int) == Int(app.processIdentifier) }
}
func named(_ name: String) -> [String: Any]? {
    return ownWindows().first { ($0[kCGWindowName as String] as? String) == name }
}
func bounds(_ window: [String: Any]) -> CGRect {
    let b = window[kCGWindowBounds as String] as! [String: Any]
    return CGRect(x: b["X"] as! Double,y: b["Y"] as! Double,width: b["Width"] as! Double,height: b["Height"] as! Double)
}
func require(_ condition: Bool, _ name: String) throws {
    if !condition { throw NSError(domain:"NativeSmoke",code:2,userInfo:[NSLocalizedDescriptionKey:name]) }
    passed.append(name)
}
func waitWindow(_ name: String, visible: Bool = true, timeout: Double = 15) throws -> [String: Any]? {
    let end = Date().addingTimeInterval(timeout)
    while Date() < end && app.isRunning {
        let window = named(name)
        if (window != nil) == visible { return window }
        Thread.sleep(forTimeInterval:0.08)
    }
    throw NSError(domain:"NativeSmoke",code:3,userInfo:[NSLocalizedDescriptionKey:"Timed out waiting for native window: \(name), visible=\(visible)"])
}
func stableWindow(_ name: String) throws -> [String: Any] {
    let end=Date().addingTimeInterval(10)
    var previous: CGRect?=nil
    var stableSince=Date()
    while Date()<end {
        let window=try waitWindow(name)!
        let current=bounds(window)
        if current != previous { previous=current;stableSince=Date() }
        if Date().timeIntervalSince(stableSince)>0.4 { return window }
        Thread.sleep(forTimeInterval:0.08)
    }
    throw NSError(domain:"NativeSmoke",code:5,userInfo:[NSLocalizedDescriptionKey:"Window geometry did not settle: \(name)"])
}
func hoverPet(_ x: Double,_ y: Double) throws -> CGRect {
    let r=bounds(try waitWindow("大熊")!)
    mouse(.mouseMoved,CGPoint(x:r.minX+x,y:r.minY+y))
    return r
}
func mouse(_ kind: CGEventType, _ point: CGPoint, _ button: CGMouseButton = .left) {
    CGEvent(mouseEventSource:nil,mouseType:kind,mouseCursorPosition:point,mouseButton:button)?.post(tap:.cghidEventTap)
}
func key(_ code: CGKeyCode) {
    CGEvent(keyboardEventSource:nil,virtualKey:code,keyDown:true)?.post(tap:.cghidEventTap)
    CGEvent(keyboardEventSource:nil,virtualKey:code,keyDown:false)?.post(tap:.cghidEventTap)
    Thread.sleep(forTimeInterval:0.12)
}
func petMenu() throws {
    let window = try waitWindow("大熊")!
    let r = bounds(window);let point = CGPoint(x:r.minX+150,y:r.minY+160)
    mouse(.mouseMoved,point);Thread.sleep(forTimeInterval:0.3)
    mouse(.rightMouseDown,point,.right);mouse(.rightMouseUp,point,.right)
    Thread.sleep(forTimeInterval:0.4)
}
func screenshot(_ name: String) throws {
    if CGPreflightScreenCaptureAccess() {
        let shot=Process();shot.executableURL=URL(fileURLWithPath:"/usr/sbin/screencapture")
        shot.arguments=["-x",directory.appendingPathComponent(name).path]
        try shot.run();shot.waitUntilExit()
        if shot.terminationStatus != 0 { throw NSError(domain:"NativeSmoke",code:4,userInfo:[NSLocalizedDescriptionKey:"Screen capture failed"]) }
    }
}
do {
    try app.run()
    var pet: [String: Any]? = nil
    let deadline = Date().addingTimeInterval(40)
    while Date() < deadline && app.isRunning {
        let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly,.excludeDesktopElements],kCGNullWindowID) as? [[String: Any]] ?? []
        pet = windows.first { window in
            guard let pid = window[kCGWindowOwnerPID as String] as? Int,
                  let bounds = window[kCGWindowBounds as String] as? [String: Any],
                  let width = bounds["Width"] as? Double,
                  let height = bounds["Height"] as? Double else { return false }
            return pid == Int(app.processIdentifier) && width >= 290 && width <= 310 && height >= 230 && height <= 250
        }
        if pet != nil { break }
        Thread.sleep(forTimeInterval: 0.1)
    }
    guard let petWindow = pet else { throw NSError(domain:"NativeSmoke",code:1,userInfo:[NSLocalizedDescriptionKey:"No visible 300×240 pet window, or process exited"] ) }
    passed.append("built application starts and creates a visible native pet window")
    report["window"] = petWindow
    report["accessibility_ready"] = AXIsProcessTrusted()
    report["screen_recording_ready"] = CGPreflightScreenCaptureAccess()
    try screenshot("desktop.png")
    if AXIsProcessTrusted() {
        report["scope"] = "native startup, rapid gaze, head/belly contact, menu, panel and mouse-held tug; single display"
        Thread.sleep(forTimeInterval:1)
        _=try hoverPet(35,52);Thread.sleep(forTimeInterval:0.5)
        let dizzyBefore=bounds(try waitWindow("大熊")!)
        for i in 0..<14 {
            mouse(.mouseMoved,CGPoint(x:dizzyBefore.minX+(i%2==0 ? 35:265),y:dizzyBefore.minY+52))
            Thread.sleep(forTimeInterval:0.085)
        }
        Thread.sleep(forTimeInterval:0.6);try screenshot("dizzy-stars.png")
        try require(bounds(try waitWindow("大熊")!).origin==dizzyBefore.origin,"rapid gaze keeps the native window stationary")
        report["dizzy_visuals"]="real rapid native gaze and screenshot; star and pose inspection is separate from the stationary check"
        _=try hoverPet(35,52);Thread.sleep(forTimeInterval:4)
        let headBefore=try hoverPet(145,125);Thread.sleep(forTimeInterval:2)
        try screenshot("head-rub.png")
        try require(bounds(try waitWindow("大熊")!).origin==headBefore.origin,"head contact keeps the native window stationary")
        mouse(.mouseMoved,CGPoint(x:5,y:5));Thread.sleep(forTimeInterval:5)
        let bellyBefore=try hoverPet(178,199);Thread.sleep(forTimeInterval:1.8)
        _=try hoverPet(172,210);Thread.sleep(forTimeInterval:1.5)
        try screenshot("belly-rub.png")
        try require(bounds(try waitWindow("大熊")!).origin==bellyBefore.origin,"belly contact keeps the native window stationary")
        report["petting_visuals"]="real pointer head/belly screenshots; pose inspection is separate from stationary checks"
        mouse(.mouseMoved,CGPoint(x:5,y:5));Thread.sleep(forTimeInterval:1.5)
        try petMenu();key(115);key(36)
        let panel=try waitWindow("和大熊一起玩")!
        try require(!bounds(panel).isEmpty,"real right-click menu opens interaction panel")
        // Use the native titlebar close button; the application should hide the panel.
        NSRunningApplication(processIdentifier:app.processIdentifier)?.activate(options:[.activateIgnoringOtherApps])
        let pr=bounds(try stableWindow("和大熊一起玩"));let close=CGPoint(x:pr.minX+13,y:pr.minY+13)
        mouse(.mouseMoved,close);Thread.sleep(forTimeInterval:0.15);mouse(.leftMouseDown,close);mouse(.leftMouseUp,close)
        _=try waitWindow("和大熊一起玩",visible:false)
        try require(app.isRunning,"closing panel keeps pet alive")
        try petMenu();key(115)
        for _ in 0..<4 { key(125) }
        key(36)
        _=try waitWindow("大熊的球")
        Thread.sleep(forTimeInterval:0.35)
        let toy=try waitWindow("大熊的球")!
        let tr=bounds(toy);let mr=bounds(try waitWindow("大熊")!)
        // Infer the actual rope side, rather than assuming the main display's work area.
        let direction:Double = tr.midX > mr.midX ? 1 : -1
        let grab=CGPoint(x:tr.midX+direction*40,y:tr.midY)
        mouse(.mouseMoved,grab);Thread.sleep(forTimeInterval:0.3)
        mouse(.leftMouseDown,grab);Thread.sleep(forTimeInterval:0.2)
        let before=bounds(try waitWindow("大熊")!)
        let pull=CGPoint(x:grab.x+direction*70,y:grab.y)
        report["rope_input"]=["grabX":Double(grab.x),"grabY":Double(grab.y),"pullX":Double(pull.x),"pullY":Double(pull.y)]
        mouse(.leftMouseDragged,pull)
        var resisted=false
        for _ in 0..<21 {
            Thread.sleep(forTimeInterval:0.1)
            if bounds(try waitWindow("大熊")!).minX != before.minX {resisted=true}
        }
        let after=bounds(try waitWindow("大熊")!)
        report["pet_motion"]=["beforeX":Double(before.minX),"afterX":Double(after.minX)]
        try require(resisted,"real held rope makes pet resist")
        try screenshot("tug-pulling.png")
        mouse(.leftMouseUp,pull)
        _=try waitWindow("大熊的球",visible:false,timeout:1.5)
        passed.append("release hides rope for celebration")
        _=try waitWindow("大熊的球",timeout:5)
        passed.append("rope returns for another round")
        try screenshot("tug-ready.png")
        report["interaction"]="passed native menu and tug smoke"
    } else {
        report["interaction_blocker"] = "Runner lacks Accessibility permission; run the manual checklist on an authorized Mac"
    }
} catch {
    failure = error.localizedDescription
    try? screenshot("failure.png")
}
if let cursor=CGEvent(source:nil)?.location { mouse(.leftMouseUp,cursor) }
if app.isRunning { app.terminate();app.waitUntilExit() }
report["passed"] = passed
if let error = failure { report["error"] = error } else { report["error"] = NSNull() }
let data = try JSONSerialization.data(withJSONObject:report,options:[.prettyPrinted,.sortedKeys])
try data.write(to:directory.appendingPathComponent("native-smoke.json"))
print(String(data:data,encoding:.utf8)!)
if failure != nil { exit(1) }
