// Native startup/window evidence on macOS. Interaction needs Accessibility permission.
import Foundation
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
    // Never label launch evidence as mouse interaction validation, even with permission.
    if !AXIsProcessTrusted() { report["interaction_blocker"] = "Runner lacks Accessibility permission; run the manual checklist on an authorized Mac" }
    if CGPreflightScreenCaptureAccess() {
        let shot = Process();shot.executableURL = URL(fileURLWithPath:"/usr/sbin/screencapture")
        shot.arguments = ["-x",directory.appendingPathComponent("desktop.png").path]
        try shot.run();shot.waitUntilExit();report["screenshot_exit"] = shot.terminationStatus
    }
} catch { failure = error.localizedDescription }
if app.isRunning { app.terminate();app.waitUntilExit() }
report["passed"] = passed
if let error = failure { report["error"] = error } else { report["error"] = NSNull() }
let data = try JSONSerialization.data(withJSONObject:report,options:[.prettyPrinted,.sortedKeys])
try data.write(to:directory.appendingPathComponent("native-smoke.json"))
print(String(data:data,encoding:.utf8)!)
if failure != nil { exit(1) }
