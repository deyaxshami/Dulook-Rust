//! UI strings (port of the original `Translations.js`).

use serde_json::{Map, Value};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    En,
    Ar,
}

impl Lang {
    pub fn from_setting(value: &str) -> Lang {
        if value == "ar" { Lang::Ar } else { Lang::En }
    }

    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Ar => "ar",
        }
    }

    pub fn is_arabic(self) -> bool {
        self == Lang::Ar
    }
}

pub static EN: &[(&str, &str)] = &[
    ("appName", "Dulook"),
    ("setup", "Choose a prayer calendar"),
    ("setupHint", "Prayer times stay local and use your system timezone."),
    ("nextPrayer", "Next prayer"),
    ("countdownToPrayer", "{countdown} to {prayer}"),
    ("noPrayerData", "No prayer times for today"),
    ("noCalendar", "No calendar selected"),
    ("invalidCalendar", "Could not read the selected calendar"),
    ("systemTimezone", "System timezone"),
    ("hadithReminder", "A reminder from the Sunnah"),
    ("loadingHadith", "Loading a Hadith..."),
    ("online", "Online"),
    ("offline", "Offline collection"),
    ("cached", "Cached"),
    ("settings", "Settings"),
    ("back", "Back"),
    ("language", "Language"),
    ("uiLanguage", "Interface language"),
    ("hadithLanguage", "Hadith language"),
    ("calendar", "Prayer calendar"),
    ("calendarPath", "CSV file"),
    ("chooseFile", "Choose file"),
    ("csvFiles", "CSV files"),
    ("mp3Files", "MP3 audio"),
    ("allFiles", "All files"),
    ("clear", "Clear"),
    ("reload", "Reload"),
    ("coverage", "Coverage"),
    ("rows", "days"),
    ("notifications", "Notifications"),
    ("enablePrayerTimeNotifications", "Prayer-time notifications"),
    ("enablePrayerTimeNotificationsHint", "Show a notification when each enabled prayer begins."),
    ("enablePrayerReminders", "15-minute reminders"),
    ("enablePrayerRemindersHint", "Remind you before each enabled prayer."),
    ("enableLastThirdReminder", "Last-third reminder"),
    ("enableLastThirdReminderHint", "Notify when the last third begins, calculated from Maghrib to the next Fajr."),
    ("prayerTimePerPrayer", "Prayer-time notifications by prayer"),
    ("reminderPerPrayer", "15-minute reminders by prayer"),
    ("bypassDnd", "Bypass Do Not Disturb"),
    ("bypassDndHint", "Allow prayer notifications and enabled sound during DND."),
    ("sound", "Adhan sound"),
    ("enableSound", "Play Adhan audio"),
    ("soundPath", "MP3 file"),
    ("adhanChoice", "Adhan"),
    ("adhanOption", "Adhan {number}"),
    ("customSound", "Custom file…"),
    ("volume", "Volume"),
    ("testNotification", "Test notification"),
    ("testSound", "Test sound"),
    ("stopSound", "Stop sound"),
    ("appearance", "Appearance"),
    ("uiFont", "Arabic UI font"),
    ("hadithFont", "Arabic Hadith font"),
    ("notificationTitle", "Prayer time"),
    ("notificationBody", "It is time for {prayer}."),
    ("stopAdhanHint", "Left-click to stop the Adhan."),
    ("prayerReminderTitle", "Prayer reminder"),
    ("prayerReminderBody", "{prayer} begins in 15 minutes."),
    ("lastThirdTitle", "Last third of the night"),
    ("lastThirdBody", "The last third of the night has begun."),
    ("testNotificationBody", "Dulook notifications are ready."),
    ("fileUnavailable", "The selected CSV is unavailable."),
    ("invalidSound", "Choose a readable MP3 file."),
    ("soundPlaybackFailed", "The selected audio file could not be played."),
    ("Fajr", "Fajr"),
    ("Dhuhr", "Dhuhr"),
    ("Asr", "Asr"),
    ("Maghrib", "Maghrib"),
    ("Isha", "Isha"),
    ("bukhari", "Sahih al-Bukhari"),
    ("muslim", "Sahih Muslim"),
];

