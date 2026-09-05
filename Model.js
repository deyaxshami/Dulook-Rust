// Pure calendar and scheduling logic shared by QML and the Node test suite.

var PRAYERS = ["Fajr", "Dhuhr", "Asr", "Maghrib", "Isha"]
var REQUIRED_HEADERS = ["date", "fajr", "dhuhr", "asr", "maghrib", "isha"]
var HEADER_ALIASES = {
  "date": "date",
  "التاريخ": "date",
  "fajr": "fajr",
  "الفجر": "fajr",
  "dhuhr": "dhuhr",
  "zuhr": "dhuhr",
  "الظهر": "dhuhr",
  "asr": "asr",
  "العصر": "asr",
  "maghrib": "maghrib",
  "المغرب": "maghrib",
  "isha": "isha",
  "ishaa": "isha",
  "العشاء": "isha"
}

function trim(value) {
  return String(value === undefined || value === null ? "" : value).replace(/^\s+|\s+$/g, "")
}

function pad2(value) {
  return Number(value) < 10 ? "0" + Number(value) : String(Number(value))
}

function dateKey(year, month, day) {
  return String(year) + "-" + pad2(month) + "-" + pad2(day)
}

function keyForDate(date) {
  return dateKey(date.getFullYear(), date.getMonth() + 1, date.getDate())
}

function parseCsvRow(line) {
  var values = []
  var value = ""
  var quoted = false
  for (var i = 0; i < line.length; i++) {
    var character = line.charAt(i)
    if (character === '"') {
      if (quoted && line.charAt(i + 1) === '"') {
        value += '"'
        i++
      } else {
        quoted = !quoted
      }
    } else if (character === "," && !quoted) {
      values.push(trim(value))
      value = ""
    } else {
      value += character
    }
  }
  if (quoted) throw new Error("Unclosed quoted field")
  values.push(trim(value))
  return values
}

function canonicalHeader(value) {
  var normalized = trim(value).replace(/^\ufeff/, "").toLowerCase()
  return HEADER_ALIASES[normalized] || normalized
}

function parseDate(value) {
  var text = trim(value)
  var day
  var month
  var year
  var match = /^(\d{2})\/(\d{2})\/(\d{4})$/.exec(text)
  if (match) {
    day = Number(match[1])
    month = Number(match[2])
    year = Number(match[3])
  } else {
    match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(text)
    if (!match) throw new Error("Invalid date: " + text)
    year = Number(match[1])
    month = Number(match[2])
    day = Number(match[3])
  }
  var date = new Date(year, month - 1, day, 12, 0, 0, 0)
  if (date.getFullYear() !== year || date.getMonth() !== month - 1 || date.getDate() !== day)
    throw new Error("Invalid date: " + text)
  return { year: year, month: month, day: day, key: dateKey(year, month, day) }
}

function parseTime(value) {
  var text = trim(value).toLowerCase()
  var match = /^(\d{1,2}):(\d{2})\s*(am|pm)?$/.exec(text)
  if (!match) throw new Error("Invalid time: " + value)
  var hour = Number(match[1])
  var minute = Number(match[2])
  var meridiem = match[3] || ""
  if (minute > 59 || (meridiem !== "" && (hour < 1 || hour > 12)) || (meridiem === "" && hour > 23))
    throw new Error("Invalid time: " + value)
  if (meridiem !== "") {
    hour = hour % 12
    if (meridiem === "pm") hour += 12
  }
  return pad2(hour) + ":" + pad2(minute)
}

function validatePrayerOrder(prayers, key) {
  var previous = minuteOfDay(prayers.Fajr)
  var daytime = ["Dhuhr", "Asr", "Maghrib"]
  for (var i = 0; i < daytime.length; i++) {
    var current = minuteOfDay(prayers[daytime[i]])
    if (current <= previous) throw new Error("Invalid prayer order: " + key)
    previous = current
  }
  if (minuteOfDay(prayers.Isha) === previous) throw new Error("Invalid prayer order: " + key)
}

