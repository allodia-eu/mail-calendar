import Foundation
import MailcalBindings
#if os(iOS)
import UIKit
#else
import IOKit.ps
#endif

/// The device facts every `MailcalApp` constructor reports to the core (`docs/analytics.md`).
///
/// Two things to keep in mind when touching this:
///
/// 1. **Report raw; the core coarsens.** We hand over the full OS version (`15.4.1`) and the
///    host's own locale tag (`nl-NL`); the core reduces them to a major and a language it ships
///    (`15`, `nl`) before anything crosses the wire. The reduction rule lives in one tested place
///    in Rust rather than being reimplemented per platform, and it means no client can widen the
///    payload by reporting something more precise than was asked for.
/// 2. **Nothing here is sent unless the user opted in.** These facts are handed to the core at
///    construction regardless, but the core mints no identifier and sends no event until consent
///    is given. Building this value is not "collecting" anything.
///
/// We deliberately do **not** report a raw model string (`MacBookPro18,3`). It is the strongest
/// identifier an otherwise low-entropy payload could carry, and App Store Connect already reports
/// exact models to us for free. We do not even *read* one: the laptop/desktop split comes from
/// whether the machine has an internal battery, which is the thing the class actually means.
enum DeviceFacts {
    /// Main-actor bound because `UIDevice.current` is: the interface idiom is UI state, and this is
    /// the one fact here that cannot be read from anywhere. Every caller already had a main actor
    /// to hand or can reach one before it starts work.
    @MainActor
    static func current() -> DeviceInfo {
        DeviceInfo(
            platform: platform(),
            osVersion: ProcessInfo.processInfo.operatingSystemVersion.dotted,
            deviceClass: deviceClass(),
            appVersion: appVersion(),
            locale: Locale.current.identifier
        )
    }

    @MainActor
    private static func platform() -> Platform {
        #if os(iOS)
        return UIDevice.current.userInterfaceIdiom == .pad ? .ipados : .ios
        #else
        return .macos
        #endif
    }

    @MainActor
    private static func deviceClass() -> DeviceClass {
        #if os(iOS)
        switch UIDevice.current.userInterfaceIdiom {
        case .pad: return .ipad
        case .phone: return .iphone
        default: return .unknown
        }
        #else
        return hasInternalBattery() ? .macLaptop : .macDesktop
        #endif
    }

    #if os(macOS)
    /// Laptop or desktop, decided by whether the machine has an internal battery.
    ///
    /// The obvious approach, `hw.model`, checking for a `MacBook` prefix, is **broken on every
    /// Apple Silicon Mac**: they report `Mac14,15`, `Mac15,3` and so on, and only the Intel models
    /// were ever named `MacBookPro18,3`. It silently classified an M2 MacBook Air as a desktop,
    /// which is most of the fleet. A battery is what "laptop" has actually meant all along, and it
    /// needs no model string at all, so nothing identifying is even read.
    private static func hasInternalBattery() -> Bool {
        guard let snapshot = IOPSCopyPowerSourcesInfo()?.takeRetainedValue(),
              let sources = IOPSCopyPowerSourcesList(snapshot)?.takeRetainedValue() as? [CFTypeRef]
        else {
            return false
        }
        return sources.contains { source in
            let description = IOPSGetPowerSourceDescription(snapshot, source)?
                .takeUnretainedValue() as? [String: Any]
            return description?[kIOPSTypeKey] as? String == kIOPSInternalBatteryType
        }
    }
    #endif

    private static func appVersion() -> String {
        (Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String)
            ?? "0.0.0"
    }
}

private extension OperatingSystemVersion {
    /// `15.4.1`, the core keeps only the major.
    var dotted: String { "\(majorVersion).\(minorVersion).\(patchVersion)" }
}
