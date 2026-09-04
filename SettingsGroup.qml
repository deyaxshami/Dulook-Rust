import QtQuick
import qs.Commons
import qs.Ui

BorderSurface {
  id: root

  default property alias contentItems: contentColumn.children
  property color foreground: Color.foreground

  implicitHeight: contentColumn.implicitHeight + contentTopInset + contentBottomInset
  height: implicitHeight
  padding: Style.spacing.xl
  radius: Style.cornerRadius
  color: Style.normalFillFor(foreground, Color.accent)
  borderSpec: Border.controlSpec("normal", foreground, Color.accent)

  Column {
    id: contentColumn
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.top: parent.top
    anchors.leftMargin: root.contentLeftInset
    anchors.rightMargin: root.contentRightInset
    anchors.topMargin: root.contentTopInset
    spacing: Style.spacing.lg
  }
}
