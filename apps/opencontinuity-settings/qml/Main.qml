import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ApplicationWindow {
    id: window
    objectName: "settingsWindow"
    visible: true
    width: 1120
    height: 760
    minimumWidth: 760
    minimumHeight: 560
    title: "OpenContinuity Settings"
    font.pixelSize: 14
    property alias currentPage: navigation.currentIndex
    readonly property var pageNames: ["Overview", "Devices", "Apple Watch", "AirDrop", "AirPods", "Continuity", "Research Lab / Diagnostics", "About"]
    readonly property bool compact: width < 1000
    SystemPalette { id: systemColors; colorGroup: SystemPalette.Active }
    palette.window: systemColors.window
    palette.windowText: systemColors.windowText
    palette.base: systemColors.base
    palette.alternateBase: systemColors.alternateBase
    palette.text: systemColors.text
    palette.button: systemColors.button
    palette.buttonText: systemColors.buttonText
    palette.mid: systemColors.mid
    palette.midlight: systemColors.midlight
    palette.highlight: systemColors.highlight
    palette.highlightedText: systemColors.highlightedText
    color: palette.window
    Shortcut { sequence: "Ctrl+R"; onActivated: doctor.refresh() }
    Shortcut { sequence: "Alt+Left"; onActivated: window.currentPage = Math.max(0, window.currentPage - 1) }
    Shortcut { sequence: "Alt+Right"; onActivated: window.currentPage = Math.min(7, window.currentPage + 1) }
    RowLayout {
        anchors.fill: parent
        spacing: 0
        Rectangle {
            Layout.preferredWidth: window.compact ? 205 : 250
            Layout.fillHeight: true
            color: window.palette.alternateBase
            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 16
                spacing: 22
                RowLayout {
                    Image { source: "qrc:/assets/opencontinuity-settings.svg"; sourceSize.width: 36; sourceSize.height: 36 }
                    Label { text: "OpenContinuity"; font.weight: Font.Bold; font.pixelSize: 16 }
                }
                ListView {
                    id: navigation
                    objectName: "navigation"
                    Layout.fillHeight: true
                    Layout.fillWidth: true
                    spacing: 5
                    clip: true
                    model: window.pageNames
                    currentIndex: 0
                    keyNavigationEnabled: true
                    activeFocusOnTab: true
                    Accessible.name: "Settings pages"
                    delegate: ItemDelegate {
                        required property int index
                        required property string modelData
                        objectName: "nav" + index
                        width: navigation.width
                        text: modelData
                        font.pixelSize: 14
                        highlighted: window.currentPage === index
                        background: Rectangle {
                            radius: 8
                            color: parent.highlighted ? window.palette.highlight : parent.hovered ? window.palette.midlight : "transparent"
                            border.width: parent.activeFocus || (navigation.activeFocus && parent.highlighted) ? 2 : 0
                            border.color: parent.highlighted ? window.palette.highlightedText : window.palette.highlight
                        }
                        activeFocusOnTab: true
                        onClicked: window.currentPage = index
                        ToolTip.visible: hovered
                        ToolTip.text: modelData
                        Accessible.name: modelData
                        Accessible.role: Accessible.PageTab
                    }
                }
                Label {
                    text: "Read-only foundation\nNo Linux unlock"
                    font.pixelSize: 12
                    opacity: 0.65
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                }
            }
        }
        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.margins: window.compact ? 22 : 36
            spacing: 18
            Frame {
                Layout.fillWidth: true
                padding: 12
                background: Rectangle {
                    radius: 10
                    color: window.palette.alternateBase
                    border.color: window.palette.mid
                }
                RowLayout {
                    anchors.fill: parent
                    ColumnLayout {
                        Layout.fillWidth: true
                        Label {
                            objectName: "reportStatus"
                            text: "Build report · " + doctor.reportState + (doctor.failureState && doctor.reportState === "stale" ? " / " + doctor.failureState : "")
                            textFormat: Text.PlainText
                            font.weight: Font.DemiBold
                            wrapMode: Text.WordWrap
                            Layout.fillWidth: true
                        }
                        Label {
                            text: doctor.lastError || (doctor.lastUpdated ? "CLI " + doctor.reportVersion + " · schema " + doctor.schemaVersion + " · " + doctor.lastUpdated : "Waiting for a validated CLI report")
                            textFormat: Text.PlainText
                            wrapMode: Text.WordWrap
                            Layout.fillWidth: true
                            font.pixelSize: 12
                        }
                        Label {
                            visible: doctor.reportState !== "loaded" && doctor.schemaVersion > 0
                            text: "Displayed capability rows are from the last valid report."
                            wrapMode: Text.WordWrap
                            Layout.fillWidth: true
                            font.weight: Font.DemiBold
                            font.pixelSize: 12
                        }
                    }
                    BusyIndicator { running: doctor.loading; visible: running; implicitWidth: 28; implicitHeight: 28 }
                    Button { text: "Cancel"; visible: doctor.loading; onClicked: doctor.cancel() }
                    Button { objectName: "refresh"; text: "Refresh"; enabled: !doctor.loading; onClicked: doctor.refresh(); Accessible.description: "Read static build capabilities from the CLI. Ctrl+R" }
                }
            }
            Loader {
                id: content
                objectName: "pageLoader"
                Layout.fillWidth: true
                Layout.fillHeight: true
                sourceComponent: SettingsPage { pageIndex: window.currentPage; pageName: window.pageNames[window.currentPage] }
            }
        }
    }
}
