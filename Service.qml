import QtQuick
import Quickshell
import Quickshell.Io

// Thin bridge to the Rust backend (bin/dulook). The backend owns all logic:
// calendar, alerts, Adhan audio, Hadith, translations, and formatting.
// It exchanges JSON lines over stdin/stdout.
Item {
  id: root

  property var shell: null
  property string omarchyPath: "/usr/share/omarchy"
  property var manifest: null

  readonly property var settings: {
    var barConfig = shell ? shell.barConfig : null
    if (!barConfig && shell && shell.shellConfig) barConfig = shell.shellConfig.bar
    var layout = barConfig ? barConfig.layout : null
    var sections = ["left", "centerBegin", "center", "centerEnd", "tray", "right"]
    for (var s = 0; layout && s < sections.length; s++) {
      var entries = layout[sections[s]] || []
      for (var i = 0; i < entries.length; i++)
        if (entries[i] && entries[i].id === "deyaxshami.dulook") return entries[i]
    }
    return ({})
  }

  readonly property var notificationService: shell ? shell.firstPartyServiceFor("omarchy.notifications") : null
  readonly property bool dnd: notificationService ? notificationService.doNotDisturb === true : false

  readonly property string pluginDir: {
    var dir = decodeURIComponent(String(Qt.resolvedUrl(".")).replace(/^file:\/\//, ""))
    return dir.charAt(dir.length - 1) === "/" ? dir : dir + "/"
  }
  property bool backendReady: false
  property var backendState: ({})
  property var backendTick: ({})
  signal calendarBrowserReply(var reply)
  property real calendarRevision: 0
  property real calendarRequestId: 0

  readonly property string language: backendState.language === "ar" ? "ar" : "en"
  readonly property string hadithLanguage: backendState.hadithLanguage === "ar" ? "ar" : "en"
  readonly property string location: backendState.location || "nordhorn"
  // Bundled locations (`{ value, label, downloaded: false }`), then finished downloads with
  // `value: "downloaded/<folder>"`, `folder`, `name`, `mode`, and the stored place fields.
  readonly property var installedCalendars: backendState.installedCalendars || []
  readonly property bool downloadActive: backendState.downloadActive === true
  readonly property string downloadStatus: backendState.downloadStatus || ""
  // `{ id, value }` of the latest finished install; the panel selects `value`.
  readonly property var lastInstalled: backendState.lastInstalled || null
  readonly property var placeSuggestions: backendState.placeSuggestions || []
  // `{ generation, place }` of the latest map pin lookup; `place` is null when nothing is there.
  readonly property var pinnedPlace: backendState.pinnedPlace || null
  readonly property string mapTileDir: backendState.mapTileDir || ""
  readonly property var mapTiles: backendState.mapTiles || []
  // ISO 3166-1 countries for the online location picker: static display data read once.
  property var countryList: []
  readonly property var countries: countryList.map(function(country) {
    return { value: country.code, label: language === "ar" ? country.ar : country.en }
  })
  readonly property string calendarPath: backendState.calendarPath || ""
  readonly property bool calendarReady: backendState.calendarReady === true
  readonly property string calendarError: backendState.calendarError || ""
  readonly property string coverage: backendState.coverage || ""
  readonly property string soundError: backendState.soundError || ""
  readonly property bool soundPlaying: backendState.soundPlaying === true
  readonly property bool soundEnabled: backendState.soundEnabled !== false
  readonly property string soundPath: backendState.soundPath || ""
  readonly property int soundVolume: backendState.soundVolume !== undefined ? backendState.soundVolume : 80
  readonly property var bundledSounds: backendState.bundledSounds || []
  readonly property string soundChoice: backendState.soundChoice || "adans/adan_04.mp3"
  readonly property bool bypassDnd: backendState.bypassDnd === true
  readonly property bool prayerTimeNotificationsEnabled: backendState.prayerTimeNotificationsEnabled !== false
  readonly property bool prayerRemindersEnabled: backendState.prayerRemindersEnabled !== false
  readonly property bool lastThirdReminderEnabled: backendState.lastThirdReminderEnabled !== false
  readonly property string arabicUiFont: backendState.arabicUiFont || "Noto Sans Arabic"
  readonly property string arabicHadithFont: backendState.arabicHadithFont || "Noto Naskh Arabic"
  readonly property var notifyEnabled: backendState.notifyEnabled || ({})
  readonly property var remindEnabled: backendState.remindEnabled || ({})
  readonly property var currentHadith: backendState.currentHadith || null
  readonly property int hadithIndex: backendState.hadithIndex !== undefined ? backendState.hadithIndex : -1
  readonly property bool hadithLoading: backendState.hadithLoading === true
  readonly property string hadithSource: backendState.hadithSource || "offline"
  readonly property int fallbackCount: backendState.fallbackCount || 0

  readonly property string countdown: backendTick.countdown || "--:--:--"
  readonly property string barLabel: backendTick.barLabel || "Dulook"
  readonly property string timezoneLabel: backendTick.timezoneLabel || ""
  readonly property var nextPrayer: backendTick.nextPrayer || null
  readonly property var prayerRows: backendTick.prayerRows || []
  readonly property bool alerting: backendTick.alerting === true

  function tr(key) {
    var strings = backendState.strings
    return strings && strings[key] !== undefined ? strings[key] : key
  }

  function prayerName(name) { return tr(name) }

  function send(message) {
    if (backendReady) backend.write(JSON.stringify(message) + "\n")
  }

  function sendSettings() { send({ cmd: "settings", settings: settings }) }
  function applySettings(entry) { send({ cmd: "settings", settings: entry }) }
  function reloadCalendar() { send({ cmd: "reloadCalendar" }) }
  function openCalendarBrowser() { calendarWindow.showBrowser() }
  function requestCalendarBrowser(selection) {
    calendarRequestId += 1
    send({ cmd: "browseCalendar", requestId: calendarRequestId, selection: selection })
    return calendarRequestId
  }
  function refreshClock() { send({ cmd: "refreshClock" }) }
  function nextHadith() { send({ cmd: "nextHadith" }) }
  function previousHadith() { send({ cmd: "previousHadith" }) }
  function showFallback() { send({ cmd: "showFallback" }) }
  function requestHadith() { send({ cmd: "requestHadith" }) }
  function testNotification() { send({ cmd: "testNotification" }) }
  function testSound() { send({ cmd: "testSound" }) }
  function stopSound() { send({ cmd: "stopSound" }) }
  function stopPrayerSound(alertKey) { send({ cmd: "stopPrayerSound", key: String(alertKey) }) }
  function acknowledgeAlert() { send({ cmd: "acknowledgeAlert" }) }
  function suggestPlaces(field, country, city, zip, street, houseNumber) {
    send({ cmd: "suggestPlaces", field: String(field), country: String(country), city: String(city),
      zip: String(zip), street: String(street), houseNumber: String(houseNumber) })
  }
  function clearPlaceSuggestions() { send({ cmd: "clearPlaceSuggestions" }) }
  function pinPlace(latitude, longitude) { send({ cmd: "pinPlace", latitude: Number(latitude), longitude: Number(longitude) }) }
  function requestMapTiles(keys) { send({ cmd: "mapTiles", tiles: keys }) }
  function installCalendar(request) { send(Object.assign({ cmd: "installCalendar" }, request)) }
  function renameCalendar(folder, name) { send({ cmd: "renameCalendar", folder: String(folder), name: String(name) }) }
  function deleteCalendar(folder) { send({ cmd: "deleteCalendar", folder: String(folder) }) }

  function consumeBackendLine(line) {
    var message
    try {
      message = JSON.parse(line)
    } catch (error) {
      console.warn("Dulook backend:", line)
      return
    }
    if (message.type === "state") {
      backendState = message
      calendarRevision = message.calendarRevision || 0
    } else if (message.type === "tick") {
      backendTick = message
    } else if (message.type === "calendarBrowser") {
      calendarBrowserReply(message)
    }
  }

  onSettingsChanged: sendSettings()
  onDndChanged: send({ cmd: "dnd", value: dnd })
  // The shell injects omarchyPath before shell, so the backend starts with the final paths.
  onShellChanged: if (shell && !backend.running) backend.running = true

  CalendarWindow {
    id: calendarWindow
    service: root
  }

  Process {
    id: backend
    command: [root.pluginDir + "bin/dulook",
      "--plugin-dir", root.pluginDir,
      "--omarchy-path", root.omarchyPath,
      "--cache-file", Quickshell.cachePath("dulook-last-hadith.json"),
      "--state-file", Quickshell.statePath("dulook-state.json")]
    stdinEnabled: true
    stdout: SplitParser {
      onRead: function(line) { root.consumeBackendLine(line) }
    }
    onStarted: {
      root.backendReady = true
      root.send({ cmd: "dnd", value: root.dnd })
      root.sendSettings()
    }
    onExited: {
      root.backendReady = false
      root.calendarRevision = 0
      if (root.shell) backendRetry.restart()
    }
  }

  Timer {
    id: backendRetry
    interval: 1000
    repeat: false
    onTriggered: if (!backend.running) backend.running = true
  }

  property FileView countriesFile: FileView {
    path: root.pluginDir + "data/countries.json"
    watchChanges: false
    printErrors: false
    onLoaded: {
      try {
        root.countryList = JSON.parse(text())
      } catch (error) {
        console.warn("Dulook: invalid data/countries.json:", error)
      }
    }
  }

  IpcHandler {
    target: "deyaxshami.dulook"
    function open(): void { if (root.shell) root.shell.summon("deyaxshami.dulook", "{}") }
    function close(): void { if (root.shell) root.shell.hide("deyaxshami.dulook") }
    function toggle(): void { if (root.shell) root.shell.toggle("deyaxshami.dulook", "{}") }
    function refresh(): void { root.reloadCalendar(); root.requestHadith() }
    function nextHadith(): void { root.nextHadith() }
    function fallbackHadith(): void { root.showFallback() }
    function testNotification(): void { root.testNotification() }
    function stopPrayerSound(alertKey: string): void { root.stopPrayerSound(alertKey) }
    function stopSound(): void { root.stopSound() }
    function acknowledgeAlert(): void { root.acknowledgeAlert() }
    function status(): string {
      return JSON.stringify({
        language: root.language,
        hadithLanguage: root.hadithLanguage,
        location: root.location,
        downloadStatus: root.downloadStatus,
        calendarPath: root.calendarPath,
        calendarReady: root.calendarReady,
        calendarError: root.calendarError,
        coverage: root.coverage,
        prayerRows: root.prayerRows,
        nextPrayer: root.nextPrayer,
        countdown: root.countdown,
        alerting: root.alerting,
        prayerTimeNotificationsEnabled: root.prayerTimeNotificationsEnabled,
        prayerRemindersEnabled: root.prayerRemindersEnabled,
        lastThirdReminderEnabled: root.lastThirdReminderEnabled,
        soundEnabled: root.soundEnabled,
        soundPlaying: root.soundPlaying,
        timezone: root.timezoneLabel,
        hadithSource: root.hadithSource,
        hadithCount: root.fallbackCount,
        hadithLoading: root.hadithLoading,
        dnd: root.dnd
      })
    }
  }
}