function parseCalendar(csv) {
  var lines = String(csv || "").replace(/\r/g, "").split("\n")
  var meaningful = []
  for (var i = 0; i < lines.length; i++) {
    if (trim(lines[i]) !== "") meaningful.push(lines[i])
  }
  if (meaningful.length < 2) throw new Error("The calendar is empty")

  var headers = parseCsvRow(meaningful[0])
  var columns = {}
  for (var h = 0; h < headers.length; h++) columns[canonicalHeader(headers[h])] = h
  for (var required = 0; required < REQUIRED_HEADERS.length; required++) {
    var header = REQUIRED_HEADERS[required]
    if (columns[header] === undefined) throw new Error("Missing column: " + header)
  }

  var dates = {}
  var keys = []
  for (var rowIndex = 1; rowIndex < meaningful.length; rowIndex++) {
    var row = parseCsvRow(meaningful[rowIndex])
    var parsedDate = parseDate(row[columns.date])
    if (dates[parsedDate.key]) throw new Error("Duplicate date: " + parsedDate.key)
    var prayers = {}
    for (var p = 0; p < PRAYERS.length; p++) {
      var prayer = PRAYERS[p]
      prayers[prayer] = parseTime(row[columns[prayer.toLowerCase()]])
    }
    validatePrayerOrder(prayers, parsedDate.key)
    eventsForRow(parsedDate.key, prayers)
    dates[parsedDate.key] = prayers
    keys.push(parsedDate.key)
  }
  keys.sort()
  for (var keyIndex = 0; keyIndex < keys.length - 1; keyIndex++) {
    var key = keys[keyIndex]
    var nextKey = keys[keyIndex + 1]
    var rowPrayers = dates[key]
    if (nextKey === shiftedKey(key, 1)
        && minuteOfDay(rowPrayers.Isha) < minuteOfDay(rowPrayers.Maghrib)
        && minuteOfDay(rowPrayers.Isha) >= minuteOfDay(dates[nextKey].Fajr))
      throw new Error("Invalid overnight Isha: " + key)
  }
  return {
    dates: dates,
    keys: keys,
    rowCount: keys.length,
    firstDate: keys[0],
    lastDate: keys[keys.length - 1]
  }
}

function dateFromKey(key) {
  var parts = String(key).split("-")
  return new Date(Number(parts[0]), Number(parts[1]) - 1, Number(parts[2]), 0, 0, 0, 0)
}

function shiftedKey(key, amount) {
  var date = dateFromKey(key)
  date.setDate(date.getDate() + Number(amount))
  return keyForDate(date)
}

function minuteOfDay(time) {
  var parts = String(time).split(":")
  return Number(parts[0]) * 60 + Number(parts[1])
}

function formatTime12Hour(time) {
  var text = trim(time)
  var match = /^(\d{1,2}):(\d{2})$/.exec(text)
  if (!match) return text
  var hour = Number(match[1])
  var minute = Number(match[2])
  if (hour > 23 || minute > 59) return text
  var displayHour = hour % 12 || 12
  return displayHour + ":" + pad2(minute) + (hour < 12 ? " AM" : " PM")
}

function eventsForRow(key, prayers) {
  var base = dateFromKey(key)
  var events = []
  var dayOffset = 0
  var previousMinute = -1
  for (var i = 0; i < PRAYERS.length; i++) {
    var prayer = PRAYERS[i]
    var time = prayers[prayer]
    var minute = minuteOfDay(time)
    if (previousMinute >= 0 && minute < previousMinute) dayOffset++
    var targetDate = new Date(base.getFullYear(), base.getMonth(), base.getDate() + dayOffset, 12, 0, 0, 0)
    var expectedHour = Math.floor(minute / 60)
    var expectedMinute = minute % 60
    var eventDate = new Date(targetDate.getFullYear(), targetDate.getMonth(), targetDate.getDate(),
      expectedHour, expectedMinute, 0, 0)
    if (eventDate.getFullYear() !== targetDate.getFullYear()
        || eventDate.getMonth() !== targetDate.getMonth()
        || eventDate.getDate() !== targetDate.getDate()
        || eventDate.getHours() !== expectedHour
        || eventDate.getMinutes() !== expectedMinute)
      throw new Error("Invalid local time: " + key + " " + time)
    events.push({
      key: key + "|" + prayer,
      prayerDate: key,
      name: prayer,
      time: time,
      timestamp: eventDate.getTime()
    })
    previousMinute = minute
  }
  return events
}

function eventsAround(calendar, nowMs) {
  if (!calendar || !calendar.dates) return []
  var now = new Date(nowMs)
  var today = keyForDate(now)
  var events = []
  for (var offset = -1; offset <= 2; offset++) {
    var key = shiftedKey(today, offset)
    var prayers = calendar.dates[key]
    if (prayers) events = events.concat(eventsForRow(key, prayers))
  }
  events.sort(function(left, right) { return left.timestamp - right.timestamp })
  return events
}