pub static AR: &[(&str, &str)] = &[
    ("appName", "دلوك"),
    ("setup", "اختر تقويم الصلاة"),
    ("setupHint", "تبقى المواقيت محلية وتستخدم المنطقة الزمنية للنظام."),
    ("nextPrayer", "الصلاة القادمة"),
    ("countdownToPrayer", "{countdown} حتى {prayer}"),
    ("noPrayerData", "لا توجد مواقيت صلاة لليوم"),
    ("noCalendar", "لم يتم اختيار تقويم"),
    ("invalidCalendar", "تعذرت قراءة التقويم المحدد"),
    ("systemTimezone", "المنطقة الزمنية للنظام"),
    ("hadithReminder", "تذكرة من السنة"),
    ("loadingHadith", "جارٍ تحميل حديث…"),
    ("online", "متصل"),
    ("offline", "المجموعة المحلية"),
    ("cached", "محفوظ"),
    ("settings", "الإعدادات"),
    ("back", "رجوع"),
    ("language", "اللغة"),
    ("uiLanguage", "لغة الواجهة"),
    ("hadithLanguage", "لغة الحديث"),
    ("calendar", "تقويم الصلاة"),
    ("calendarPath", "ملف CSV"),
    ("chooseFile", "اختر ملفًا"),
    ("csvFiles", "ملفات CSV"),
    ("mp3Files", "ملفات MP3 الصوتية"),
    ("allFiles", "كل الملفات"),
    ("clear", "مسح"),
    ("reload", "إعادة تحميل"),
    ("coverage", "التغطية"),
    ("rows", "يومًا"),
    ("notifications", "الإشعارات"),
    ("enablePrayerTimeNotifications", "إشعارات دخول وقت الصلاة"),
    ("enablePrayerTimeNotificationsHint", "اعرض إشعارًا عند دخول وقت كل صلاة مفعّلة."),
    ("enablePrayerReminders", "تذكير قبل الصلاة بـ ١٥ دقيقة"),
    ("enablePrayerRemindersHint", "ذكّرني قبل كل صلاة مفعّلة."),
    ("enableLastThirdReminder", "تذكير الثلث الأخير"),
    ("enableLastThirdReminderHint", "أرسل إشعارًا عند بداية الثلث الأخير، محسوبًا من المغرب إلى فجر اليوم التالي."),
    ("prayerTimePerPrayer", "إشعارات دخول الوقت لكل صلاة"),
    ("reminderPerPrayer", "تذكيرات ١٥ دقيقة لكل صلاة"),
    ("bypassDnd", "تجاوز وضع عدم الإزعاج"),
    ("bypassDndHint", "اسمح بإشعارات الصلاة والصوت المفعّل أثناء عدم الإزعاج."),
    ("sound", "صوت الأذان"),
    ("enableSound", "تشغيل صوت الأذان"),
    ("soundPath", "ملف MP3"),
    ("adhanChoice", "الأذان"),
    ("adhanOption", "الأذان {number}"),
    ("customSound", "ملف مخصص…"),
    ("volume", "مستوى الصوت"),
    ("testNotification", "اختبار الإشعار"),
    ("testSound", "اختبار الصوت"),
    ("stopSound", "إيقاف الصوت"),
    ("appearance", "المظهر"),
    ("uiFont", "خط الواجهة العربية"),
    ("hadithFont", "خط الحديث العربي"),
    ("notificationTitle", "حان وقت الصلاة"),
    ("notificationBody", "حان الآن وقت صلاة {prayer}."),
    ("stopAdhanHint", "انقر بالزر الأيسر لإيقاف الأذان."),
    ("prayerReminderTitle", "تذكير بالصلاة"),
    ("prayerReminderBody", "تبقى ١٥ دقيقة على صلاة {prayer}."),
    ("lastThirdTitle", "الثلث الأخير من الليل"),
    ("lastThirdBody", "بدأ الثلث الأخير من الليل."),
    ("testNotificationBody", "إشعارات دلوك جاهزة."),
    ("fileUnavailable", "ملف CSV المحدد غير متاح."),
    ("invalidSound", "اختر ملف MP3 صالحًا للقراءة."),
    ("soundPlaybackFailed", "تعذر تشغيل ملف الصوت المحدد."),
    ("Fajr", "الفجر"),
    ("Dhuhr", "الظهر"),
    ("Asr", "العصر"),
    ("Maghrib", "المغرب"),
    ("Isha", "العشاء"),
    ("bukhari", "صحيح البخاري"),
    ("muslim", "صحيح مسلم"),
];

fn table(lang: Lang) -> &'static [(&'static str, &'static str)] {
    match lang {
        Lang::En => EN,
        Lang::Ar => AR,
    }
}

fn lookup(lang: Lang, key: &str) -> Option<&'static str> {
    table(lang).iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

/// Translated text: selected language, then English, then the key itself.
pub fn text(key: &str, lang: Lang) -> &str {
    lookup(lang, key).or_else(|| lookup(Lang::En, key)).unwrap_or(key)
}

/// Replaces the first occurrence of each `{name}` placeholder, like JS `String.replace`.
pub fn interpolate(key: &str, lang: Lang, values: &[(&str, &str)]) -> String {
    let mut result = text(key, lang).to_owned();
    for (name, value) in values {
        result = result.replacen(&format!("{{{name}}}"), value, 1);
    }
    result
}

/// Every string of `lang`, for the state payload.
pub fn strings(lang: Lang) -> Map<String, Value> {
    table(lang).iter().map(|(k, v)| ((*k).to_owned(), Value::String((*v).to_owned()))).collect()
}
