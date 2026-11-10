import QtQuick
import Quickshell
import Quickshell.Io

// Thin bridge to the Rust backend (bin/dulook). The backend owns all logic:
// calendar, alerts, Adhan audio, Hadith, translations, and formatting.
// It receives commands as JSON lines on stdin and answers with
// {"type":"state"} and {"type":"tick"} JSON lines on stdout.
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
        if (entries[i] && entries[i].id === "karem.dulook") return entries[i]
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

  readonly property string language: backendState.language === "ar" ? "ar" : "en"
  readonly property string hadithLanguage: backendState.hadithLanguage === "ar" ? "ar" : "en"
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
  readonly property string soundChoice: backendState.soundChoice || "adans/adan_01.mp3"
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
  function refreshClock() { send({ cmd: "refreshClock" }) }
  function nextHadith() { send({ cmd: "nextHadith" }) }
  function previousHadith() { send({ cmd: "previousHadith" }) }
  function showFallback() { send({ cmd: "showFallback" }) }
  function requestHadith() { send({ cmd: "requestHadith" }) }
  function testNotification() { send({ cmd: "testNotification" }) }
  function testSound() { send({ cmd: "testSound" }) }
  function stopSound() { send({ cmd: "stopSound" }) }
  function stopPrayerSound(alertKey) { send({ cmd: "stopPrayerSound", key: String(alertKey) }) }

  function consumeBackendLine(line) {
    var message
    try {
      message = JSON.parse(line)
    } catch (error) {
      console.warn("Dulook backend:", line)
      return
    }
    if (message.type === "state") backendState = message
    else if (message.type === "tick") backendTick = message
  }

  onSettingsChanged: sendSettings()
  onDndChanged: send({ cmd: "dnd", value: dnd })
  // The shell injects omarchyPath before shell, so the backend starts with the final paths.
  onShellChanged: if (shell && !backend.running) backend.running = true

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
      if (root.shell) backendRetry.restart()
    }
  }

  Timer {
    id: backendRetry
    interval: 1000
    repeat: false
    onTriggered: if (!backend.running) backend.running = true
  }

  IpcHandler {
    target: "karem.dulook"
    function open(): void { if (root.shell) root.shell.summon("karem.dulook", "{}") }
    function close(): void { if (root.shell) root.shell.hide("karem.dulook") }
    function toggle(): void { if (root.shell) root.shell.toggle("karem.dulook", "{}") }
    function refresh(): void { root.reloadCalendar(); root.requestHadith() }
    function nextHadith(): void { root.nextHadith() }
    function fallbackHadith(): void { root.showFallback() }
    function testNotification(): void { root.testNotification() }
    function stopPrayerSound(alertKey: string): void { root.stopPrayerSound(alertKey) }
    function stopSound(): void { root.stopSound() }
    function status(): string {
      return JSON.stringify({
        language: root.language,
        hadithLanguage: root.hadithLanguage,
        calendarPath: root.calendarPath,
        calendarReady: root.calendarReady,
        calendarError: root.calendarError,
        coverage: root.coverage,
        prayerRows: root.prayerRows,
        nextPrayer: root.nextPrayer,
        countdown: root.countdown,
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
