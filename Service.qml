import QtQuick
import Quickshell
import Quickshell.Io
import "Model.js" as Model
import "Translations.js" as Tr

Item {
  id: root

  property var shell: null
  property string omarchyPath: "/usr/share/omarchy"
  property var manifest: null

  readonly property var settings: {
    var config = shell ? shell.shellConfig : null
    var layout = config && config.bar ? config.bar.layout : null
    var sections = ["left", "center", "right"]
    for (var s = 0; layout && s < sections.length; s++) {
      var entries = layout[sections[s]] || []
      for (var i = 0; i < entries.length; i++)
        if (entries[i] && entries[i].id === "karem.dulook") return entries[i]
    }
    return ({})
  }

  readonly property string language: String(setting("language", "en")) === "ar" ? "ar" : "en"
  readonly property string calendarPath: String(setting("calendarPath", "")).trim()
  readonly property string effectiveCalendarPath: expandPath(calendarPath)
  readonly property bool notificationsEnabled: boolSetting("notificationsEnabled", true)
  readonly property bool soundEnabled: boolSetting("soundEnabled", false)
  readonly property bool bypassDnd: boolSetting("bypassDnd", false)
  readonly property string soundPath: String(setting("soundPath", "")).trim()
  readonly property string effectiveSoundPath: expandPath(soundPath)
  readonly property int soundVolume: Math.max(0, Math.min(100, Number(setting("soundVolume", 80))))
  readonly property string arabicUiFont: String(setting("arabicUiFont", "Noto Sans Arabic"))
  readonly property string arabicHadithFont: String(setting("arabicHadithFont", "Noto Naskh Arabic"))
  readonly property var notificationService: shell ? shell.firstPartyServiceFor("omarchy.notifications") : null
  readonly property bool dnd: notificationService ? notificationService.doNotDisturb === true : false

  property var calendar: null
  property bool calendarReady: false
  property string calendarError: ""
  property string calendarErrorKey: ""
  property string coverage: ""
  property var prayerRows: []
  property var nextPrayer: null
  property string countdown: "--:--:--"
  property int timezoneOffset: new Date().getTimezoneOffset()
  property double previousTick: 0
  property string lastAlertKey: ""

  property var fallbackHadiths: []
  property var fallbackOrder: []
  property int fallbackPosition: 0
  property var hadithHistory: []
  property int hadithIndex: -1
  property int fallbackIndex: -1
  property bool hadithLoading: false
  property string hadithSource: "offline"
  property string hadithError: ""
  property string soundError: ""
  property string soundErrorKey: ""
  property bool stoppingSound: false
  property string pendingCollection: ""
  property int pendingNumber: 0
  property var pendingArabic: null
  property string pendingArabicRaw: ""
  property string pendingEnglishRaw: ""
  readonly property var currentHadith: hadithIndex >= 0 && hadithIndex < hadithHistory.length
    ? hadithHistory[hadithIndex] : null

  function setting(name, fallback) {
    var value = settings ? settings[name] : undefined
    return value === undefined || value === null ? fallback : value
  }

  function boolSetting(name, fallback) {
    var value = setting(name, fallback)
    if (typeof value === "boolean") return value
    var normalized = String(value).toLowerCase()
    if (["true", "1", "yes", "on"].indexOf(normalized) !== -1) return true
    if (["false", "0", "no", "off"].indexOf(normalized) !== -1) return false
    return fallback
  }

  function expandPath(value) {
    var path = String(value || "").trim()
    if (path.indexOf("file://") === 0) {
      path = path.substring(7)
      try { path = decodeURIComponent(path) } catch (error) {}
    }
    var home = String(Quickshell.env("HOME") || "")
    if (path.indexOf("~/") === 0) path = home + path.substring(1)
    else if (path.indexOf("$HOME/") === 0) path = home + path.substring(5)
    else if (path.indexOf("${HOME}/") === 0) path = home + path.substring(7)
    return path
  }

  function tr(key) {
    return Tr.text(key, language)
  }

  function prayerName(name) {
    return Tr.text(name, language)
  }

  function localizedDigits(value) {
    if (language !== "ar") return String(value)
    return String(value).replace(/\d/g, function(digit) { return "٠١٢٣٤٥٦٧٨٩"[digit] })
  }

  function formatTime(time) {
    return localizedDigits(Model.formatTime12Hour(time || "--:--"))
  }

  function timezoneLabel() {
    var offset = -new Date().getTimezoneOffset()
    var sign = offset >= 0 ? "+" : "-"
    var absolute = Math.abs(offset)
    return localizedDigits("UTC" + sign + Model.pad2(Math.floor(absolute / 60)) + ":" + Model.pad2(absolute % 60))
  }

  function localizedReference(hadith) {
    if (!hadith) return ""
    var collection = Tr.text(hadith.collection, language)
    return collection + " " + localizedDigits(hadith.hadithNumber) + " · "
      + (language === "ar" ? "الكتاب " : "Book ") + localizedDigits(hadith.bookNumber)
      + " · " + (language === "ar" ? "الحديث " : "Hadith ") + localizedDigits(hadith.inBookHadithNumber)
  }

  function clearCalendar(messageKey) {
    calendar = null
    calendarReady = false
    calendarErrorKey = messageKey || ""
    calendarError = calendarErrorKey === "" ? "" : tr(calendarErrorKey)
    coverage = ""
    prayerRows = []
    nextPrayer = null
    countdown = "--:--:--"
  }

  function loadCalendar() {
    if (calendarPath === "") {
      clearCalendar("noCalendar")
      return
    }
    try {
      var parsed = Model.parseCalendar(calendarFile.text())
      calendar = parsed
      calendarReady = true
      calendarError = ""
      calendarErrorKey = ""
      coverage = localizedDigits(parsed.firstDate + " — " + parsed.lastDate + " · " + parsed.rowCount) + " " + tr("rows")
      previousTick = Date.now()
      updateClock(false)
    } catch (error) {
      console.warn("Dulook calendar validation failed:", String(error))
      clearCalendar("invalidCalendar")
    }
  }

  function reloadCalendar() {
    if (calendarPath === "") {
      clearCalendar("noCalendar")
      return
    }
    calendarFile.reload()
  }

  function updateClock(checkAlerts) {
    var now = Date.now()
    var currentOffset = new Date(now).getTimezoneOffset()
    if (!calendarReady || !calendar) {
      timezoneOffset = currentOffset
      previousTick = now
      return
    }

    if (currentOffset !== timezoneOffset) {
      timezoneOffset = currentOffset
      previousTick = now
      loadCalendar()
      return
    }

    var state = Model.snapshot(calendar, now)
    timezoneOffset = currentOffset
    nextPrayer = state.next
    countdown = state.next ? localizedDigits(Model.formatCountdown(state.remainingSeconds)) : "--:--:--"
    var rows = []
    var displayDate = state.next ? state.next.prayerDate : state.dateKey
    var displayPrayers = calendar.dates[displayDate] || state.prayers
    if (displayPrayers) {
      for (var i = 0; i < Model.PRAYERS.length; i++) {
        var name = Model.PRAYERS[i]
        rows.push({ name: name, time: displayPrayers[name], prayerDate: displayDate })
      }
    }
    prayerRows = rows

    if (checkAlerts) {
      var lower = previousTick > 0 ? previousTick : now - 120000
      var due = Model.dueEvents(calendar, lower, now, 120000)
      for (var d = 0; d < due.length; d++) triggerPrayer(due[d])
    }
    previousTick = now
  }

  function prayerEnabled(name) {
    return boolSetting("notify" + name, true)
  }

  function triggerPrayer(event) {
    if (!event || event.key === lastAlertKey) return
    lastAlertKey = event.key
    saveState()
    if (!prayerEnabled(event.name)) return
    if (notificationsEnabled) sendNotification(event.name, false)
    if (soundEnabled && (!dnd || bypassDnd)) playSound()
  }

  function notificationCommand(prayer, test) {
    var title = tr("notificationTitle")
    var body = test ? tr("testNotificationBody")
      : Tr.interpolate("notificationBody", language, { prayer: prayerName(prayer) })
    var command = [omarchyPath + "/bin/omarchy-notification-send"]
    if (!bypassDnd) command = command.concat(["--app-name", "karem.dulook"])
    return command.concat(["-u", "normal", "-t", "8000", "-g", "☾", title, body])
  }

  function sendNotification(prayer, test) {
    Quickshell.execDetached(notificationCommand(prayer || "Fajr", test === true))
  }

  function playSound() {
    soundError = ""
    soundErrorKey = ""
    if (effectiveSoundPath === "" || audioProc.running) return
    stoppingSound = false
    audioProc.command = [
      "/usr/bin/mpv", "--no-config", "--no-load-scripts", "--no-video",
      "--no-terminal", "--really-quiet", "--audio-client-name=karem.dulook",
      "--volume=" + soundVolume, "--", effectiveSoundPath
    ]
    audioProc.running = true
  }

  function stopSound() {
    if (audioProc.running) {
      stoppingSound = true
      audioProc.running = false
    }
  }

  function appendHadith(item, source) {
    var stored = {}
    for (var key in item) stored[key] = item[key]
    stored.source = source
    var history = hadithHistory.slice(0, hadithIndex + 1)
    history.push(stored)
    if (history.length > 20) history.shift()
    hadithHistory = history
    hadithIndex = history.length - 1
    hadithSource = source
    hadithError = ""
  }

  function showFallback() {
    if (fallbackHadiths.length === 0) return
    if (fallbackPosition >= fallbackOrder.length) {
      fallbackOrder = Model.shuffledIndices(fallbackHadiths.length, fallbackIndex)
      fallbackPosition = 0
    }
    fallbackIndex = fallbackOrder[fallbackPosition++]
    appendHadith(fallbackHadiths[fallbackIndex], "offline")
  }

  function requestHadith() {
    if (hadithLoading || arabicHadithProc.running || englishHadithProc.running) return
    hadithLoading = true
    hadithError = ""
    pendingCollection = Math.random() < 0.5 ? "bukhari" : "muslim"
    pendingNumber = Math.floor(Math.random() * 7563) + 1
    pendingArabic = null
    pendingArabicRaw = ""
    pendingEnglishRaw = ""
    arabicHadithProc.command = ["/usr/bin/curl", "-fLsS", "--max-time", "10", hadithUrl("ara")]
    arabicHadithProc.running = true
  }

  function hadithUrl(prefix) {
    return "https://cdn.jsdelivr.net/gh/fawazahmed0/hadith-api@df57907be35291c91ad6a6691180e22ca9920784/editions/"
      + prefix + "-" + pendingCollection + "/" + pendingNumber + ".json"
  }

  function recordFromResponse(raw) {
    var parsed = JSON.parse(String(raw || ""))
    var records = parsed.hadiths || []
    for (var i = 0; i < records.length; i++)
      if (String(records[i].text || "").trim() !== "") return records[i]
    throw new Error("Empty Hadith")
  }

  function finishArabicRequest() {
    try {
      pendingArabic = recordFromResponse(pendingArabicRaw)
      englishHadithProc.command = ["/usr/bin/curl", "-fLsS", "--max-time", "10", hadithUrl("eng")]
      englishHadithProc.running = true
    } catch (error) {
      failHadith(error)
    }
  }

  function finishEnglishRequest() {
    try {
      var english = recordFromResponse(pendingEnglishRaw)
      if (!pendingArabic || String(english.hadithnumber) !== String(pendingArabic.hadithnumber))
        throw new Error("Hadith pair mismatch")
      var canonical = String(english.arabicnumber || english.hadithnumber)
      var variant = /^(\d+)\.(\d+)$/.exec(canonical)
      if (variant) canonical = variant[1] + String.fromCharCode(96 + Number(variant[2]))
      var reference = english.reference || {}
      var item = {
        id: pendingCollection + "-" + canonical,
        collection: pendingCollection,
        hadithNumber: canonical,
        bookNumber: Number(reference.book || 0),
        inBookHadithNumber: Number(reference.hadith || 0),
        arabic: String(pendingArabic.text).trim(),
        english: String(english.text).trim()
      }
      appendHadith(item, "online")
      hadithCache.setText(JSON.stringify(item))
      hadithLoading = false
    } catch (error) {
      failHadith(error)
    }
  }

  function failHadith(error) {
    hadithLoading = false
    hadithError = String(error)
    showFallback()
    hadithRetry.restart()
  }

  function nextHadith() {
    if (hadithIndex < hadithHistory.length - 1) {
      hadithIndex++
      hadithSource = currentHadith && currentHadith.source ? currentHadith.source : hadithSource
      return
    }
    requestHadith()
  }

  function previousHadith() {
    if (hadithIndex > 0) {
      hadithIndex--
      hadithSource = currentHadith && currentHadith.source ? currentHadith.source : hadithSource
    }
  }

  function loadFallbackHadiths() {
    try {
      var parsed = JSON.parse(fallbackHadithFile.text())
      fallbackHadiths = parsed.hadiths || []
      if (!currentHadith) showFallback()
      requestHadith()
    } catch (error) {
      hadithError = String(error)
    }
  }

  function loadCachedHadith() {
    try {
      var item = JSON.parse(hadithCache.text())
      if (item && item.arabic && item.english) appendHadith(item, "cached")
    } catch (error) {}
  }

  function loadState() {
    try {
      var parsed = JSON.parse(stateFile.text())
      lastAlertKey = String(parsed.lastAlertKey || "")
    } catch (error) {}
  }

  function saveState() {
    stateFile.setText(JSON.stringify({ version: 1, lastAlertKey: lastAlertKey }) + "\n")
  }

  onCalendarPathChanged: {
    clearCalendar(calendarPath === "" ? "noCalendar" : "")
    if (calendarPath !== "") Qt.callLater(root.reloadCalendar)
  }

  onLanguageChanged: {
    if (calendarErrorKey !== "") calendarError = tr(calendarErrorKey)
    if (soundErrorKey !== "") soundError = tr(soundErrorKey)
    if (calendarReady && calendar)
      coverage = localizedDigits(calendar.firstDate + " — " + calendar.lastDate + " · " + calendar.rowCount) + " " + tr("rows")
  }

  FileView {
    id: calendarFile
    path: root.effectiveCalendarPath
    blockLoading: true
    watchChanges: true
    printErrors: false
    onLoaded: root.loadCalendar()
    onFileChanged: reload()
    onLoadFailed: root.clearCalendar("fileUnavailable")
  }

  FileView {
    id: fallbackHadithFile
    path: Qt.resolvedUrl("data/hadith-fallback.json")
    blockLoading: true
    printErrors: true
    onLoaded: root.loadFallbackHadiths()
  }

  FileView {
    id: hadithCache
    path: Quickshell.cachePath("dulook-last-hadith.json")
    blockLoading: true
    printErrors: false
    onLoaded: root.loadCachedHadith()
  }

  FileView {
    id: stateFile
    path: Quickshell.statePath("dulook-state.json")
    blockLoading: true
    printErrors: false
    onLoaded: root.loadState()
  }

  Process {
    id: arabicHadithProc
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        root.pendingArabicRaw = String(text || "")
        Qt.callLater(root.finishArabicRequest)
      }
    }
  }

  Process {
    id: englishHadithProc
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        root.pendingEnglishRaw = String(text || "")
        Qt.callLater(root.finishEnglishRequest)
      }
    }
  }

  Process {
    id: audioProc
    onExited: function(exitCode) {
      if (exitCode !== 0 && !root.stoppingSound) {
        root.soundErrorKey = "soundPlaybackFailed"
        root.soundError = root.tr(root.soundErrorKey)
      }
      root.stoppingSound = false
    }
  }

  Component.onCompleted: {
    if (calendarPath === "") clearCalendar("noCalendar")
  }

  Timer {
    interval: 1000
    running: true
    repeat: true
    triggeredOnStart: true
    onTriggered: root.updateClock(true)
  }

  Timer {
    interval: 10 * 60 * 1000
    running: true
    repeat: true
    onTriggered: root.requestHadith()
  }

  Timer {
    id: hadithRetry
    interval: 5 * 60 * 1000
    repeat: false
    onTriggered: root.requestHadith()
  }

  IpcHandler {
    target: "karem.dulook"
    function open(): void { if (root.shell) root.shell.summon("karem.dulook", "{}") }
    function close(): void { if (root.shell) root.shell.hide("karem.dulook") }
    function toggle(): void { if (root.shell) root.shell.toggle("karem.dulook", "{}") }
    function refresh(): void { root.reloadCalendar(); root.requestHadith() }
    function nextHadith(): void { root.nextHadith() }
    function fallbackHadith(): void { root.showFallback() }
    function testNotification(): void { root.sendNotification("Fajr", true) }
    function stopSound(): void { root.stopSound() }
    function status(): string {
      return JSON.stringify({
        language: root.language,
        calendarPath: root.calendarPath,
        calendarReady: root.calendarReady,
        calendarError: root.calendarError,
        coverage: root.coverage,
        prayerRows: root.prayerRows,
        nextPrayer: root.nextPrayer,
        countdown: root.countdown,
        timezone: root.timezoneLabel(),
        hadithSource: root.currentHadith && root.currentHadith.source ? root.currentHadith.source : root.hadithSource,
        hadithCount: root.fallbackHadiths.length,
        hadithLoading: root.hadithLoading,
        dnd: root.dnd
      })
    }
  }
}
