var STRINGS = {
  en: {
    appName: "Dulook",
    setup: "Choose a prayer calendar",
    setupHint: "Prayer times stay local and use your system timezone.",
    nextPrayer: "Next prayer",
    noPrayerData: "No prayer times for today",
    noCalendar: "No calendar selected",
    invalidCalendar: "Could not read the selected calendar",
    systemTimezone: "System timezone",
    hadithReminder: "A reminder from the Sunnah",
    loadingHadith: "Loading a Hadith...",
    online: "Online",
    offline: "Offline collection",
    cached: "Cached",
    settings: "Settings",
    back: "Back",
    language: "Language",
    calendar: "Prayer calendar",
    calendarPath: "CSV file",
    chooseFile: "Choose file",
    csvFiles: "CSV files",
    mp3Files: "MP3 audio",
    allFiles: "All files",
    clear: "Clear",
    reload: "Reload",
    coverage: "Coverage",
    rows: "days",
    notifications: "Notifications",
    enableNotifications: "Prayer notifications",
    enableNotificationsHint: "Show a native Omarchy notification at prayer time.",
    bypassDnd: "Bypass Do Not Disturb",
    bypassDndHint: "Allow prayer notifications and enabled sound during DND.",
    sound: "Adhan sound",
    enableSound: "Play Adhan audio",
    soundPath: "MP3 file",
    volume: "Volume",
    testNotification: "Test notification",
    testSound: "Test sound",
    stopSound: "Stop sound",
    appearance: "Appearance",
    uiFont: "Arabic UI font",
    hadithFont: "Arabic Hadith font",
    notificationTitle: "Prayer time",
    notificationBody: "It is time for {prayer}.",
    testNotificationBody: "Dulook notifications are ready.",
    fileUnavailable: "The selected CSV is unavailable.",
    invalidSound: "Choose a readable MP3 file.",
    soundPlaybackFailed: "The selected audio file could not be played.",
    Fajr: "Fajr",
    Dhuhr: "Dhuhr",
    Asr: "Asr",
    Maghrib: "Maghrib",
    Isha: "Isha",
    bukhari: "Sahih al-Bukhari",
    muslim: "Sahih Muslim"
  },
  ar: {
    appName: "دلوك",
    setup: "اختر تقويم الصلاة",
    setupHint: "تبقى المواقيت محلية وتستخدم المنطقة الزمنية للنظام.",
    nextPrayer: "الصلاة القادمة",
    noPrayerData: "لا توجد مواقيت صلاة لليوم",
    noCalendar: "لم يتم اختيار تقويم",
    invalidCalendar: "تعذرت قراءة التقويم المحدد",
    systemTimezone: "المنطقة الزمنية للنظام",
    hadithReminder: "تذكرة من السنة",
    loadingHadith: "جارٍ تحميل حديث…",
    online: "متصل",
    offline: "المجموعة المحلية",
    cached: "محفوظ",
    settings: "الإعدادات",
    back: "رجوع",
    language: "اللغة",
    calendar: "تقويم الصلاة",
    calendarPath: "ملف CSV",
    chooseFile: "اختر ملفًا",
    csvFiles: "ملفات CSV",
    mp3Files: "ملفات MP3 الصوتية",
    allFiles: "كل الملفات",
    clear: "مسح",
    reload: "إعادة تحميل",
    coverage: "التغطية",
    rows: "يومًا",
    notifications: "الإشعارات",
    enableNotifications: "إشعارات الصلاة",
    enableNotificationsHint: "اعرض إشعار أوماركي عند دخول وقت الصلاة.",
    bypassDnd: "تجاوز وضع عدم الإزعاج",
    bypassDndHint: "اسمح بإشعارات الصلاة والصوت المفعّل أثناء عدم الإزعاج.",
    sound: "صوت الأذان",
    enableSound: "تشغيل صوت الأذان",
    soundPath: "ملف MP3",
    volume: "مستوى الصوت",
    testNotification: "اختبار الإشعار",
    testSound: "اختبار الصوت",
    stopSound: "إيقاف الصوت",
    appearance: "المظهر",
    uiFont: "خط الواجهة العربية",
    hadithFont: "خط الحديث العربي",
    notificationTitle: "حان وقت الصلاة",
    notificationBody: "حان الآن وقت صلاة {prayer}.",
    testNotificationBody: "إشعارات دلوك جاهزة.",
    fileUnavailable: "ملف CSV المحدد غير متاح.",
    invalidSound: "اختر ملف MP3 صالحًا للقراءة.",
    soundPlaybackFailed: "تعذر تشغيل ملف الصوت المحدد.",
    Fajr: "الفجر",
    Dhuhr: "الظهر",
    Asr: "العصر",
    Maghrib: "المغرب",
    Isha: "العشاء",
    bukhari: "صحيح البخاري",
    muslim: "صحيح مسلم"
  }
}

function language(value) {
  return String(value) === "ar" ? "ar" : "en"
}

function text(key, selectedLanguage) {
  var lang = language(selectedLanguage)
  return STRINGS[lang][key] !== undefined ? STRINGS[lang][key] : (STRINGS.en[key] || key)
}

function interpolate(key, selectedLanguage, values) {
  var result = text(key, selectedLanguage)
  for (var name in values) result = result.replace("{" + name + "}", values[name])
  return result
}

function isRtl(selectedLanguage) {
  return language(selectedLanguage) === "ar"
}
