import QtQuick
import Quickshell
import qs.Ui

BarWidget {
  id: root
  moduleName: "karem.dulook"

  readonly property var dulookService: bar && bar.shell ? bar.shell.serviceFor("karem.dulook") : null
  readonly property bool opened: panelLoader.item ? panelLoader.item.opened === true : false
  readonly property bool popoutSwitchClosing: panelLoader.item ? panelLoader.item.popoutSwitchClosing === true : false
  readonly property string label: {
    if (!dulookService) return "☾ Dulook"
    return "☾ " + dulookService.nextPrayerCountdownLabel()
  }

  function injectPanel() {
    var target = panelLoader.item
    if (!target) return
    target.bar = root.bar
    target.settings = root.settings
    target.anchorItem = button
    target.hostWidget = root
    target.service = root.dulookService
  }

  function open() { if (panelLoader.item) panelLoader.item.open() }
  function close() { if (panelLoader.item) panelLoader.item.close() }
  function togglePanel() { if (panelLoader.item) panelLoader.item.toggle() }
  function closeForPopoutSwitch() { if (panelLoader.item) panelLoader.item.closeForPopoutSwitch() }

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight
  onBarChanged: injectPanel()
  onSettingsChanged: injectPanel()
  onDulookServiceChanged: injectPanel()

  Loader {
    id: panelLoader
    active: true
    source: Qt.resolvedUrl("Panel.qml")
    visible: false
    onLoaded: { root.injectPanel(); Qt.callLater(root.injectPanel) }
  }

  WidgetButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: root.vertical ? "☾" : root.label
    labelVisible: true
    horizontalMargin: 8.75
    verticalPadding: 8.75
    onPressed: function(mouseButton) {
      if (!root.dulookService) return
      if (mouseButton === Qt.MiddleButton) root.dulookService.reloadCalendar()
      else if (mouseButton === Qt.RightButton) root.dulookService.nextHadith()
      else root.togglePanel()
    }
  }
}
