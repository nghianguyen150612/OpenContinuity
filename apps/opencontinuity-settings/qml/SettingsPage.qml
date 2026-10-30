import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ScrollView {
    id: page
    required property int pageIndex
    required property string pageName
    objectName: "settingsPage"
    clip: true
    ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
    contentWidth: availableWidth
    property string capabilityId: pageIndex === 2 ? "apple_watch_auto_unlock" : pageIndex === 3 ? "airdrop" : "airpods_settings"
    property var capability: {
        // Reading reportState makes a completed refresh invalidate this binding.
        const reportState = doctor.reportState
        return doctor.capability(capabilityId)
    }
    ColumnLayout {
        width: page.availableWidth
        spacing: 18
        Label {
            objectName: "pageHeading"
            text: page.pageName
            font.pixelSize: 29
            font.weight: Font.Bold
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
        }
        Label {
            text: ["Your Apple ecosystem, with a clear view of what is available.",
                   "Device discovery will appear here when an adapter is implemented.",
                   "Native Watch integration and its separate trust requirements.",
                   "File sharing through native Apple interoperability.",
                   "Headphone capabilities, without guessed device information.",
                   "A foundation for future cross-device experiences.",
                   "Research evidence is separate from native Apple functionality.",
                   "An independent, open source desktop companion."][page.pageIndex]
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            font.pixelSize: 14
            opacity: 0.75
        }
        Card {
            visible: page.pageIndex === 0
            heading: "A native Settings foundation"
            status: "P011"
            detail: "Explore build capabilities and the evidence behind them. This report is static: it does not scan for devices or test hardware."
        }
        Repeater {
            model: page.pageIndex === 0 ? doctor.capabilities : null
            delegate: Card {
                required property string title
                required property string state
                required property string summary
                heading: title
                status: state
                detail: summary
            }
        }
        Card {
            visible: page.pageIndex === 1
            heading: "No device discovery adapter"
            status: "Not implemented"
            detail: "This app has no discovered-device list. No Bluetooth scan, pairing or background monitoring is performed. Connected devices cannot be determined in this build."
        }
        Card {
            visible: page.pageIndex >= 2 && page.pageIndex <= 4
            heading: page.pageIndex === 2 ? "Apple Watch Auto Unlock" : page.pageName
            status: page.capability.state
            detail: page.capability.summary
        }
        Label {
            visible: page.pageIndex === 2
            text: "Auto Unlock trust stages"
            font.pixelSize: 17
            font.weight: Font.DemiBold
        }
        Repeater {
            model: page.pageIndex === 2 ? doctor.trustStages : null
            delegate: Card {
                required property string title
                required property string state
                heading: title
                status: state
                detail: "Build contract only; no physical assessment has been performed."
            }
        }
        Card {
            visible: page.pageIndex === 3
            heading: "No transfers available"
            detail: "Native AWDL/AirDrop interoperability is not implemented. This app does not advertise, receive or send files."
        }
        Card {
            visible: page.pageIndex === 4
            heading: "No headphone data available"
            detail: "Battery levels, listening modes, pairing and automatic switching are not available. No AirPods have been detected by this application."
        }
        Card {
            visible: page.pageIndex === 5
            heading: "Cross-device features"
            status: "Not implemented"
            detail: "Handoff, Universal Clipboard, AirPlay, HomeKit and iCloud integrations are not implemented. Doctor v1 does not report these capabilities."
        }
        Card {
            visible: page.pageIndex === 6
            heading: "Safety boundary"
            status: "NO-GO"
            detail: "authorization: never_evaluated\nNo Linux unlock is available. Hardware is not verified. Production functionality remains unsupported. Research cannot be started from this application."
        }
        Repeater {
            model: page.pageIndex === 6 ? [
                {name: "P007 / P007B · Synthetic GATT", detail: "Source implemented · software tests passed in prior CI. Synthetic transport is not native Watch identity or Apple Auto Unlock."},
                {name: "P008 · Signed research protocol", detail: "Source implemented · software signature tests passed in prior CI. Independent test pins are not genuine Watch attestation."},
                {name: "P009 / P009A · Authenticated transport", detail: "Source implemented · Rust → Swift → Rust software fixture tests passed in prior CI. Physical BLE and consent are unverified."},
                {name: "P010 · Qualification infrastructure", detail: "Source implemented · software tests and Apple SDK build passed in prior CI. Physical Watch ↔ BlueZ qualification remains NO-GO."}
            ] : []
            delegate: Card {
                required property var modelData
                heading: modelData.name
                status: "Research only"
                detail: modelData.detail
            }
        }
        Card {
            visible: page.pageIndex === 6
            heading: "Evidence and diagnostics"
            detail: "These are documented baseline results, not a live CI or hardware probe. See docs/P010_VALIDATION.md and docs/P011_VALIDATION.md in the repository. Refresh only reads doctor --json."
        }
        Card {
            visible: page.pageIndex === 7
            heading: "OpenContinuity Settings"
            status: "0.1.0"
            detail: "Native Qt 6 / QML · C++20\nPortable Rust capability contracts remain independent of Qt.\nMIT project source · system Qt libraries have separate licenses.\nNo telemetry. No network access is required at startup."
        }
        Card {
            visible: page.pageIndex === 7
            heading: "Project and attribution"
            detail: "github.com/nghianguyen150612/OpenContinuity\nOriginal interface and icon. Not affiliated with Apple. Apple product names identify interoperability goals. See LICENSE and docs/UPSTREAM_MANIFEST.md for distribution obligations."
        }
        Item { Layout.preferredHeight: 12 }
    }
}
