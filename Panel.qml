import QtQuick
import QtQuick.Dialogs
import qs.Commons
import qs.Ui

Panel {
  id: root
  moduleName: "karem.dulook"
  manageIpc: false

  property var anchorItem: null
  property var hostWidget: null
  property var service: null
  property bool settingsView: false

  readonly property var barIdentity: hostWidget || root
  readonly property bool rtl: service ? service.language === "ar" : false
  readonly property color foreground: bar ? bar.barForeground : Color.foreground
  readonly property string baseFont: bar ? bar.fontFamily : Style.font.family
  readonly property string uiFont: rtl && service ? service.arabicUiFont : baseFont
  readonly property string hadithFont: rtl && service ? service.arabicHadithFont : baseFont
  readonly property var hadith: service ? service.currentHadith : null

  function tr(key) { return service ? service.tr(key) : key }

  function open() {
    controller.show()
    if (service) service.updateClock(false)
    Qt.callLater(function() { if (root.opened) root.setCenterHoverRevealSuppressed(true) })
  }

  function close() {
    setCenterHoverRevealSuppressed(false)
    controller.hide()
  }

  function switchPanel(direction) {
    if (bar && typeof bar.switchPanelFrom === "function") return bar.switchPanelFrom(barIdentity, direction)
    return false
  }

  function setCenterHoverRevealSuppressed(value) {
    if (bar && "centerHoverRevealSuppressed" in bar) bar.centerHoverRevealSuppressed = value
  }

  function persistSettings(values) {
    var entry = { id: moduleName }
    for (var key in settings) if (key !== "id") entry[key] = settings[key]
    for (var changed in values) entry[changed] = values[changed]
    settings = entry
    if (hostWidget) hostWidget.settings = entry
    if (bar && bar.shell && typeof bar.shell.updateEntryInline === "function")
      bar.shell.updateEntryInline(moduleName, entry)
  }

  function pathFromUrl(url) {
    var value = String(url || "")
    if (value.indexOf("file://") === 0) value = value.substring(7)
    try { return decodeURIComponent(value) } catch (error) { return value }
  }

  function fontOptions() {
    var preferred = ["Noto Sans Arabic", "Noto Naskh Arabic", "Noto Kufi Arabic",
      "Noto Naskh Arabic UI", "Amiri Quran", "Kitab", "Traditional Naskh", "Vazirmatn UI Decurled"]
    var installed = Qt.fontFamilies()
    var result = []
    for (var i = 0; i < preferred.length; i++)
      if (installed.indexOf(preferred[i]) !== -1) result.push(preferred[i])
    return result.length ? result : ["Noto Sans Arabic", "Noto Naskh Arabic"]
  }

  FileDialog {
    id: calendarDialog
    title: root.tr("chooseFile")
    nameFilters: [root.tr("csvFiles") + " (*.csv)", root.tr("allFiles") + " (*)"]
    onAccepted: root.persistSettings({ calendarPath: root.pathFromUrl(selectedFile) })
  }

  FileDialog {
    id: soundDialog
    title: root.tr("chooseFile")
    nameFilters: [root.tr("mp3Files") + " (*.mp3)", root.tr("allFiles") + " (*)"]
    onAccepted: root.persistSettings({ soundPath: root.pathFromUrl(selectedFile) })
  }

  KeyboardPanel {
    id: popup
    anchorItem: root.anchorItem
    owner: root.barIdentity
    bar: root.bar
    open: root.opened
    centerOnBar: true
    focusTarget: keyCatcher
    contentWidth: popup.fittedContentWidth(Style.space(570))
    contentHeight: popup.fittedContentHeight(root.settingsView ? settingsColumn.implicitHeight : mainColumn.implicitHeight,
      popup.screenH * 0.8)

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      LayoutMirroring.enabled: root.rtl
      LayoutMirroring.childrenInherit: true
      onMoveRequested: function(dx, dy) {
        if (!root.settingsView && root.service) {
          if (dx < 0) root.service.nextHadith()
          else if (dx > 0) root.service.previousHadith()
        }
      }
      onCloseRequested: root.close()
      onTabRequested: function(direction) { root.switchPanel(direction) }

      Flickable {
        id: panelFlick
        anchors.fill: parent
        contentWidth: width
        contentHeight: root.settingsView ? settingsColumn.implicitHeight : mainColumn.implicitHeight
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        flickableDirection: Flickable.VerticalFlick
        interactive: contentHeight > height

        Column {
          id: mainColumn
          visible: !root.settingsView
          width: parent.width
          spacing: Style.spacing.xxl

          Row {
            width: parent.width
            spacing: Style.spacing.md
            Text {
              textFormat: Text.PlainText
              width: parent.width - settingsButton.width - parent.spacing
              text: root.tr("appName")
              color: root.foreground
              font.family: root.uiFont
              font.pixelSize: Style.font.title
              font.bold: true
            }
            PanelActionButton {
              id: settingsButton
              iconText: "⚙"
              tooltipText: root.tr("settings")
              foreground: root.foreground
              focusable: true
              onClicked: root.settingsView = true
            }
          }

          BorderSurface {
            width: parent.width
            height: heroContent.implicitHeight + Style.spacing.huge * 2
            radius: Style.cornerRadius
            color: Style.selectedFillFor(root.foreground, Color.accent)
            borderSpec: Border.controlSpec("selected", root.foreground, Color.accent)
            Column {
              id: heroContent
              anchors.centerIn: parent
              width: parent.width - Style.spacing.huge * 2
              spacing: Style.spacing.sm
              Text {
                textFormat: Text.PlainText
                anchors.horizontalCenter: parent.horizontalCenter
                text: root.service && root.service.calendarReady ? root.tr("nextPrayer") : root.tr("setup")
                color: Qt.darker(root.foreground, 1.35)
                font.family: root.uiFont
                font.pixelSize: Style.font.caption
                font.bold: true
              }
              Text {
                textFormat: Text.PlainText
                anchors.horizontalCenter: parent.horizontalCenter
                text: root.service && root.service.nextPrayer
                  ? root.service.prayerName(root.service.nextPrayer.name) + " · " + root.service.formatTime(root.service.nextPrayer.time)
                  : (root.service && root.service.calendarPath !== "" ? root.tr("noPrayerData") : root.tr("noCalendar"))
                color: root.foreground
                font.family: root.uiFont
                font.pixelSize: Style.font.display
                font.bold: true
              }
              Text {
                textFormat: Text.PlainText
                anchors.horizontalCenter: parent.horizontalCenter
                text: root.service ? root.service.countdown : "--:--:--"
                color: Color.accent
                font.family: root.baseFont
                font.pixelSize: Style.font.title
                font.letterSpacing: 2
              }
              Text {
                textFormat: Text.PlainText
                anchors.horizontalCenter: parent.horizontalCenter
                text: root.service ? root.tr("systemTimezone") + " · " + root.service.timezoneLabel() : ""
                color: Qt.darker(root.foreground, 1.5)
                font.family: root.uiFont
                font.pixelSize: Style.font.caption
              }
            }
          }

          Row {
            id: prayerRow
            visible: root.service && root.service.prayerRows.length > 0
            width: parent.width
            spacing: Style.spacing.sm
            Repeater {
              model: root.service ? root.service.prayerRows : []
              BorderSurface {
                required property var modelData
                readonly property bool selectedPrayer: root.service && root.service.nextPrayer
                  && modelData.name === root.service.nextPrayer.name
                  && modelData.prayerDate === root.service.nextPrayer.prayerDate
                width: (prayerRow.width - prayerRow.spacing * 4) / 5
                height: Style.space(68)
                radius: Style.cornerRadius
                color: selectedPrayer ? Style.selectedFillFor(root.foreground, Color.accent)
                  : Style.normalFillFor(root.foreground, Color.accent)
                borderSpec: Border.controlSpec(selectedPrayer ? "selected" : "normal", root.foreground, Color.accent)
                Column {
                  anchors.centerIn: parent
                  spacing: Style.spacing.xs
                  Text {
                    textFormat: Text.PlainText
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: root.service ? root.service.prayerName(modelData.name) : modelData.name
                    color: selectedPrayer ? Color.accent : Qt.darker(root.foreground, 1.35)
                    font.family: root.uiFont
                    font.pixelSize: Style.font.caption
                    font.bold: selectedPrayer
                  }
                  Text {
                    textFormat: Text.PlainText
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: root.service ? root.service.formatTime(modelData.time) : modelData.time
                    color: root.foreground
                    font.family: root.uiFont
                    font.pixelSize: Style.font.bodySmall
                    font.bold: true
                  }
                }
              }
            }
          }

          BorderSurface {
            width: parent.width
            height: hadithContent.implicitHeight + Style.spacing.huge * 2
            radius: Style.cornerRadius
            color: Style.normalFillFor(root.foreground, Color.accent)
            borderSpec: Border.controlSpec("normal", root.foreground, Color.accent)
            Column {
              id: hadithContent
              anchors.centerIn: parent
              width: parent.width - Style.spacing.huge * 2
              spacing: Style.spacing.xxl
              Row {
                width: parent.width
                PanelSectionHeader {
                  width: parent.width - sourceText.width
                  text: root.tr("hadithReminder")
                  foreground: root.foreground
                  fontFamily: root.uiFont
                }
                Text {
                  id: sourceText
                  textFormat: Text.PlainText
                  text: root.hadith && root.hadith.source ? root.tr(root.hadith.source) : ""
                  color: Color.accent
                  font.family: root.uiFont
                  font.pixelSize: Style.font.caption
                }
              }
              Text {
                textFormat: Text.PlainText
                width: parent.width
                text: root.hadith ? (root.rtl ? root.hadith.arabic : root.hadith.english) : root.tr("loadingHadith")
                wrapMode: Text.WordWrap
                color: root.foreground
                font.family: root.hadithFont
                font.pixelSize: Style.font.title
                lineHeight: root.rtl ? 1.55 : 1.35
                horizontalAlignment: root.rtl ? Text.AlignRight : Text.AlignLeft
              }
              Row {
                width: parent.width
                spacing: Style.spacing.sm
                Text {
                  textFormat: Text.PlainText
                  width: parent.width - previousButton.width - nextButton.width - parent.spacing * 2
                  anchors.verticalCenter: parent.verticalCenter
                  text: root.service ? root.service.localizedReference(root.hadith) : ""
                  color: Qt.darker(root.foreground, 1.35)
                  font.family: root.uiFont
                  font.pixelSize: Style.font.bodySmall
                  elide: Text.ElideRight
                }
                Button {
                  id: previousButton
                  text: root.rtl ? "→" : "←"
                  foreground: root.foreground
                  bordered: true
                  focusable: true
                  enabled: root.service && root.service.hadithIndex > 0
                  onClicked: root.service.previousHadith()
                }
                Button {
                  id: nextButton
                  text: root.rtl ? "←" : "→"
                  foreground: root.foreground
                  bordered: true
                  focusable: true
                  enabled: root.service && !root.service.hadithLoading
                  onClicked: root.service.nextHadith()
                }
              }
            }
          }

          Text {
            visible: root.service && root.service.calendarError !== ""
            width: parent.width
            textFormat: Text.PlainText
            text: root.service ? root.service.calendarError : ""
            color: Color.urgent
            font.family: root.uiFont
            font.pixelSize: Style.font.bodySmall
            wrapMode: Text.WordWrap
            horizontalAlignment: Text.AlignHCenter
          }
          Button {
            visible: !root.service || !root.service.calendarReady
            anchors.horizontalCenter: parent.horizontalCenter
            text: root.tr("chooseFile")
            iconText: "+"
            foreground: root.foreground
            bordered: true
            focusable: true
            onClicked: calendarDialog.open()
          }
        }

        Column {
          id: settingsColumn
          visible: root.settingsView
          width: parent.width
          spacing: Style.spacing.xxl

          Row {
            width: parent.width
            spacing: Style.spacing.md
            Button {
              text: root.tr("back")
              iconText: root.rtl ? "→" : "←"
              foreground: root.foreground
              bordered: true
              focusable: true
              onClicked: root.settingsView = false
            }
            Text {
              anchors.verticalCenter: parent.verticalCenter
              textFormat: Text.PlainText
              text: root.tr("settings")
              color: root.foreground
              font.family: root.uiFont
              font.pixelSize: Style.font.title
              font.bold: true
            }
          }

          SettingsGroup {
            width: parent.width
            foreground: root.foreground
            PanelSectionHeader { text: root.tr("appearance"); foreground: root.foreground; fontFamily: root.uiFont }
            Dropdown {
              width: parent.width
              label: root.tr("uiFont")
              value: root.service ? root.service.arabicUiFont : "Noto Sans Arabic"
              options: root.fontOptions()
              foreground: root.foreground
              fontFamily: root.uiFont
              onChanged: function(value) { root.persistSettings({ arabicUiFont: value }) }
            }
            Dropdown {
              width: parent.width
              label: root.tr("hadithFont")
              value: root.service ? root.service.arabicHadithFont : "Noto Naskh Arabic"
              options: root.fontOptions()
              foreground: root.foreground
              fontFamily: root.uiFont
              onChanged: function(value) { root.persistSettings({ arabicHadithFont: value }) }
            }
          }

          SettingsGroup {
            width: parent.width
            foreground: root.foreground
            PanelSectionHeader { text: root.tr("language"); foreground: root.foreground; fontFamily: root.uiFont }
            Dropdown {
              width: parent.width
              value: root.service ? root.service.language : "en"
              options: [{ value: "en", label: "English" }, { value: "ar", label: "العربية" }]
              foreground: root.foreground
              fontFamily: root.uiFont
              showLabel: false
              onChanged: function(value) { root.persistSettings({ language: value }) }
            }
          }

          SettingsGroup {
            width: parent.width
            foreground: root.foreground
            PanelSectionHeader { text: root.tr("calendar"); foreground: root.foreground; fontFamily: root.uiFont }
            TextField {
              width: parent.width
              text: root.service ? root.service.calendarPath : ""
              placeholderText: root.tr("calendarPath")
              foreground: root.foreground
              onEditingFinished: root.persistSettings({ calendarPath: text.trim() })
            }
            Row {
              id: calendarActions
              width: parent.width
              spacing: Style.spacing.sm
              Button { width: (calendarActions.width - calendarActions.spacing * 2) / 3; text: root.tr("chooseFile"); bordered: true; focusable: true; foreground: root.foreground; onClicked: calendarDialog.open() }
              Button { width: (calendarActions.width - calendarActions.spacing * 2) / 3; text: root.tr("reload"); bordered: true; focusable: true; foreground: root.foreground; enabled: root.service && root.service.calendarPath !== ""; onClicked: root.service.reloadCalendar() }
              Button { width: (calendarActions.width - calendarActions.spacing * 2) / 3; text: root.tr("clear"); bordered: true; focusable: true; foreground: root.foreground; enabled: root.service && root.service.calendarPath !== ""; onClicked: root.persistSettings({ calendarPath: "" }) }
            }
            Text {
              width: parent.width
              textFormat: Text.PlainText
              text: root.service && root.service.coverage !== "" ? root.tr("coverage") + ": " + root.service.coverage : (root.service ? root.service.calendarError : "")
              color: root.service && root.service.calendarReady ? Qt.darker(root.foreground, 1.4) : Color.urgent
              font.family: root.uiFont
              font.pixelSize: Style.font.bodySmall
              wrapMode: Text.WordWrap
            }
          }

          SettingsGroup {
            width: parent.width
            foreground: root.foreground
            PanelSectionHeader { text: root.tr("notifications"); foreground: root.foreground; fontFamily: root.uiFont }
            Toggle {
              width: parent.width
              label: root.tr("enableNotifications")
              description: root.tr("enableNotificationsHint")
              checked: root.service ? root.service.notificationsEnabled : true
              foreground: root.foreground
              fontFamily: root.uiFont
              onClicked: root.persistSettings({ notificationsEnabled: !checked })
            }
            Toggle {
              width: parent.width
              label: root.tr("bypassDnd")
              description: root.tr("bypassDndHint")
              checked: root.service ? root.service.bypassDnd : false
              foreground: root.foreground
              fontFamily: root.uiFont
              onClicked: root.persistSettings({ bypassDnd: !checked })
            }
            Grid {
              id: prayerToggleGrid
              width: parent.width
              columns: 2
              columnSpacing: Style.spacing.sm
              rowSpacing: Style.spacing.sm
              Repeater {
                model: ["Fajr", "Dhuhr", "Asr", "Maghrib", "Isha"]
                Toggle {
                  required property string modelData
                  width: (prayerToggleGrid.width - prayerToggleGrid.columnSpacing) / 2
                  label: root.service ? root.service.prayerName(modelData) : modelData
                  checked: root.service ? root.service.prayerEnabled(modelData) : true
                  foreground: root.foreground
                  fontFamily: root.uiFont
                  onClicked: {
                    var values = {}
                    values["notify" + modelData] = !checked
                    root.persistSettings(values)
                  }
                }
              }
            }
            Button { width: parent.width; text: root.tr("testNotification"); bordered: true; focusable: true; foreground: root.foreground; onClicked: root.service.sendNotification("Fajr", true) }
          }

          SettingsGroup {
            width: parent.width
            foreground: root.foreground
            PanelSectionHeader { text: root.tr("sound"); foreground: root.foreground; fontFamily: root.uiFont }
            Toggle {
              width: parent.width
              label: root.tr("enableSound")
              checked: root.service ? root.service.soundEnabled : false
              foreground: root.foreground
              fontFamily: root.uiFont
              onClicked: root.persistSettings({ soundEnabled: !checked })
            }
            TextField {
              width: parent.width
              text: root.service ? root.service.soundPath : ""
              placeholderText: root.tr("soundPath")
              foreground: root.foreground
              onEditingFinished: root.persistSettings({ soundPath: text.trim() })
            }
            Row {
              id: soundActions
              width: parent.width
              spacing: Style.spacing.sm
              Button { width: (soundActions.width - soundActions.spacing * 2) / 3; text: root.tr("chooseFile"); bordered: true; focusable: true; foreground: root.foreground; onClicked: soundDialog.open() }
              Button { width: (soundActions.width - soundActions.spacing * 2) / 3; text: root.tr("testSound"); bordered: true; focusable: true; foreground: root.foreground; enabled: root.service && root.service.soundPath !== ""; onClicked: root.service.playSound() }
              Button { width: (soundActions.width - soundActions.spacing * 2) / 3; text: root.tr("stopSound"); bordered: true; focusable: true; foreground: root.foreground; onClicked: root.service.stopSound() }
            }
            Row {
              width: parent.width
              spacing: Style.spacing.md
              Text {
                width: parent.width - volumeField.width - parent.spacing
                anchors.verticalCenter: parent.verticalCenter
                textFormat: Text.PlainText
                text: root.tr("volume")
                color: Qt.darker(root.foreground, 1.4)
                font.family: root.uiFont
                font.pixelSize: Style.font.bodySmall
              }
              NumberField {
                id: volumeField
                anchors.verticalCenter: parent.verticalCenter
                label: ""
                value: root.service ? root.service.soundVolume : 80
                from: 0
                to: 100
                stepSize: 5
                foreground: root.foreground
                fontFamily: root.uiFont
                onModified: function(value) { root.persistSettings({ soundVolume: value }) }
              }
            }
            Text {
              visible: root.service && root.service.soundError !== ""
              width: parent.width
              textFormat: Text.PlainText
              text: root.service ? root.service.soundError : ""
              color: Color.urgent
              font.family: root.uiFont
              font.pixelSize: Style.font.bodySmall
            }
          }

        }
      }
    }
  }
}
