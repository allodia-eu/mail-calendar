// Turns a simulator's simulated hardware keyboard on or off: Device Hub's Device > Keyboard >
// Simulate Hardware Keyboard, without the window.
//
// While it is on, the first key that arrives from outside the device (a Mac keystroke, or `idb ui
// text`, which types through the same HID path) tells iOS a keyboard is attached, and from then on
// a focused field raises no on-screen keyboard. The screen then shows what no phone shows, and
// anything that depends on the keyboard's height, a field scrolled clear of it or a list placed
// above it, cannot be seen or tested. Off, a field always raises the on-screen keyboard and typing
// from the Mac still reaches it.
//
// No `simctl` command reaches the switch, so this calls CoreSimulator's own, the one Device Hub
// uses. It is private API; when a new Xcode moves it, this fails with "no such method" rather than
// doing anything else.
//
// Run with `xcrun swift scripts/dev/sim-keyboard.swift <udid> <on|off>`, or through
// `scripts/dev/control.sh iphone hardware-keyboard <on|off>`, which finds the booted simulator.

import Foundation

let args = CommandLine.arguments
guard args.count == 3, ["on", "off"].contains(args[2]) else {
    fputs("usage: sim-keyboard.swift <udid> <on|off>\n", stderr)
    exit(2)
}
guard dlopen("/Library/Developer/PrivateFrameworks/CoreSimulator.framework/CoreSimulator", RTLD_NOW) != nil,
    let contextClass = NSClassFromString("SimServiceContext")
else {
    fputs("CoreSimulator is not installed\n", stderr)
    exit(1)
}

/// Fails loudly on a method this Xcode no longer has, rather than calling into nothing.
func implementation(_ target: AnyObject, _ name: String) -> (Selector, IMP) {
    let selector = NSSelectorFromString(name)
    guard target.responds(to: selector), let imp = target.method(for: selector) else {
        fputs("CoreSimulator has no such method: \(name)\n", stderr)
        exit(1)
    }
    return (selector, imp)
}

let developerDir = ProcessInfo.processInfo.environment["DEVELOPER_DIR"]
    ?? "/Applications/Xcode.app/Contents/Developer"

typealias ContextFn = @convention(c) (AnyClass, Selector, NSString, UnsafeMutablePointer<NSError?>?) -> NSObject?
let (contextSel, contextImp) = implementation(contextClass, "sharedServiceContextForDeveloperDir:error:")
guard let context = unsafeBitCast(contextImp, to: ContextFn.self)(contextClass, contextSel, developerDir as NSString, nil)
else {
    fputs("no CoreSimulator service for \(developerDir)\n", stderr)
    exit(1)
}

typealias DeviceSetFn = @convention(c) (NSObject, Selector, UnsafeMutablePointer<NSError?>?) -> NSObject?
let (setSel, setImp) = implementation(context, "defaultDeviceSetWithError:")
guard let deviceSet = unsafeBitCast(setImp, to: DeviceSetFn.self)(context, setSel, nil),
    let devices = deviceSet.value(forKey: "devices") as? [NSObject],
    let device = devices.first(where: {
        ($0.value(forKey: "UDID") as? NSUUID)?.uuidString == args[1].uppercased()
    })
else {
    fputs("no simulator with UDID \(args[1])\n", stderr)
    exit(1)
}

typealias KeyboardFn = @convention(c) (NSObject, Selector, Bool, UInt8, UnsafeMutablePointer<NSError?>?) -> Bool
let (keyboardSel, keyboardImp) = implementation(device, "setHardwareKeyboardEnabled:keyboardType:error:")
var error: NSError?
guard unsafeBitCast(keyboardImp, to: KeyboardFn.self)(device, keyboardSel, args[2] == "on", 0, &error) else {
    fputs("could not switch the hardware keyboard: \(error?.localizedDescription ?? "no reason given")\n", stderr)
    exit(1)
}
print("hardware keyboard \(args[2]) for \(device.value(forKey: "name") ?? args[1])")
