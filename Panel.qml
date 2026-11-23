import QtQuick
import QtQuick.Dialogs
import qs.Commons
import qs.Ui

Panel {
  id: root
  moduleName: "deyaxshami.dulook"
  manageIpc: false

  property var anchorItem: null
  property var hostWidget: null
  property var service: null
  property bool settingsView: false
  property bool customSoundSelected: false

  readonly property var barIdentity: hostWidget || root
  readonly property bool rtl: service ? service.language === "ar" : false
  readonly property bool hadithRtl: service ? service.hadithLanguage === "ar" : false
  readonly property color foreground: bar ? bar.barForeground : Color.foreground
  readonly property string baseFont: bar ? bar.fontFamily : Style.font.family
  readonly property string uiFont: rtl && service ? service.arabicUiFont : baseFont
  readonly property string hadithFont: hadithRtl && service ? service.arabicHadithFont : baseFont
  readonly property var hadith: service ? service.currentHadith : null
  readonly property bool customSound: customSoundSelected || (service ? service.soundChoice === "custom" : false)
  readonly property bool customCalendar: service ? service.location === "custom" : false
  // Online services only find a place and download its calendar: their fields are drafts
  // until "Find & download", which installs the calendar and selects it.
  property string draftCountry: ""
  property string draftCity: ""
  property string draftZip: ""
  property string draftStreet: ""
  property string draftHouseNumber: ""
  property string draftLatitude: ""
  property string draftLongitude: ""
  property string draftName: ""
  // Downloaded calendar folder the online service is editing; empty for a new calendar.
  property string editingFolder: ""
  // Delete asks for a second click.
  property bool deleteArmed: false
  property string suggestField: "city"
  // Chosen service: online-address, online-map, online-coordinates, offline-installed or custom.
  property string sourceChoice: "offline-installed"
  readonly property string draftMode: sourceChoice.indexOf("online-") === 0 ? sourceChoice.substring(7) : ""
  // Generation of the last pin lookup copied into the drafts.
  property real appliedPinGeneration: -1
  // Id of the last install selected; installs finished before the panel loaded are not reapplied.
  property real appliedInstallId: 0

  function tr(key) { return service ? service.tr(key) : key }

  function sourceFromService() {
    return service && service.location === "custom" ? "custom" : "offline-installed"
  }

  // The installed calendar entry of the current location, or null.
  function selectedInstalled() {
    var list = service ? service.installedCalendars : []
    for (var i = 0; i < list.length; i++)
      if (list[i].value === service.location) return list[i]
    return null
  }

  function sourceOptions() {
    var options = [
      { value: "online-address", label: tr("sourceOnlineAddress") },
      { value: "online-map", label: tr("sourceOnlineMap") },
      { value: "online-coordinates", label: tr("sourceOnlineCoordinates") }
    ]
    if (service && service.installedCalendars.length > 0)
      options.push({ value: "offline-installed", label: tr("sourceOfflineInstalled") })
    options.push({ value: "custom", label: tr("customCalendar") })
    return options
  }

  // The installed calendar to show: the current one, else nordhorn, else the first.
  function installedChoice() {
    var list = service ? service.installedCalendars : []
    var fallback = list.length > 0 ? list[0].value : "nordhorn"
    for (var i = 0; i < list.length; i++) {
      if (list[i].value === service.location) return service.location
      if (list[i].value === "nordhorn") fallback = "nordhorn"
    }
    return fallback
  }

  function clearLocationDraft() {
    draftCountry = ""
    draftCity = ""
    draftZip = ""
    draftStreet = ""
    draftHouseNumber = ""
    draftLatitude = ""
    draftLongitude = ""
    draftName = ""
  }

  function resetLocationDraft() {
    sourceChoice = sourceFromService()
    editingFolder = ""
    deleteArmed = false
    clearLocationDraft()
  }

  function chooseSource(value) {
    suggestTimer.stop()
    if (service) service.clearPlaceSuggestions()
    sourceChoice = value
    editingFolder = ""
    deleteArmed = false
    if (value === "custom") {
      // Without a file the switch waits for the dialog: cancelling it must not leave an empty custom calendar.
      if (service && service.calendarPath === "") calendarDialog.open()
      else persistSettings({ location: "custom" })
    } else if (value === "offline-installed") {
      persistSettings({ location: installedChoice() })
    } else {
      clearLocationDraft()
      if (draftMode === "map") Qt.callLater(centerMap)
    }
  }

  // Opens the downloaded calendar's online service with its stored place filled in.
  function editInstalled() {
    var entry = selectedInstalled()
    if (!entry || !entry.downloaded) return
    draftCountry = entry.country
    draftCity = entry.city
    draftZip = entry.zip
    draftStreet = entry.street
    draftHouseNumber = entry.houseNumber
    draftLatitude = entry.latitude
    draftLongitude = entry.longitude
    draftName = entry.name
    editingFolder = entry.folder
    deleteArmed = false
    sourceChoice = "online-" + entry.mode
    if (entry.mode === "map") Qt.callLater(centerMap)
  }

  // Selects another installed calendar first, so the deleted one is never shown as missing.
  function deleteInstalled() {
    var entry = selectedInstalled()
    if (!entry || !entry.downloaded) return
    var list = service.installedCalendars
    var fallback = ""
    for (var i = 0; i < list.length; i++) {
      if (list[i].value === entry.value) continue
      if (fallback === "" || list[i].value === "nordhorn") fallback = list[i].value
    }
    persistSettings({ location: fallback !== "" ? fallback : "nordhorn" })
    service.deleteCalendar(entry.folder)
    deleteArmed = false
  }

  // Entries saved while online services were a calendar source keep the download the backend
  // migrated them to; until the backend's first state `service.location` is only a default.
  function migrateLegacyLocation() {
    if (service && service.backendState.location !== undefined && settings && settings.location === "online"
        && service.location !== "online")
      persistSettings({ location: service.location })
  }

  // Typed changes drop coordinates picked from a suggestion and ask for new suggestions.
  function editLocationDraft() {
    draftLatitude = ""
    draftLongitude = ""
    suggestFor("city")
  }

  // Suggestions follow the field being typed in: "city" (also country/ZIP), "street" or "house".
  function suggestFor(field) {
    suggestField = field
    suggestTimer.restart()
  }

  function pickSuggestion(place) {
    suggestTimer.stop()
    if (place.street !== "") {
      draftStreet = place.street
      draftHouseNumber = place.houseNumber
    }
    if (place.city !== "") draftCity = place.city
    if (place.zip !== "") draftZip = place.zip
    if (draftCountry === "") draftCountry = place.country
    draftLatitude = String(place.latitude)
    draftLongitude = String(place.longitude)
    if (!service) return
    service.clearPlaceSuggestions()
    // A picked street lists its house numbers right away.
    if (place.street !== "" && place.houseNumber === "") suggestFor("house")
  }

  // A click on the map: its coordinates are kept and the place's fields are filled in when
  // the reverse lookup returns (onPinnedPlaceChanged).
  function setPin(latitude, longitude) {
    draftLatitude = latitude.toFixed(6)
    draftLongitude = longitude.toFixed(6)
    suggestTimer.stop()
    if (!service) return
    service.clearPlaceSuggestions()
    service.pinPlace(latitude, longitude)
  }

  function centerMap() {
    var entry = selectedInstalled()
    if (draftLatitude !== "" && draftLongitude !== "") {
      osmMap.centerLatitude = Number(draftLatitude)
      osmMap.centerLongitude = Number(draftLongitude)
      osmMap.zoom = 17
    } else if (entry && entry.downloaded) {
      osmMap.centerLatitude = Number(entry.latitude)
      osmMap.centerLongitude = Number(entry.longitude)
      osmMap.zoom = 13
    } else {
      osmMap.centerLatitude = 20
      osmMap.centerLongitude = 0
      osmMap.zoom = 2
    }
  }

  function validCoordinate(text, limit) {
    var t = String(text).trim()
    var v = Number(t)
    return t !== "" && isFinite(v) && Math.abs(v) <= limit
  }

  function canSave() {
    if (draftMode === "address") return draftCountry !== "" && draftCity.trim() !== ""
    return validCoordinate(draftLatitude, 90) && validCoordinate(draftLongitude, 180)
  }

  function placeSummary() {
    return [(draftStreet + " " + draftHouseNumber).trim(), (draftZip + " " + draftCity).trim(), draftCountry]
      .filter(function(part) { return part !== "" }).join(", ")
  }

  // The backend applies each service's rules (house number needs a street, a street drops
  // suggestion coordinates, typed coordinates ignore the address).
  function saveLocationDraft() {
    suggestTimer.stop()
    if (!service) return
    service.installCalendar({ mode: draftMode, country: draftCountry, city: draftCity, zip: draftZip, street: draftStreet,
      houseNumber: draftHouseNumber, latitude: draftLatitude, longitude: draftLongitude,
      name: draftName, replace: editingFolder })
    service.clearPlaceSuggestions()
  }

  // Dropdown assigns `value` imperatively on selection, which drops its binding.
  function soundChoiceBinding() {
    return Qt.binding(function() {
      return root.customSound ? "custom" : (root.service ? root.service.soundChoice : "adans/adan_04.mp3")
    })
  }

  function open() {
    controller.show()
    if (service) service.refreshClock()
    resetLocationDraft()
    Qt.callLater(function() { if (root.opened) root.setCenterHoverRevealSuppressed(true) })
  }

  function close() {
    setCenterHoverRevealSuppressed(false)
    customSoundSelected = false
    deleteArmed = false
    soundChoiceDropdown.value = soundChoiceBinding()
    controller.hide()
  }

  function switchPanel(direction) {
    if (bar && typeof bar.switchPanelFrom === "function") return bar.switchPanelFrom(barIdentity, direction)
    return false
  }

  function setCenterHoverRevealSuppressed(value) {
    if (root.bar && typeof root.bar.setCenterHoverRevealSuppressed === "function")
      root.bar.setCenterHoverRevealSuppressed(value)
    else if (root.bar && "centerHoverRevealSuppressed" in root.bar)
      root.bar.centerHoverRevealSuppressed = value
  }

  function persistSettings(values) {
    var entry = { id: moduleName }
    // Location fields of the old online calendar source are dropped from the entry.
    var legacy = ["locationCountry", "locationCity", "locationZip", "locationStreet", "locationHouseNumber",
      "locationLatitude", "locationLongitude", "locationMode"]
    for (var key in settings) if (key !== "id" && legacy.indexOf(key) === -1) entry[key] = settings[key]
    for (var changed in values) entry[changed] = values[changed]
    settings = entry
    if (hostWidget) hostWidget.settings = entry
    if (bar && bar.shell && typeof bar.shell.updateEntryInline === "function")
      bar.shell.updateEntryInline(moduleName, entry)
    // The shell hands plugin services the bar config one change late, so push the new entry directly.
    if (service) service.applySettings(entry)
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
    onAccepted: root.persistSettings({ location: "custom", calendarPath: root.pathFromUrl(selectedFile) })
  }

  FileDialog {
    id: soundDialog
    title: root.tr("chooseFile")
    nameFilters: [root.tr("mp3Files") + " (*.mp3)", root.tr("allFiles") + " (*)"]
    onAccepted: root.persistSettings({ soundPath: root.pathFromUrl(selectedFile) })
  }

  onCustomSoundChanged: soundChoiceDropdown.value = soundChoiceBinding()
  onServiceChanged: {
    appliedInstallId = service && service.lastInstalled ? service.lastInstalled.id : 0
    migrateLegacyLocation()
  }
  onSettingsChanged: migrateLegacyLocation()

  Connections {
    target: root.service
    ignoreUnknownSignals: true
    function onSoundChoiceChanged() { soundChoiceDropdown.value = root.soundChoiceBinding() }
    function onLocationChanged() {
      root.migrateLegacyLocation()
      root.resetLocationDraft()
    }
    // Selects a finished install once; backendState objects change on every state line.
    function onLastInstalledChanged() {
      var installed = root.service.lastInstalled
      if (!installed || installed.id <= root.appliedInstallId) return
      root.appliedInstallId = installed.id
      root.persistSettings({ location: installed.value })
      root.resetLocationDraft()
    }
    // Fills the place's fields once per lookup; backendState objects change on every state line.
    function onPinnedPlaceChanged() {
      var result = root.service.pinnedPlace
      if (!result || result.generation <= root.appliedPinGeneration) return
      root.appliedPinGeneration = result.generation
      var place = result.place
      if (!place || root.draftMode !== "map") return
      if (place.country !== "") root.draftCountry = place.country
      root.draftCity = place.city
      root.draftZip = place.zip
      root.draftStreet = place.street
      root.draftHouseNumber = place.houseNumber
    }
  }

  Timer {
    id: suggestTimer
    interval: 400
    repeat: false
    onTriggered: if (root.service) root.service.suggestPlaces(root.suggestField, root.draftCountry, root.draftCity,
      root.draftZip, root.draftStreet, root.draftHouseNumber)
  }

  KeyboardPanel {
    id: popup
    anchorItem: root.anchorItem
    owner: root.barIdentity
    bar: root.bar
    open: root.opened
    centerOnBar: true
    focusTarget: keyCatcher
    contentWidth: popup.fittedContentWidth(Style.space(660))
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
              width: parent.width - calendarButton.width - settingsButton.width - 2 * parent.spacing
              text: root.tr("appName")
              color: root.foreground
              font.family: root.uiFont
              font.pixelSize: Style.font.title
              font.bold: true
            }
            PanelActionButton {
              id: calendarButton
              iconText: "▦"
              tooltipText: root.tr("calendar")
              foreground: root.foreground
              focusable: true
              enabled: !!root.service
              onClicked: {
                root.service.openCalendarBrowser()
                root.close()
              }
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
                text: root.service && root.service.calendarReady
                  ? (root.service.nextPrayer && root.service.nextPrayer.isPrayer === false ? root.tr("nextTime") : root.tr("nextPrayer"))
                  : root.tr("setup")
                color: Qt.darker(root.foreground, 1.35)
                font.family: root.uiFont
                font.pixelSize: Style.font.caption
                font.bold: true
              }
              Text {
                textFormat: Text.PlainText
                anchors.horizontalCenter: parent.horizontalCenter
                text: root.service && root.service.nextPrayer
                  ? root.service.nextPrayer.label
                  : (root.service && (!root.customCalendar || root.service.calendarPath !== "") ? root.tr("noPrayerData") : root.tr("noCalendar"))
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
                text: root.service ? root.tr("systemTimezone") + " · " + root.service.timezoneLabel : ""
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
              id: prayerTiles
              model: root.service ? root.service.prayerRows : []
              BorderSurface {
                required property var modelData
                readonly property bool selectedPrayer: root.service && root.service.nextPrayer
                  && modelData.name === root.service.nextPrayer.name
                  && modelData.prayerDate === root.service.nextPrayer.prayerDate
                width: (prayerRow.width - prayerRow.spacing * (prayerTiles.count - 1)) / Math.max(prayerTiles.count, 1)
                height: Style.space(68)
                radius: Style.cornerRadius
                color: selectedPrayer ? Style.selectedFillFor(root.foreground, Color.accent)
                  : Style.normalFillFor(root.foreground, Color.accent)
                borderSpec: Border.controlSpec(selectedPrayer ? "selected" : "normal", root.foreground, Color.accent)
                Column {
                  anchors.centerIn: parent
                  width: parent.width - Style.spacing.xs * 2
                  spacing: Style.spacing.xs
                  Text {
                    textFormat: Text.PlainText
                    width: parent.width
                    horizontalAlignment: Text.AlignHCenter
                    fontSizeMode: Text.HorizontalFit
                    minimumPixelSize: 8
                    text: modelData.nameLabel
                    color: selectedPrayer ? Color.accent : Qt.darker(root.foreground, 1.35)
                    font.family: root.uiFont
                    font.pixelSize: Style.font.caption
                    font.bold: selectedPrayer
                  }
                  Text {
                    textFormat: Text.PlainText
                    width: parent.width
                    horizontalAlignment: Text.AlignHCenter
                    fontSizeMode: Text.HorizontalFit
                    minimumPixelSize: 8
                    text: modelData.timeLabel
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
                text: root.hadith ? (root.hadithRtl ? root.hadith.arabic : root.hadith.english) : root.tr("loadingHadith")
                wrapMode: Text.WordWrap
                color: root.foreground
                font.family: root.hadithFont
                font.pixelSize: Style.font.title
                lineHeight: root.hadithRtl ? 1.55 : 1.35
                horizontalAlignment: root.hadithRtl ? Text.AlignRight : Text.AlignLeft
              }
              Row {
                width: parent.width
                spacing: Style.spacing.sm
                Text {
                  textFormat: Text.PlainText
                  width: parent.width - previousButton.width - nextButton.width - parent.spacing * 2
                  anchors.verticalCenter: parent.verticalCenter
                  text: root.hadith ? root.hadith.referenceLabel : ""
                  color: Qt.darker(root.foreground, 1.35)
                  font.family: root.hadithFont
                  font.pixelSize: Style.font.bodySmall
                  horizontalAlignment: root.hadithRtl ? Text.AlignRight : Text.AlignLeft
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
              label: root.tr("uiLanguage")
              value: root.service ? root.service.language : "en"
              options: [{ value: "en", label: "English" }, { value: "ar", label: "العربية" }]
              foreground: root.foreground
              fontFamily: root.uiFont
              onChanged: function(value) { root.persistSettings({ language: value }) }
            }
            Dropdown {
              width: parent.width
              label: root.tr("hadithLanguage")
              value: root.service ? root.service.hadithLanguage : "en"
              options: [{ value: "en", label: "English" }, { value: "ar", label: "العربية" }]
              foreground: root.foreground
              fontFamily: root.uiFont
              onChanged: function(value) { root.persistSettings({ hadithLanguage: value }) }
            }
          }

          SettingsGroup {
            width: parent.width
            foreground: root.foreground
            PanelSectionHeader { text: root.tr("calendar"); foreground: root.foreground; fontFamily: root.uiFont }
            Dropdown {
              id: sourceDropdown
              width: parent.width
              label: root.tr("location")
              value: root.sourceChoice
              options: root.sourceOptions()
              foreground: root.foreground
              fontFamily: root.uiFont
              onChanged: function(value) {
                // Dropdown assigns `value` imperatively before emitting; keep following the choice.
                sourceDropdown.value = Qt.binding(function() { return root.sourceChoice })
                root.chooseSource(value)
              }
            }
            Dropdown {
              id: installedDropdown
              visible: root.sourceChoice === "offline-installed"
              width: parent.width
              label: root.tr("installedCalendar")
              value: root.service ? root.service.location : "nordhorn"
              options: root.service ? root.service.installedCalendars : []
              foreground: root.foreground
              fontFamily: root.uiFont
              onChanged: function(value) {
                installedDropdown.value = Qt.binding(function() { return root.service ? root.service.location : "nordhorn" })
                root.persistSettings({ location: value })
              }
            }
            Column {
              id: installedManage
              // Bundled calendars are read-only; downloads can be named, edited and deleted.
              readonly property var entry: {
                var selected = root.selectedInstalled()
                return selected && selected.downloaded === true ? selected : null
              }
              visible: root.sourceChoice === "offline-installed" && entry !== null
              width: parent.width
              spacing: Style.spacing.sm
              Text {
                width: parent.width
                textFormat: Text.PlainText
                text: installedManage.entry ? root.tr("calendarFolder") + " " + installedManage.entry.folder : ""
                color: Qt.darker(root.foreground, 1.4)
                font.family: root.uiFont
                font.pixelSize: Style.font.bodySmall
                wrapMode: Text.WordWrap
              }
              TextField {
                width: parent.width
                text: installedManage.entry ? installedManage.entry.name : ""
                placeholderText: root.tr("calendarName")
                foreground: root.foreground
                onEditingFinished: {
                  var entry = installedManage.entry
                  if (entry && text.trim() !== entry.name) root.service.renameCalendar(entry.folder, text.trim())
                }
              }
              Row {
                id: installedActions
                width: parent.width
                spacing: Style.spacing.sm
                Button {
                  width: (installedActions.width - installedActions.spacing) / 2
                  text: root.tr("editLocation")
                  bordered: true
                  focusable: true
                  foreground: root.foreground
                  enabled: root.service && !root.service.downloadActive
                  onClicked: root.editInstalled()
                }
                Button {
                  width: (installedActions.width - installedActions.spacing) / 2
                  text: root.deleteArmed ? root.tr("confirmDelete") : root.tr("deleteCalendar")
                  bordered: true
                  focusable: true
                  foreground: root.foreground
                  enabled: root.service && !root.service.downloadActive
                  onClicked: {
                    if (root.deleteArmed) root.deleteInstalled()
                    else root.deleteArmed = true
                  }
                }
              }
            }
            Column {
              visible: root.draftMode !== ""
              width: parent.width
              spacing: Style.spacing.sm
              Column {
                visible: root.draftMode === "address"
                width: parent.width
                spacing: Style.spacing.sm
                // Measures the dropdown's search field; the stock header (row height + padding,
                // minus md margins on both sides) is shorter than the field and clips its text.
                TextField {
                  id: countrySearchProbe
                  visible: false
                  font.family: root.uiFont
                }
                SearchableDropdown {
                  id: countryDropdown
                  width: parent.width
                  label: root.tr("country")
                  value: root.draftCountry
                  options: root.service ? root.service.countries : []
                  foreground: root.foreground
                  fontFamily: root.uiFont
                  // The stock popup caps at about 6 rows; the country list is long.
                  popupMinHeight: Style.spacing.searchablePopupMinHeight * 2
                  popupRowHeight: Math.max(Style.spacing.popupRowHeight,
                    countrySearchProbe.implicitHeight + 2 * Style.spacing.md - Style.spacing.controlPaddingX)
                  onChanged: function(value) {
                    countryDropdown.value = Qt.binding(function() { return root.draftCountry })
                    root.draftCountry = value
                    root.editLocationDraft()
                  }
                }
                TextField {
                  width: parent.width
                  text: root.draftCity
                  placeholderText: root.tr("city")
                  foreground: root.foreground
                  onTextEdited: {
                    root.draftCity = text
                    root.editLocationDraft()
                  }
                }
                TextField {
                  width: parent.width
                  text: root.draftZip
                  placeholderText: root.tr("zipOptional")
                  foreground: root.foreground
                  onTextEdited: {
                    root.draftZip = text
                    root.editLocationDraft()
                  }
                }
                TextField {
                  width: parent.width
                  text: root.draftStreet
                  placeholderText: root.tr("streetOptional")
                  foreground: root.foreground
                  onTextEdited: {
                    root.draftStreet = text
                    root.suggestFor("street")
                  }
                }
                TextField {
                  width: parent.width
                  enabled: root.draftStreet.trim() !== ""
                  text: root.draftHouseNumber
                  placeholderText: root.tr("houseNumberOptional")
                  foreground: root.foreground
                  onTextEdited: {
                    root.draftHouseNumber = text
                    root.suggestFor("house")
                  }
                }
                Column {
                  id: placeSuggestionList
                  width: parent.width
                  spacing: Style.spacing.sm
                  Repeater {
                    model: root.service ? root.service.placeSuggestions : []
                    Button {
                      required property var modelData
                      width: placeSuggestionList.width
                      text: modelData.label
                      bordered: true
                      focusable: true
                      foreground: root.foreground
                      onClicked: root.pickSuggestion(modelData)
                    }
                  }
                }
              }
              Column {
                visible: root.draftMode === "map"
                width: parent.width
                spacing: Style.spacing.sm
                Text {
                  width: parent.width
                  textFormat: Text.PlainText
                  text: root.draftLatitude === "" ? root.tr("pinHint")
                    : root.tr("pinnedAt") + " " + root.draftLatitude + ", " + root.draftLongitude
                      + (root.placeSummary() !== "" ? "\n" + root.placeSummary() : "")
                  color: Qt.darker(root.foreground, 1.4)
                  font.family: root.uiFont
                  font.pixelSize: Style.font.bodySmall
                  wrapMode: Text.WordWrap
                }
                OsmMap {
                  id: osmMap
                  width: parent.width
                  height: Style.space(320)
                  service: root.service
                  foreground: root.foreground
                  fontFamily: root.uiFont
                  pinLatitude: root.draftLatitude !== "" ? Number(root.draftLatitude) : NaN
                  pinLongitude: root.draftLongitude !== "" ? Number(root.draftLongitude) : NaN
                  onPicked: function(latitude, longitude) { root.setPin(latitude, longitude) }
                }
              }
              Column {
                visible: root.draftMode === "coordinates"
                width: parent.width
                spacing: Style.spacing.sm
                TextField {
                  width: parent.width
                  text: root.draftLatitude
                  placeholderText: root.tr("latitude")
                  foreground: root.foreground
                  onTextEdited: root.draftLatitude = text
                }
                TextField {
                  width: parent.width
                  text: root.draftLongitude
                  placeholderText: root.tr("longitude")
                  foreground: root.foreground
                  onTextEdited: root.draftLongitude = text
                }
              }
              Text {
                width: parent.width
                visible: root.editingFolder !== ""
                textFormat: Text.PlainText
                text: root.tr("editingCalendar") + " " + root.editingFolder
                color: Qt.darker(root.foreground, 1.4)
                font.family: root.uiFont
                font.pixelSize: Style.font.bodySmall
                wrapMode: Text.WordWrap
              }
              TextField {
                width: parent.width
                text: root.draftName
                placeholderText: root.tr("calendarName")
                foreground: root.foreground
                onTextEdited: root.draftName = text
              }
              Row {
                id: onlineActions
                width: parent.width
                spacing: Style.spacing.sm
                Button {
                  width: root.editingFolder !== "" ? (onlineActions.width - onlineActions.spacing) / 2 : onlineActions.width
                  text: root.tr("findLocation")
                  bordered: true
                  focusable: true
                  foreground: root.foreground
                  enabled: root.canSave() && root.service && !root.service.downloadActive
                  onClicked: root.saveLocationDraft()
                }
                Button {
                  visible: root.editingFolder !== ""
                  width: (onlineActions.width - onlineActions.spacing) / 2
                  text: root.tr("cancel")
                  bordered: true
                  focusable: true
                  foreground: root.foreground
                  onClicked: root.resetLocationDraft()
                }
              }
              Text {
                width: parent.width
                visible: text !== ""
                textFormat: Text.PlainText
                text: root.service ? root.service.downloadStatus : ""
                color: root.service && root.service.downloadStatus !== "" && !root.service.downloadActive ? Color.urgent : Qt.darker(root.foreground, 1.4)
                font.family: root.uiFont
                font.pixelSize: Style.font.bodySmall
                wrapMode: Text.WordWrap
              }
            }
            TextField {
              visible: root.sourceChoice === "custom"
              width: parent.width
              text: root.service ? root.service.calendarPath : ""
              placeholderText: root.tr("calendarPath")
              foreground: root.foreground
              onEditingFinished: {
                var path = text.trim()
                root.persistSettings(path !== "" ? { location: "custom", calendarPath: path } : { calendarPath: "" })
              }
            }
            Row {
              id: calendarActions
              visible: root.sourceChoice === "custom"
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
              label: root.tr("enablePrayerTimeNotifications")
              description: root.tr("enablePrayerTimeNotificationsHint")
              checked: root.service ? root.service.prayerTimeNotificationsEnabled : true
              foreground: root.foreground
              fontFamily: root.uiFont
              onClicked: root.persistSettings({ prayerTimeNotificationsEnabled: !checked })
            }
            Toggle {
              width: parent.width
              label: root.tr("enablePrayerReminders")
              description: root.tr("enablePrayerRemindersHint")
              checked: root.service ? root.service.prayerRemindersEnabled : true
              foreground: root.foreground
              fontFamily: root.uiFont
              onClicked: root.persistSettings({ prayerRemindersEnabled: !checked })
            }
            Toggle {
              width: parent.width
              label: root.tr("enableLastThirdReminder")
              description: root.tr("enableLastThirdReminderHint")
              checked: root.service ? root.service.lastThirdReminderEnabled : true
              foreground: root.foreground
              fontFamily: root.uiFont
              onClicked: root.persistSettings({ lastThirdReminderEnabled: !checked })
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
            Text {
              width: parent.width
              text: root.tr("prayerTimePerPrayer")
              color: Qt.darker(root.foreground, 1.3)
              font.family: root.uiFont
              font.pixelSize: Style.font.bodySmall
              wrapMode: Text.WordWrap
            }
            Grid {
              id: prayerNotificationGrid
              width: parent.width
              columns: 2
              columnSpacing: Style.spacing.sm
              rowSpacing: Style.spacing.sm
              Repeater {
                model: ["Fajr", "Sunrise", "Dhuhr", "Asr", "Yellowing", "Maghrib", "Isha", "Midnight"]
                Toggle {
                  required property string modelData
                  width: (prayerNotificationGrid.width - prayerNotificationGrid.columnSpacing) / 2
                  label: root.service ? root.service.prayerName(modelData) : modelData
                  checked: root.service ? root.service.notifyEnabled[modelData] !== false : true
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
            Text {
              width: parent.width
              text: root.tr("reminderPerPrayer")
              color: Qt.darker(root.foreground, 1.3)
              font.family: root.uiFont
              font.pixelSize: Style.font.bodySmall
              wrapMode: Text.WordWrap
            }
            Grid {
              id: prayerReminderGrid
              width: parent.width
              columns: 2
              columnSpacing: Style.spacing.sm
              rowSpacing: Style.spacing.sm
              Repeater {
                model: ["Fajr", "Sunrise", "Dhuhr", "Asr", "Yellowing", "Maghrib", "Isha", "Midnight"]
                Toggle {
                  required property string modelData
                  width: (prayerReminderGrid.width - prayerReminderGrid.columnSpacing) / 2
                  label: root.service ? root.service.prayerName(modelData) : modelData
                  checked: root.service ? root.service.remindEnabled[modelData] !== false : true
                  foreground: root.foreground
                  fontFamily: root.uiFont
                  onClicked: {
                    var values = {}
                    values["remind" + modelData] = !checked
                    root.persistSettings(values)
                  }
                }
              }
            }
            Button { width: parent.width; text: root.tr("testNotification"); bordered: true; focusable: true; foreground: root.foreground; onClicked: root.service.testNotification() }
          }

          SettingsGroup {
            width: parent.width
            foreground: root.foreground
            PanelSectionHeader { text: root.tr("sound"); foreground: root.foreground; fontFamily: root.uiFont }
            Toggle {
              width: parent.width
              label: root.tr("enableSound")
              checked: root.service ? root.service.soundEnabled : true
              foreground: root.foreground
              fontFamily: root.uiFont
              onClicked: root.persistSettings({ soundEnabled: !checked })
            }
            Dropdown {
              id: soundChoiceDropdown
              width: parent.width
              label: root.tr("adhanChoice")
              value: root.customSound ? "custom" : (root.service ? root.service.soundChoice : "adans/adan_04.mp3")
              options: (root.service ? root.service.bundledSounds : []).concat([{ value: "custom", label: root.tr("customSound") }])
              foreground: root.foreground
              fontFamily: root.uiFont
              onChanged: function(value) {
                if (value === "custom") {
                  root.customSoundSelected = true
                  soundDialog.open()
                } else {
                  root.customSoundSelected = false
                  root.persistSettings({ soundPath: value })
                }
              }
            }
            TextField {
              visible: root.customSound
              width: parent.width
              text: root.service ? root.service.soundPath : ""
              placeholderText: root.tr("soundPath")
              foreground: root.foreground
              onEditingFinished: root.persistSettings({ soundPath: text.trim() })
            }
            Button { visible: root.customSound; width: parent.width; text: root.tr("chooseFile"); bordered: true; focusable: true; foreground: root.foreground; onClicked: soundDialog.open() }
            Row {
              id: soundActions
              width: parent.width
              spacing: Style.spacing.sm
              Button { width: (soundActions.width - soundActions.spacing) / 2; text: root.tr("testSound"); bordered: true; focusable: true; foreground: root.foreground; enabled: root.service && root.service.soundPath !== ""; onClicked: root.service.testSound() }
              Button { width: (soundActions.width - soundActions.spacing) / 2; text: root.tr("stopSound"); bordered: true; focusable: true; foreground: root.foreground; onClicked: root.service.stopSound() }
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
