import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Frame {
    id: card
    property string heading
    property string detail
    property string status: ""
    padding: 20
    Layout.fillWidth: true
    background: Rectangle {
        color: card.palette.base
        radius: 12
        border.color: card.palette.mid
        border.width: card.activeFocus ? 2 : 1
    }
    ColumnLayout {
        anchors.fill: parent
        spacing: 10
        RowLayout {
            Layout.fillWidth: true
            Label {
                text: card.heading
                textFormat: Text.PlainText
                font.pixelSize: 17
                font.weight: Font.DemiBold
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }
            Label {
                visible: card.status.length > 0
                text: card.status
                textFormat: Text.PlainText
                font.pixelSize: 13
                font.weight: Font.Medium
                padding: 7
                background: Rectangle { radius: 6; color: card.palette.alternateBase }
                Accessible.name: card.heading + ": " + card.status
            }
        }
        Label {
            visible: card.detail.length > 0
            objectName: card.objectName.length > 0 ? card.objectName + "Detail" : ""
            text: card.detail
            textFormat: Text.PlainText
            color: card.palette.text
            opacity: 0.85
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
            font.pixelSize: 14
            lineHeight: 1.2
        }
    }
}
