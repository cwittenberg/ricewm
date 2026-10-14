use super::*;
#[test]
fn dutch_locale_is_available_to_native_apps_and_settings() {
    assert_eq!(locale_code("nl"), 23);
    assert_eq!(language_name("nl"), Some("Nederlands"));
    assert!(LANGUAGE_VALUES.contains(&"nl"));
    with_locale(locale_code("nl"), || {
        assert_eq!(t("common.save"), "Opslaan");
        assert_eq!(t("common.cancel"), "Annuleren");
        assert_eq!(direction(), TextDirection::Ltr);
    });
}

#[test]
fn east_asian_locales_have_distinct_native_translations() {
    for (locale, code, save, system) in [
        ("zh-Hans", 2, "保存", "系统"),
        ("zh-Hant", 24, "儲存", "系統"),
        ("ja", 12, "保存", "システム"),
        ("ko", 19, "저장", "시스템"),
    ] {
        assert_eq!(locale_code(locale), code);
        with_locale(code, || {
            assert_eq!(t("common.save"), save);
            assert_eq!(t("settings.system"), system);
        });
    }
}
    #[test]
    fn scoped_languages_and_english_fallback() {
        assert_eq!(t("common.save"), "Save");
        with_locale(1, || {
            assert_eq!(t("Save"), "Guardar");
            with_locale(2, || assert_eq!(t("Save"), "保存"));
            assert_eq!(t("Save"), "Guardar");
            assert_eq!(t("Untranslated message"), "Untranslated message");
        });
        assert_eq!(t("Save"), "Save");
        with_locale(99, || assert_eq!(t("common.save"), "Save"));
    }
    #[test]
    fn interpolation_does_not_reinterpret_user_content() {
        assert_eq!(format("Delete {name}?", &[("name", "{other}")]), "Delete {other}?");
    }

#[test]
fn date_time_patterns_follow_the_locale_and_validate_calendar_components() {
    for (locale,expected) in [(0,"5/9/2024, 03:04:05"),(1,"9/5/2024, 03:04:05"),(2,"2024年5月9日 03:04:05"),(255,"5/9/2024, 03:04:05")] {
        with_locale(locale, || assert_eq!(format_date_time(2024,5,9,3,4,5).as_deref(),Some(expected)));
    }
    assert!(format_date_time(2024,2,29,0,0,0).is_some());
    assert!(format_date_time(2000,2,29,23,59,59).is_some());
    for (year,month,day,hour,minute,second) in [(1900,2,29,0,0,0),(2023,2,29,0,0,0),(2024,4,31,0,0,0),(2024,0,1,0,0,0),(2024,13,1,0,0,0),(2024,1,0,0,0,0),(0,1,1,0,0,0),(10000,1,1,0,0,0),(2024,1,1,24,0,0),(2024,1,1,0,60,0),(2024,1,1,0,0,60)] {
        assert_eq!(format_date_time(year,month,day,hour,minute,second),None);
    }
}


#[test]
fn base_direction_follows_scoped_locale_and_unknown_locale_falls_back() {
    use super::{TextDirection, direction, locale_code, with_locale};
    for locale in ["ar", "fa", "ur"] {
        with_locale(locale_code(locale), || {
            assert_eq!(direction(), TextDirection::Rtl);
            with_locale(locale_code("es"), || assert_eq!(direction(), TextDirection::Ltr));
            assert_eq!(direction(), TextDirection::Rtl);
        });
    }
    with_locale(255, || assert_eq!(direction(), TextDirection::Ltr));
    assert_eq!(direction(), TextDirection::Ltr);
}
