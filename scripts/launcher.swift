/// Tiny native macOS launcher for Clean Up.
/// Imports AppKit so macOS treats the .app as a GUI app (no Terminal window).
/// Starts the Bun-compiled web server and stays alive until it exits.

import AppKit
import Foundation

class AppDelegate: NSObject, NSApplicationDelegate {
    let serverProcess = Process()

    /// Kill any leftover clean-up-server processes from a previous session
    /// (e.g. user closed the browser tab without quitting the app).
    private func killStaleServers() {
        let task = Process()
        task.executableURL = URL(fileURLWithPath: "/usr/bin/pkill")
        task.arguments = ["-f", "clean-up-server --web"]
        try? task.run()
        task.waitUntilExit()
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        killStaleServers()

        let bundle = Bundle.main.bundlePath
        let serverBin = bundle + "/Contents/MacOS/clean-up-server"

        serverProcess.executableURL = URL(fileURLWithPath: serverBin)
        serverProcess.arguments = ["--web"]
        serverProcess.terminationHandler = { _ in
            DispatchQueue.main.async {
                NSApp.terminate(nil)
            }
        }

        do {
            try serverProcess.run()
        } catch {
            let alert = NSAlert()
            alert.messageText = "Clean Up"
            alert.informativeText = "Failed to start: \(error.localizedDescription)"
            alert.runModal()
            NSApp.terminate(nil)
        }
    }

    func applicationWillTerminate(_ notification: Notification) {
        if serverProcess.isRunning {
            serverProcess.terminate()
        }
    }
}

let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate
app.run()
