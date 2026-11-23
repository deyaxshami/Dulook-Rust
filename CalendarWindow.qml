import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import Quickshell
import Quickshell.Hyprland
import qs.Commons
import qs.Commons as Commons
import qs.Ui

FloatingWindow {
  id: root

  required property var service
  property bool opened: false
  property string openingTitle: ""
  property var openingScreen: null
  property string basis: "gregorian"
  property bool yearView: false
  property var selections: ({ gregorian: null, hijri: null })
  property var catalog: null
  property var rows: []
  property real pendingRequest: 0
  property real requestRevision: 0
  property bool loading: false
  property string error: ""

  readonly property bool rtl: service.language === "ar"
  readonly property string uiFont: rtl ? service.arabicUiFont : Style.font.family
  readonly property var monthLocale: Qt.locale(rtl ? "ar_EG" : "en_US")
  readonly property var period: selections[basis]
  readonly property bool hasHijri: !!catalog && catalog.hijriYears.length > 0
  readonly property var selectedYear: catalog && period ? coverageYears(catalog, basis).find(entry => entry.year === period.year) : null
  readonly property color foreground: Commons.Color.popups.text
  readonly property color quietText: Qt.tint(foreground, Qt.alpha(color, 0.27))
  readonly property int dateWidth: Style.space(120)
  readonly property int timeWidth: Style.space(100)
  readonly property int rowHeight: Style.space(32)
  readonly property int headingHeight: Style.space(48)
  readonly property string missing: "—"
  readonly property int windowMargin: Style.space(24)

  function fitToScreen(preferred, extent) {
    return Math.max(1, Math.min(preferred, extent - 2 * windowMargin))
  }

  title: tr("appName") + " — " + tr("calendar")
  implicitWidth: fitToScreen(2 * dateWidth + 8 * timeWidth + Style.space(80), openingScreen?.width ?? Infinity)
  implicitHeight: fitToScreen(Style.space(700), openingScreen?.height ?? Infinity)
  minimumSize: Qt.size(Math.min(600, implicitWidth), Math.min(360, implicitHeight))
  color: Commons.Color.popups.background
  visible: opened
  onClosed: opened = false
  onOpenedChanged: {
    if (!opened) {
      clearResult()
      loading = false
    }
  }

  function tr(key) { return service.tr(key) }

  function showBrowser() {
    if (opened) return
    basis = "gregorian"
    yearView = false
    selections = { gregorian: null, hijri: null }
    const monitor = Hyprland.focusedMonitor
    openingScreen = Quickshell.screens.find(output => output.name === monitor?.name) ?? screen
    openingTitle = title
    opened = true
    resetHorizontal()
    scheduleOpen()
  }

  // A Quickshell FloatingWindow is an xdg toplevel, so Hyprland can tile it
  // unless asked to float. Place our mapped window, never the active window.
  Connections {
    target: Hyprland
    enabled: root.opened
    function onRawEvent(event) {
      if (event.name !== "openwindow") return
      const parts = event.parse(4)
      if (parts[2] !== Quickshell.appId || (parts[3] !== root.title && parts[3] !== root.openingTitle)
          || !/^[0-9a-f]+$/i.test(parts[0])) return

      const selector = "address:0x" + parts[0]
      const width = root.implicitWidth
      const height = root.implicitHeight
      if (Hyprland.usingLua) {
        Hyprland.dispatch(`(function()
          local window = hl.get_window("${selector}")
          if not window or window.pid ~= ${Quickshell.processId} then return function() end end
          hl.dispatch(hl.dsp.window.float({ action = "on", window = window }))
          hl.dispatch(hl.dsp.window.resize({ x = ${width}, y = ${height}, relative = false, window = window }))
          return hl.dsp.window.center({ window = window })
        end)()`)
      } else {
        Hyprland.dispatch("setfloating " + selector)
        Hyprland.dispatch(`resizewindowpixel exact ${width} ${height},${selector}`)
        const screen = root.openingScreen
        if (screen) {
          const x = Math.round(screen.x + (screen.width - width) / 2)
          const y = Math.round(screen.y + (screen.height - height) / 2)
          Hyprland.dispatch(`movewindowpixel exact ${x} ${y},${selector}`)
        }
      }
    }
  }

  function clearResult() {
    catalog = null
    rows = []
    pendingRequest = 0
    error = ""
  }

  function scheduleOpen() {
    if (!opened) return
    clearResult()
    loading = true
    Qt.callLater(sendOpen)
  }

  function sendOpen() {
    if (!opened || !service.calendarRevision || pendingRequest) return
    requestRevision = service.calendarRevision
    pendingRequest = service.requestCalendarBrowser({ view: "open" })
  }

  function coverageYears(reply, system) {
    return system === "hijri" ? reply.hijriYears : reply.gregorianYears
  }

  function datePeriod(date) {
    return date ? { year: Number(date.slice(0, 4)), month: Number(date.slice(5, 7)) } : null
  }

  function keepPeriod(system, fallback, reply) {
    const current = selections[system]
    if (current && (current.year === fallback?.year || coverageYears(reply, system).some(entry => entry.year === current.year)))
      return current
    return fallback
  }

  function receive(reply) {
    if (!visible || reply.requestId !== pendingRequest) return
    pendingRequest = 0
    if (reply.error === "calendarBrowserReload") {
      // Invalidation increments the revision. If its state line is still in flight,
      // wait for it instead of opening against the old revision first.
      clearResult()
      loading = true
      if (service.calendarRevision !== requestRevision) scheduleOpen()
      return
    }
    loading = false
    if (reply.error) {
      error = reply.error
      return
    }
    if (reply.view === "open") {
      selections = {
        gregorian: keepPeriod("gregorian", datePeriod(reply.todayGregorian), reply),
        hijri: keepPeriod("hijri", datePeriod(reply.defaultHijri), reply)
      }
      if (!selections.hijri) basis = "gregorian"
      catalog = reply
      refreshPeriod()
    } else {
      rows = reply.rows
    }
  }

  function refreshPeriod() {
    table.contentY = 0
    overview.contentY = 0
    rows = []
    error = ""
    pendingRequest = 0
    loading = false
    if (!catalog || !period || yearView) return
    loading = true
    requestRevision = service.calendarRevision
    pendingRequest = service.requestCalendarBrowser({ view: "month", basis: basis, year: period.year, month: period.month })
  }

  function selectPeriod(year, month) {
    selections = Object.assign({}, selections, { [basis]: { year: year, month: month } })
    refreshPeriod()
  }

  function resetHorizontal() {
    table.contentX = rtl ? Math.max(0, table.contentWidth - table.width) : 0
  }

  function digits(text) {
    const value = String(text)
    return rtl ? value.replace(/[0-9]/g, n => "٠١٢٣٤٥٦٧٨٩"[Number(n)]) : value
  }

  function numeric(text) {
    return text ? "\u202a" + digits(text) + "\u202c" : missing
  }

  function dateText(iso) {
    return iso.slice(8, 10) + "/" + iso.slice(5, 7) + "/" + iso.slice(0, 4)
  }

  function otherRange(month) {
    if (!month || !month.otherStart) return missing
    const start = dateText(month.otherStart)
    return numeric(month.otherStart === month.otherEnd ? start : start + " – " + dateText(month.otherEnd))
  }

  function monthName(month) {
    return basis === "hijri" ? tr("hijriMonth" + month) : monthLocale.standaloneMonthName(month - 1, Locale.LongFormat)
  }

  function yearOptions() {
    if (!catalog) return []
    const years = coverageYears(catalog, basis).map(entry => entry.year)
    if (basis === "gregorian") {
      const current = datePeriod(catalog.todayGregorian).year
      if (!years.includes(current)) {
        years.push(current)
        years.sort((a, b) => a - b)
      }
    }
    return years.map(year => ({ value: year, label: digits(year) }))
  }

  function headings() {
    const dates = [tr("calendarGregorian"), tr("calendarHijri")]
    if (basis === "hijri") dates.reverse()
    return dates.concat(["Fajr", "Sunrise", "Dhuhr", "Asr", "Yellowing", "Maghrib", "Isha", "calendarIslamicMidnight"].map(key => tr(key)))
  }

  function dayCells(day) {
    const dates = [day.gregorianDate, day.hijriDate]
    if (basis === "hijri") dates.reverse()
    return dates.map(date => numeric(date ? dateText(date) : null)).concat(day.times.map(time => numeric(time)))
  }

  Connections {
    target: root.service
    function onCalendarRevisionChanged() { root.scheduleOpen() }
    function onCalendarBrowserReply(reply) { root.receive(reply) }
  }

  Shortcut {
    sequences: [StandardKey.Close]
    onActivated: root.opened = false
  }

  component Label: Text {
    textFormat: Text.PlainText
    color: root.foreground
    font.family: root.uiFont
    font.pixelSize: Style.font.body
  }

  component TableLine: Row {
    id: line
    required property var cells
    property bool heading: false
    Repeater {
      model: line.cells
      delegate: Label {
        required property int index
        required property string modelData
        width: index < 2 ? root.dateWidth : root.timeWidth
        height: line.height
        text: modelData
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        wrapMode: line.heading ? Text.WordWrap : Text.NoWrap
        font.bold: line.heading
        color: text === root.missing ? root.quietText : root.foreground
      }
    }
  }

  ColumnLayout {
    anchors.fill: parent
    anchors.margins: Style.space(20)
    spacing: Style.spacing.xxl
    LayoutMirroring.enabled: root.rtl
    LayoutMirroring.childrenInherit: true

    Label {
      text: root.tr("calendar")
      font.pixelSize: Style.space(22)
      font.bold: true
    }

    RowLayout {
      Layout.fillWidth: true
      spacing: Style.spacing.lg

      Dropdown {
        id: basisPicker
        Layout.fillWidth: true
        Layout.preferredWidth: Style.space(144)
        label: root.tr("calendar")
        value: root.basis
        options: root.hasHijri ? [
          { value: "gregorian", label: root.tr("calendarGregorian") },
          { value: "hijri", label: root.tr("calendarHijri") }
        ] : [{ value: "gregorian", label: root.tr("calendarGregorian") }]
        enabled: !!root.catalog
        opacity: enabled ? 1 : 0.5
        fontFamily: root.uiFont
        onChanged: function(value) {
          if (root.basis !== value) {
            root.basis = value
            root.resetHorizontal()
            root.refreshPeriod()
          }
          basisPicker.value = Qt.binding(() => root.basis)
        }
      }

      Dropdown {
        id: yearPicker
        Layout.fillWidth: true
        Layout.preferredWidth: Style.space(110)
        label: root.tr("calendarYear")
        value: root.period ? String(root.period.year) : ""
        options: root.yearOptions()
        enabled: !!root.catalog
        opacity: enabled ? 1 : 0.5
        fontFamily: root.uiFont
        onChanged: function(value) {
          root.selectPeriod(Number(value), root.period.month)
          yearPicker.value = Qt.binding(() => root.period ? String(root.period.year) : "")
        }
      }

      Dropdown {
        id: monthPicker
        Layout.fillWidth: true
        Layout.preferredWidth: Style.space(150)
        visible: !root.yearView
        label: root.tr("calendarMonth")
        value: root.period ? String(root.period.month) : ""
        options: root.catalog ? Array.from({ length: 12 }, (_, i) => ({ value: i + 1, label: root.monthName(i + 1) })) : []
        enabled: !!root.catalog
        opacity: enabled ? 1 : 0.5
        fontFamily: root.uiFont
        onChanged: function(value) {
          root.selectPeriod(root.period.year, Number(value))
          monthPicker.value = Qt.binding(() => root.period ? String(root.period.month) : "")
        }
      }

      Button {
        Layout.alignment: Qt.AlignBottom
        Layout.preferredHeight: Style.spacing.controlHeight
        text: root.tr(root.yearView ? "calendarMonth" : "calendarYearOverview")
        foreground: root.foreground
        fontFamily: root.uiFont
        bordered: true
        focusable: true
        enabled: !!root.catalog
        opacity: enabled ? 1 : 0.5
        onClicked: {
          root.yearView = !root.yearView
          root.refreshPeriod()
        }
      }
    }

    Label {
      Layout.fillWidth: true
      visible: !!text
      text: {
        if (!root.catalog) return ""
        const messages = []
        if (root.catalog.warning) messages.push(root.tr(root.catalog.warning))
        if (!root.hasHijri) messages.push(root.tr("calendarBrowserNoHijri"))
        return messages.join("\n")
      }
      wrapMode: Text.WordWrap
      color: root.quietText
    }

    BorderSurface {
      Layout.fillWidth: true
      Layout.fillHeight: true
      color: root.color
      radius: Style.cornerRadius
      borderSpec: Border.controlSpec("normal", root.foreground, Commons.Color.accent)

      Flickable {
        id: table
        anchors.fill: parent
        anchors.margins: Style.spacing.hairline
        visible: !root.yearView && !root.loading && !root.error && root.rows.length > 0
        contentWidth: Math.max(width, 2 * root.dateWidth + 8 * root.timeWidth)
        contentHeight: root.headingHeight + days.height
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        flickableDirection: Flickable.AutoFlickIfNeeded
        Controls.ScrollBar.horizontal: Controls.ScrollBar {
          policy: size < 1 ? Controls.ScrollBar.AlwaysOn : Controls.ScrollBar.AlwaysOff
          palette.mid: root.quietText
        }
        Controls.ScrollBar.vertical: Controls.ScrollBar {
          policy: size < 1 ? Controls.ScrollBar.AlwaysOn : Controls.ScrollBar.AlwaysOff
          palette.mid: root.quietText
        }
        onVisibleChanged: if (visible) root.resetHorizontal()
        onWidthChanged: if (root.rtl) root.resetHorizontal()

        Rectangle {
          y: table.contentY
          z: 1
          width: table.contentWidth
          height: root.headingHeight
          color: Qt.tint(root.color, Qt.alpha(Commons.Color.accent, 0.12))
          TableLine {
            anchors.fill: parent
            heading: true
            cells: root.headings()
          }
        }

        Column {
          id: days
          y: root.headingHeight
          width: table.contentWidth

          Repeater {
            model: root.rows
            delegate: Rectangle {
              required property int index
              required property var modelData
              width: days.width
              height: root.rowHeight
              color: index % 2 ? Qt.tint(root.color, Qt.alpha(root.foreground, 0.05)) : root.color
              TableLine {
                anchors.fill: parent
                cells: root.dayCells(parent.modelData)
              }
            }
          }
        }
      }

      ColumnLayout {
        anchors.fill: parent
        anchors.margins: Style.spacing.hairline
        visible: root.yearView && !!root.catalog && !root.loading && !root.error
        spacing: 0

        Rectangle {
          Layout.fillWidth: true
          Layout.preferredHeight: root.headingHeight
          color: Qt.tint(root.color, Qt.alpha(Commons.Color.accent, 0.12))
          Label {
            anchors.left: parent.left
            anchors.leftMargin: Style.spacing.controlPaddingX + Style.spacing.hairline
            anchors.verticalCenter: parent.verticalCenter
            text: root.tr("calendarMonth")
            font.bold: true
          }
          Label {
            anchors.right: parent.right
            anchors.rightMargin: Style.spacing.controlPaddingX + Style.spacing.hairline
            anchors.verticalCenter: parent.verticalCenter
            text: root.tr("calendarOtherDateRange")
            font.bold: true
          }
        }

        Flickable {
          id: overview
          Layout.fillWidth: true
          Layout.fillHeight: true
          contentWidth: width
          contentHeight: months.height
          clip: true
          boundsBehavior: Flickable.StopAtBounds
          flickableDirection: Flickable.VerticalFlick
          Controls.ScrollBar.vertical: Controls.ScrollBar {
            policy: size < 1 ? Controls.ScrollBar.AlwaysOn : Controls.ScrollBar.AlwaysOff
            palette.mid: root.quietText
          }

          Column {
            id: months
            width: overview.width
            Repeater {
              model: 12
              delegate: Button {
                required property int index
                readonly property var coverage: root.selectedYear ? root.selectedYear.months[index] : null
                width: months.width
                height: root.rowHeight + Style.spacing.md
                enabled: !!coverage
                opacity: enabled ? 1 : 0.5
                focusable: enabled
                leftAlign: true
                fontFamily: root.uiFont
                foreground: root.foreground
                text: root.monthName(index + 1)
                selected: enabled && !!root.period && root.period.month === index + 1
                background: index % 2 ? Qt.tint(root.color, Qt.alpha(root.foreground, 0.05)) : root.color
                onClicked: {
                  root.yearView = false
                  root.selectPeriod(root.period.year, index + 1)
                }
                onActiveFocusChanged: {
                  if (!activeFocus) return
                  if (y < overview.contentY) overview.contentY = y
                  else if (y + height > overview.contentY + overview.height)
                    overview.contentY = y + height - overview.height
                }
                Label {
                  anchors.right: parent.right
                  anchors.rightMargin: Style.spacing.controlPaddingX + Style.spacing.hairline
                  anchors.verticalCenter: parent.verticalCenter
                  text: root.otherRange(parent.coverage)
                }
              }
            }
          }
        }
      }

      Label {
        anchors.centerIn: parent
        width: parent.width - 2 * Style.spacing.xxl
        horizontalAlignment: Text.AlignHCenter
        wrapMode: Text.WordWrap
        visible: root.loading || !!root.error || (!root.yearView && root.rows.length === 0)
        text: root.tr(root.loading ? "calendarBrowserLoading" : (root.error || "calendarBrowserNoRows"))
      }
    }

    Label {
      Layout.fillWidth: true
      text: root.tr("setupHint")
      color: root.quietText
      wrapMode: Text.WordWrap
    }
  }
}