function prayerEventForRow(key, prayers, name) {
  var events = eventsForRow(key, prayers)
  for (var i = 0; i < events.length; i++)
    if (events[i].name === name) return events[i]
  return null
}

function lastThirdEventsAround(calendar, nowMs) {
  if (!calendar || !calendar.dates) return []
  var today = keyForDate(new Date(nowMs))
  var events = []
  for (var offset = -2; offset <= 1; offset++) {
    var nightDate = shiftedKey(today, offset)
    var nextDate = shiftedKey(nightDate, 1)
    var prayers = calendar.dates[nightDate]
    var nextPrayers = calendar.dates[nextDate]
    if (!prayers || !nextPrayers) continue
    var maghrib = prayerEventForRow(nightDate, prayers, "Maghrib")
    var fajr = prayerEventForRow(nextDate, nextPrayers, "Fajr")
    if (!maghrib || !fajr || fajr.timestamp <= maghrib.timestamp) continue
    events.push({
      key: nightDate + "|last-third",
      nightDate: nightDate,
      timestamp: Math.round(maghrib.timestamp + (fajr.timestamp - maghrib.timestamp) * 2 / 3),
      maghribTimestamp: maghrib.timestamp,
      fajrTimestamp: fajr.timestamp
    })
  }
  return events
}

function snapshot(calendar, nowMs) {
  var now = new Date(nowMs)
  var today = keyForDate(now)
  var events = eventsAround(calendar, nowMs)
  var next = null
  for (var i = 0; i < events.length; i++) {
    if (events[i].timestamp >= nowMs) {
      next = events[i]
      break
    }
  }
  return {
    dateKey: today,
    prayers: calendar && calendar.dates ? (calendar.dates[today] || null) : null,
    next: next,
    remainingSeconds: next ? Math.max(0, Math.floor((next.timestamp - nowMs) / 1000)) : 0,
    timezoneOffset: now.getTimezoneOffset()
  }
}

function eventsDue(events, previousMs, nowMs, graceMs, advanceMs) {
  var lowerBound = Math.max(Number(previousMs), Number(nowMs) - Number(graceMs))
  var advance = Math.max(0, Number(advanceMs) || 0)
  var due = []
  for (var i = 0; i < events.length; i++) {
    var dueAt = events[i].timestamp - advance
    if (dueAt > lowerBound && dueAt <= nowMs) due.push(events[i])
  }
  return due
}

function dueEvents(calendar, previousMs, nowMs, graceMs, advanceMs) {
  return eventsDue(eventsAround(calendar, nowMs), previousMs, nowMs, graceMs, advanceMs)
}

function dueLastThirdEvents(calendar, previousMs, nowMs, graceMs) {
  return eventsDue(lastThirdEventsAround(calendar, nowMs), previousMs, nowMs, graceMs, 0)
}

function formatCountdown(seconds) {
  var value = Math.max(0, Math.floor(Number(seconds) || 0))
  var hours = Math.floor(value / 3600)
  var minutes = Math.floor((value % 3600) / 60)
  var remaining = value % 60
  return pad2(hours) + ":" + pad2(minutes) + ":" + pad2(remaining)
}

function nextFallbackIndex(length, currentIndex, randomValue) {
  var count = Math.max(0, Number(length) || 0)
  if (count === 0) return -1
  if (count === 1) return 0
  var candidate = Math.floor(Math.max(0, Math.min(0.999999, Number(randomValue))) * count)
  if (candidate === currentIndex) candidate = (candidate + 1) % count
  return candidate
}

function shuffledIndices(length, avoidFirst, randomValues) {
  var count = Math.max(0, Math.floor(Number(length) || 0))
  var order = []
  for (var i = 0; i < count; i++) order.push(i)
  var randomIndex = 0
  for (var j = order.length - 1; j > 0; j--) {
    var random = randomValues && randomIndex < randomValues.length
      ? Number(randomValues[randomIndex++]) : Math.random()
    var swapIndex = Math.floor(Math.max(0, Math.min(0.999999, random)) * (j + 1))
    var value = order[j]
    order[j] = order[swapIndex]
    order[swapIndex] = value
  }
  if (order.length > 1 && order[0] === avoidFirst) {
    var first = order[0]
    order[0] = order[1]
    order[1] = first
  }
  return order
}
