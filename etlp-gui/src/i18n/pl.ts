import { zhCN } from "./zh-CN";

export const pl: typeof zhCN = {
    ...zhCN,

    // App
    app_name: "Genshin",

    // Nav
    nav_overview: "Przegląd",
    nav_player: "Odtwarzacz",
    nav_version_prefer: "Wersja",
    nav_network: "Sieć",
    nav_config: "Konfiguracja",
    nav_system: "System",
    nav_logs: "Dzienniki",
    nav_sec_play: "Odtwarzanie",
    nav_sec_settings: "Ustawienia",
    nav_sec_sync: "Synchronizacja",
    nav_bangumi: "Bangumi",
    nav_trakt: "Trakt",
    nav_sec_debug: "Debugowanie",
    nav_download: "Pobieranie",

    // Common
    add: "Dodaj",
    add_placeholder: "Wpisz i naciśnij Enter, aby dodać",
    open_dir: "Otwórz folder",
    loading: "Ładowanie konfiguracji…",

    // Overview
    page_overview: "Przegląd",
    ov_service: "Usługa lokalna",
    ov_running: "Działa",
    ov_stopped: "Zatrzymana",
    ov_port: "Port",
    ov_port_desc: "Lokalny adres nasłuchu",
    ov_uptime: "Czas działania",
    ov_uptime_desc: "Od uruchomienia usługi",
    ov_address: "Adres",
    ov_address_desc: "Tylko localhost",
    ov_config: "Konfiguracja",
    ov_config_file: "Plik konfiguracyjny",
    ov_config_file_desc: "Wyświetl lub otwórz w zewnętrznym edytorze",
    ov_edit_config: "Edytuj konfigurację",
    ov_restart: "Uruchom ponownie usługę",
    ov_restart_desc:
        "Zatrzymaj usługę, zwolnij zasoby i uruchom ponownie z najnowszą konfiguracją",
    ov_about: "O programie",
    ov_about_desc: "Informacje o wersji i podziękowania dla open source",
    ov_view: "Wyświetl",
    ov_start: "Uruchom",
    ov_stop: "Zatrzymaj",

    // Toasts
    toast_started: "Usługa uruchomiona na porcie {port}",
    toast_stopped: "Usługa zatrzymana",
    toast_restarted: "Usługa uruchomiona ponownie na porcie {port}",
    toast_start_failed: "Uruchomienie usługi nie powiodło się",
    toast_stop_failed: "Zatrzymanie usługi nie powiodło się",
    toast_restart_failed: "Ponowne uruchomienie usługi nie powiodło się",
    toast_open_failed: "Otwarcie nie powiodło się",
    sync_not_configured: "Jeszcze nie skonfigurowano — najpierw wypełnij pola",

    // Player
    page_player: "Odtwarzacz",
    pl_type: "Typ odtwarzacza",
    pl_type_desc: "Wybierz lokalny odtwarzacz multimediów",
    pl_startup: "Opcje uruchamiania",
    pl_fullscreen: "Pełny ekran",
    pl_fullscreen_desc: "Uruchom odtwarzacz w trybie pełnoekranowym",
    pl_mute: "Uruchom wyciszony",
    pl_mute_desc: "Uruchom wyciszony (mpv --mute=yes)",
    pl_pretty_title: "Ładny tytuł",
    pl_pretty_title_desc: "Dodaj nazwę serwera na początku tytułu okna odtwarzacza",
    pl_kill_start: "Zamknij przy uruchamianiu",
    pl_kill_start_desc: "Zakończ istniejące procesy odtwarzacza przy uruchamianiu",
    pl_path: "Ścieżka odtwarzacza",
    pl_path_desc: "Opcjonalne — zostaw puste, aby użyć odtwarzacza z systemowego PATH",
    pl_browse: "Przeglądaj…",
    pl_path_error: "Nie znaleziono ścieżki — sprawdź dane wejściowe",
    pl_progress_support:
        "Raportowanie postępu: mpv / IINA są w pełni obsługiwane — aktualizacje w czasie rzeczywistym podczas odtwarzania, zapis pozycji wznowienia przy wyjściu, oznaczanie jako obejrzane, synchronizacja z Trakt / Bangumi i śledzenie według odcinków. Inne odtwarzacze zapisują tylko końcową pozycję i synchronizują przy wyjściu, bez raportowania w czasie rzeczywistym; VLC odtwarza cały sezon po kolei, MPC i dandanplay dla jednego odcinka, odczyt pozycji w PotPlayer działa tylko na Windows",

    // Version prefer
    page_vp: "Preferencje wersji",
    vp_priority: "Kolejność priorytetów wersji",
    vp_keywords: "Etykiety wersji",
    vp_keywords_desc:
        "Gdy dla tego samego odcinka istnieje wiele plików, wybierany jest ten, którego ścieżka odpowiada etykiecie najwyżej w liście. Przykład: «TeamX → GroupA → StreamB» — jeśli dostępne są wszystkie trzy wersje, wybierany jest TeamX; jeśli nie — GroupA; i tak dalej",
    vp_keywords_placeholder: "np. TeamX, GroupA, StreamB",
    vp_playlist: "Zastosuj do playlisty",
    vp_playlist_desc: "Użyj preferencji wersji przy tworzeniu playlisty",
    vp_subtitle: "Preferencje napisów",
    vp_sub_priority: "Priorytet napisów",
    vp_sub_priority_desc: "Dopasuj słowa kluczowe ścieżek napisów po kolei",
    vp_sub_priority_placeholder: "np. Simplified, CHS",
    vp_sub_extract: "Wyodrębnianie napisów między wersjami",
    vp_sub_extract_desc:
        "Wyodrębnij napisy z innych wersji, gdy nie znaleziono ich w bieżącej",
    vp_sub_extract_placeholder: "np. CHS, Simplified",
    vp_limits: "Limity playlisty",
    vp_max_eps: "Maks. odcinków na sesję",
    vp_max_eps_desc:
        "Odcinki są przycinane po osiągnięciu tego limitu; 0 lub puste oznacza brak limitu (zalecane: 10–100)",
    vp_last_ep: "Zatrzymaj na ostatnim odcinku",
    vp_last_ep_desc:
        "Włączone: podczas odtwarzania ostatniego odcinka sezonu lista nie jest tworzona i otwierany jest tylko ten odcinek (nic po nim); Wyłączone: lista jest zawsze tworzona (bieżący + następne odcinki)",
    vp_filter: "Odcisk palca wersji",
    vp_filter_desc:
        "Wyodrębnia cechy wersji ze ścieżki aktualnie odtwarzanego pliku jako «odcisk palca». Do playlisty dodawane są tylko odcinki, których ścieżki zawierają dokładnie ten sam zestaw cech, blokując cały sezon do tej samej wersji. Przykład: jeśli regex pasuje do «TeamX» i «1080p» w bieżącym pliku, uwzględniane są tylko odcinki zawierające oba słowa (puste = wyłączone)",
    vp_filter_placeholder: "np. |TeamX|1080p|CHS",
    vp_filter_valid: "Prawidłowe wyrażenie regularne",
    vp_filter_invalid: "Nieprawidłowe wyrażenie regularne",

    // Network
    page_network: "Sieć",
    net_proxy_http: "Proxy HTTP",
    net_proxy_https: "Proxy HTTPS",
    net_proxy_socks5: "Proxy SOCKS5",
    net_proxy_desc:
        "Wpisz tylko host:port; wklej pełny URL dla automatycznego wykrycia schematu; zostaw puste aby wyłączyć",
    net_proxy_https_desc:
        "Używany dla szyfrowanych połączeń (HTTPS); jeśli pusty, przełącza się na proxy HTTP; ten sam format co HTTP",
    net_proxy_socks5_desc:
        "Przekierowuje cały ruch protokołów; idealny dla sieci bez tunelu HTTP; zostaw puste, aby wyłączyć",
    net_proxy_enabled: "Włącz proxy",
    net_proxy_enabled_desc:
        "Po wyłączeniu URL jest zachowany, ale wszystkie połączenia są bezpośrednie; prywatne adresy IP (192.168.x, 10.x) są zawsze automatycznie pomijane",
    net_skip_tls: "Pomiń weryfikację TLS",
    net_skip_tls_desc:
        "Dla serwerów multimedialnych z certyfikatami samopodpisanymi — niebezpieczne",
    net_redirect: "Wykrywanie przekierowań",
    net_redirect_hosts: "Hosty do sprawdzania przekierowań",
    net_redirect_hosts_desc:
        "Adresy URL strumieni tych hostów są sprawdzane pod kątem przekierowania 30x przed wysłaniem do odtwarzacza (domyślnie puste)",

    // System
    page_system: "System",
    sys_appearance: "Wygląd",
    sys_theme: "Motyw",
    sys_theme_desc: "Jasny, ciemny lub systemowy",
    sys_lang: "Język",
    sys_lang_desc: "Język interfejsu",
    sys_theme_system: "Systemowy",
    sys_theme_light: "Jasny",
    sys_theme_dark: "Ciemny",
    sys_lang_system: "Systemowy",
    sys_display: "Wyświetlacz",
    sys_font_size: "Rozmiar czcionki",
    sys_font_size_desc: "Dostosuj rozmiar tekstu interfejsu",
    sys_zoom: "Powiększenie interfejsu",
    sys_zoom_desc:
        "Ogólne skalowanie dla HiDPI / wysokiej rozdzielczości — bieżący DPR: {dpr}",
    sys_font: "Czcionka interfejsu",
    sys_font_desc: "Wybierz czcionkę interfejsu",
    sys_font_default: "Domyślna (system-ui)",
    sys_startup: "Uruchamianie",
    sys_autostart: "Uruchom przy logowaniu",
    sys_autostart_desc: "Automatycznie uruchamiaj aplikację przy logowaniu do systemu",
    sys_silent_start: "Ciche uruchamianie",
    sys_silent_start_desc:
        "Uruchom ukryte w zasobniku bez wyświetlania głównego okna (ciszej z uruchamianiem przy logowaniu)",
    sys_logs_title: "Dzienniki",
    sys_log_level: "Poziom dziennika",
    sys_log_level_desc:
        "Ustaw Debug, aby uzyskać bardziej szczegółowe dane wyjściowe podczas debugowania",
    sys_log_max_size: "Maks. rozmiar dziennika (MB)",
    sys_log_max_size_desc:
        "Przełącz na nowy plik, gdy bieżący przekroczy ten rozmiar (20–200 MB)",
    sys_log_max_size_capped: "Ograniczono do maksymalnie 200 MB",
    sys_log_max_size_floored: "Podwyższono do minimum 20 MB",
    sys_log_max_files: "Maks. plików dziennika",
    sys_log_max_files_desc:
        "Liczba rotowanych plików dziennika do zachowania (1–14); najstarszy jest usuwany",
    sys_log_max_files_capped: "Ograniczono do maksymalnie 14 plików",
    sys_log_mask: "Maskuj wrażliwe tokeny",
    sys_log_mask_desc: "Zastąp wrażliwy tekst w dziennikach symbolami zastępczymi",
    sys_cache: "Pamięć podręczna",
    sys_cache_size: "Bieżący rozmiar pamięci podręcznej",
    sys_cache_size_desc:
        "Miejsce na dysku używane przez dzienniki i inną pamięć podręczną środowiska wykonawczego",
    sys_cache_clear: "Wyczyść pamięć podręczną",
    sys_cache_clear_desc: "Opróżnij pliki dziennika, aby zwolnić miejsce na dysku",
    cache_confirm_title: "Wyczyść pamięć podręczną",
    cache_confirm_message:
        "Usługa musi być zatrzymana przed wyczyszczeniem pamięci podręcznej, w przeciwnym razie zapisywane dzienniki mogą stać się niespójne. Czy potwierdzasz, że usługa jest zatrzymana i chcesz kontynuować?",
    cache_confirm_ok: "Wyczyść",
    cache_confirm_cancel: "Anuluj",
    cache_stop_first: "Zatrzymaj usługę przed wyczyszczeniem pamięci podręcznej",
    cache_cleared: "Pamięć podręczna wyczyszczona, zwolniono {size}",
    sys_general: "Ogólne",
    sys_about: "O programie",
    sys_about_desc: "Informacje o wersji i podziękowania dla open source",
    sys_download: "Pobieranie",
    sys_speed_limit: "Limit prędkości (MiB/s)",
    sys_speed_limit_desc:
        "Ogranicza przepustowość używaną przez pobieranie i wstępne buforowanie (MiB/s); 0 = bez limitu",
    sys_download_note:
        "Wstępne buforowanie i tryb pobierania są uruchamiane poleceniami skryptu użytkownika przeglądarki, a nie tutaj: 'buforuj podczas odtwarzania' w skrypcie to wstępne buforowanie, a 'tylko pobierz' to tryb pobierania; tryb pobierania wymaga również, aby konto serwera multimediów zezwalało na pobieranie zasobów",
    sys_trakt: "Scrobbling Trakt.tv",
    sys_trakt_sync_note:
        "Po zakończeniu odtwarzania Twoje oglądanie jest automatycznie synchronizowane z Trakt: osiągnięcie około 80% lub więcej oznacza odcinek jako obejrzany, mniej pozostaje nieoznaczone; inne odcinki tego samego sezonu, które ukończyłeś na serwerze multimediów, są również oznaczane, bez duplikowania istniejących. Poniżej 80% Twoja pozycja jest zapamiętywana do wznowienia później, a następny odcinek pojawia się w 'Kontynuuj oglądanie'; ponowne odtworzenie tego samego odcinka zapisuje go ponownie — czy dozwolony jest krótki interwał kontrolowany jest przez przełącznik 'zezwalaj na duplikaty' poniżej.",
    sys_trakt_dashboard: "Otwórz pulpit Trakt",
    sys_trakt_setup_title: "Konfiguracja",
    sys_trakt_setup_step1: "1. Utwórz aplikację na Trakt: ",
    sys_trakt_setup_link: "trakt.tv/oauth/applications",
    sys_trakt_setup_step2: "2. Ustaw 'Redirect uri' aplikacji na poniższy adres:",
    sys_trakt_setup_copy: "Kopiuj",
    sys_trakt_setup_copied: "Redirect URI skopiowany",
    sys_trakt_setup_copy_failed:
        "Kopiowanie nie powiodło się — zaznacz i skopiuj ręcznie",
    sys_trakt_id: "Client ID",
    sys_trakt_id_desc:
        "Uzyskiwany podczas tworzenia aplikacji na trakt.tv — zostaw puste, aby wyłączyć",
    sys_trakt_id_placeholder: "Zostaw puste, aby wyłączyć Trakt",
    sys_trakt_secret: "Client Secret",
    sys_trakt_secret_desc:
        "Uzyskiwany podczas tworzenia aplikacji na trakt.tv — zostaw puste, aby wyłączyć",
    sys_trakt_secret_placeholder: "Zostaw puste, aby wyłączyć Trakt",
    sys_trakt_user: "Nazwa użytkownika",
    sys_trakt_user_desc: "Twoja nazwa użytkownika Trakt (nie wyświetlana nazwa)",
    sys_trakt_user_placeholder: "np. your_trakt_user",
    sys_trakt_host: "Włącz hosta",
    sys_trakt_host_desc:
        'Słowa kluczowe hosta oddzielone przecinkami, puste aby wyłączyć；np. emby.local, 192.168.1；wpisz "." by włączyć wszystkich',
    sys_trakt_host_placeholder: "np. localhost, 192.168., emby.example.com",
    sys_trakt_dup: "Zezwalaj na ponowne oznaczanie",
    sys_trakt_dup_desc:
        "Gdy włączone, każde ukończenie ponownie oznacza ten sam odcinek/film; gdy wyłączone, stosowana jest deduplikacja z ograniczeniem: ten sam element ukończony ponownie w oknie ograniczenia poniżej jest oznaczany tylko raz (wcześniejsze odcinki backfill są zawsze deduplikowane)",
    sys_trakt_dup_throttle: "Ograniczenie ponownego oznaczania (sekundy)",
    sys_trakt_dup_throttle_desc:
        "Aktywne gdy 'Zezwalaj na ponowne oznaczanie' jest wyłączone: ten sam element ukończony ponownie w ciągu tej liczby sekund jest oznaczany tylko raz. Minimum 120 s",
    sys_trakt_dup_throttle_floored:
        "Ograniczenie nie może być mniejsze niż 120 sekund; ustawiono na 120",
    sys_bangumi: "Śledzenie Bangumi.tv",
    sys_bangumi_sync_note:
        "Po zakończeniu odtwarzania Twoje oglądanie jest automatycznie synchronizowane z Bangumi: osiągnięcie ≥ 80% oznacza bieżący odcinek jako obejrzany, poniżej pozostaje nieoznaczony; inne odcinki tego samego sezonu ukończone na serwerze multimediów są również dodawane bez duplikatów. Jeśli nie ma nic do oznaczenia (< 80% i brak historii), dzieło jest ustawiane jako 'oglądam' tylko jeśli efektywny czas odtwarzania wynosi ≥ 20 sekund, w przeciwnym razie jest pomijane, aby zapobiec przypadkowym wpisom.",
    sys_bangumi_host: "Włącz hosta",
    sys_bangumi_host_desc:
        'Słowa kluczowe hosta oddzielone przecinkami, puste aby wyłączyć；np. emby.local, 192.168.1；wpisz "." by włączyć wszystkich',
    sys_bangumi_host_placeholder: "np. localhost, 192.168., emby.example.com",
    sys_bangumi_user: "Nazwa użytkownika / UID",
    sys_bangumi_user_desc: "Nazwa użytkownika bgm.tv lub cyfry w bgm.tv/user/123456",
    sys_bangumi_user_placeholder: "np. 123456",
    sys_bangumi_token: "Token dostępu",
    sys_bangumi_token_desc:
        "Generowany na next.bgm.tv/demo/access-token — zostaw puste, aby wyłączyć",
    sys_bangumi_token_placeholder: "Zostaw puste, aby wyłączyć Bangumi",
    sys_bangumi_private: "Prywatna kolekcja",
    sys_bangumi_private_desc:
        "Ukryj nowo zsynchronizowane wpisy z Twojego publicznego profilu",
    sys_bangumi_genres: "Filtr gatunków",
    sys_bangumi_genres_desc:
        "Wyrażenie regularne dopasowane do gatunków serialu; synchronizowane są tylko pasujące seriale",
    sys_bangumi_genres_placeholder: "动画|anime",
    sys_bangumi_map: "Mapowanie ID",
    sys_bangumi_map_desc:
        "Przypnij serial lub film z tmdb/imdb/tvdb do dokładnego obiektu Bangumi; ma najwyższy priorytet. Trzy formaty sezonu: cały sezon (S4), zamknięty zakres odcinków (S5E1~S5E50, tylko odcinki 1–50), otwarty zakres (S5E51++, od odcinka 51 włącznie). E±N po prawej przesuwa lokalny indeks odcinka na numer sortowania Bangumi. Przykłady: tmdb:10000|type:tv|S4 -> bgm:20000|E+59; tmdb:10000|type:tv|S5E1~S5E50 -> bgm:20001; tmdb:10000|type:tv|S5E51++ -> bgm:20002; tmdb:10001|type:movie -> bgm:30000. Bez type wnioskuje z sezonu (obecność sezonu = TV, inaczej film)",
    map_placeholder: "tmdb:10000|type:tv|S4 -> bgm:20000|E+59",
    map_check: "Sprawdź i dodaj",
    map_remove: "Usuń",
    map_copy: "Kopiuj",
    map_group_add: "Nowa grupa",
    map_group_name_placeholder: "Nazwa grupy",
    map_group_add_confirm: "Utwórz",
    map_group_delete: "Usuń grupę",
    map_group_delete_confirm: 'Usunąć grupę „{name}" i wszystkie jej wpisy?',
    map_item_delete_title: "Usuń wpis",
    map_item_delete_confirm: "Usunąć ten wpis?\n{entry}",
    map_group_default_label: "Domyślna",
    map_export: "Eksportuj",
    map_export_done: "Mapowania wyeksportowane",
    map_import: "Importuj",
    map_import_prefer: "Preferuj importowane (nadpisz lokalne konflikty)",
    map_import_done: "Import zakończony: dodano {added}, zastąpiono {replaced}",
    map_import_url: "Importuj z URL",
    map_import_url_placeholder: "https://example.com/bangumi_map.json",
    map_import_url_confirm: "Importuj",
    cfg_backup_busy: "Tworzenie kopii zapasowej…",
    cfg_importing: "Importowanie…",
    bgm_auto_mark_subject_watched: "Automatyczne oznaczanie jako obejrzane",
    bgm_auto_mark_subject_watched_desc:
        "Automatycznie oznacza całą pozycję jako obejrzaną, gdy wszystkie jej główne odcinki są oznaczone jako obejrzane",
    bgm_mark_watching: "Oznacz jako oglądane",
    bgm_mark_watching_desc:
        "Włączone: częściowe obejrzenie oznacza dzieło jako oglądane. Wyłączone: status aktualizuje się tylko po ukończeniu całego odcinka.",
    map_err_empty: "Wprowadź mapowanie",
    map_err_format: "Nieprawidłowy format — oczekiwano 'LHS -> RHS'",
    map_err_provider: "Nieznany dostawca; obsługiwane są tylko tmdb / imdb / tvdb",
    map_err_provider_id:
        "Nieprawidłowe ID (tmdb/tvdb numeryczne, imdb zaczyna się od tt)",
    map_err_type: "type musi być tv lub movie",
    map_err_season: "Nieprawidłowy sezon; oczekiwana dodatnia liczba całkowita, np. S4",
    map_err_ep_range:
        "Nieprawidłowy zakres odcinków; użyj S5E106~S5E157 (zamknięty) lub S5E158++ (otwarty); początek nie może być większy niż koniec",
    map_err_subject:
        "Nieprawidłowe ID obiektu Bangumi; oczekiwana dodatnia liczba całkowita",
    map_err_offset:
        "Nieprawidłowe przesunięcie odcinków; oczekiwana liczba całkowita, np. E+59 lub E-3",
    map_err_movie_season: "Film nie może mieć przesunięcia sezonu ani odcinka",
    map_err_duplicate: "Identyczne mapowanie już istnieje",
    sync_refresh: "Odśwież autoryzację",
    sync_refreshing: "Odświeżanie…",
    sync_authorize_opened: "Strona autoryzacji została otwarta — ukończ w przeglądarce",
    sync_auth_valid: "Autoryzacja ważna",
    sync_start_service_first: "Najpierw uruchom usługę",
    sync_refresh_confirm_title: "Odśwież autoryzację",
    sync_refresh_confirm_message:
        "Odświeżyć autoryzację ręcznie teraz? Jeśli bieżący token jest nieważny, strona autoryzacji otworzy się w Twojej przeglądarce.",
    sync_refresh_confirm_ok: "Odśwież",
    sync_test: "Testuj autoryzację",
    sync_test_desc: "Sprawdź, czy bieżące dane uwierzytelniające działają",
    sync_testing: "Testowanie…",
    sync_test_ok: "Autoryzacja działa",
    sync_test_fail:
        "Autoryzacja nie powiodła się — konfiguracja może być nieprawidłowa lub nie jest jeszcze autoryzowana. Naciśnij 'Odśwież autoryzację' w prawym górnym rogu.",
    sync_incomplete:
        "Konfiguracja niekompletna — wypełnij wymagane pola przed testowaniem",

    // Config tab
    page_config: "Konfiguracja",
    cfg_file_title: "Plik konfiguracyjny",
    cfg_backup_title: "Kopia zapasowa i przywracanie",
    cfg_backup_now: "Utwórz kopię teraz",
    cfg_backup_now_desc: "Spakuj bieżącą konfigurację do kopii zip ze znacznikiem czasu",
    cfg_backup_done: "Konfiguracja zapisana",
    cfg_backup_list: "Kopie zapasowe",
    cfg_backup_list_desc: "Przechowuje do 5 kopii — obecnie {count}",
    cfg_backup_empty: "Brak kopii zapasowych",
    cfg_view: "Wyświetl",
    cfg_restore: "Przywróć",
    cfg_delete: "Usuń",
    cfg_import: "Importuj kopię",
    cfg_import_desc: "Importuj i przywróć konfigurację z zewnętrznego pliku zip",
    cfg_restore_done: "Konfiguracja przywrócona",
    cfg_restore_confirm_title: "Przywróć konfigurację",
    cfg_restore_confirm_message:
        "Nadpisać bieżącą konfigurację kopią '{name}'? Tej operacji nie można cofnąć.",
    cfg_import_confirm_title: "Importuj i przywróć konfigurację",
    cfg_import_confirm_message:
        "Zaimportować tę kopię i nadpisać bieżącą konfigurację? Tej operacji nie można cofnąć.",
    cfg_delete_confirm_title: "Usuń kopię zapasową",
    cfg_delete_confirm_message: "Usunąć kopię '{name}'?",
    cfg_reset_title: "Resetowanie",
    cfg_reset: "Resetuj do domyślnych",
    cfg_reset_desc: "Przywróć wszystkie ustawienia do wartości domyślnych",
    cfg_reset_done: "Konfiguracja zresetowana do domyślnych",
    cfg_reset_confirm_title: "Resetuj konfigurację",
    cfg_reset_confirm_message:
        "Zresetować do domyślnej konfiguracji? Bieżąca konfiguracja zostanie nadpisana — tej operacji nie można cofnąć.",
    cfg_update_title: "Aktualizacja",
    cfg_update_auto: "Automatycznie sprawdzaj aktualizacje",
    cfg_update_auto_desc:
        "Sprawdzaj nowe wersje na GitHub przy uruchomieniu i wyświetlaj monit w przeglądzie",
    cfg_update_check: "Sprawdź teraz",
    cfg_update_check_desc: "Sprawdź nowszą wersję na GitHub teraz",
    cfg_update_checking: "Sprawdzanie…",
    cfg_update_available: "Znaleziono nową wersję v{version}",
    cfg_update_latest: "Masz najnowszą wersję v{version}",
    cfg_update_current_ver: "Bieżąca wersja",
    cfg_update_latest_ver: "Najnowsza wersja",
    cfg_update_up_to_date: "Aktualna wersja",
    cfg_update_install: "Pobierz i zainstaluj",

    // Update banner
    ov_update_available: "Dostępna nowa wersja v{version}",
    ov_update_action: "Zainstaluj aktualizację",
    ov_update_dismiss: "Pomiń tę wersję",
    ov_update_downloading: "Pobieranie aktualizacji…",
    ov_update_failed: "Błąd aktualizacji",
    sys_privacy: "Prywatność",
    sys_no_progress: "Wyłącz raportowanie postępu",
    sys_no_progress_desc: "Nie zgłaszaj postępu odtwarzania do serwera Emby/Jellyfin",
    sys_accent: "Kolor akcentu",
    sys_accent_desc:
        "Kolor wyróżnienia interfejsu — wpływa na przyciski, aktywne stany i ikony",
    sys_center_nav: "Wyśrodkuj pasek boczny",
    sys_center_nav_desc: "Wyśrodkuj pionowo karty paska bocznego jako grupę",

    // Log levels
    log_error: "Error — tylko awarie",
    log_warn: "Warn — stany anomalne",
    log_info: "Info — normalna, codzienna praca",
    log_debug: "Debug — debugowanie",
    log_trace: "Trace — pełna szczegółowość",

    // Logs page
    page_logs: "Dzienniki",
    logs_app: "Dziennik aplikacji",
    logs_mpv: "Dziennik mpv",
    logs_filter: "Filtruj…",
    logs_clear: "Wyczyść",
    logs_bottom: "↓ Na dół",
    logs_empty: "Oczekiwanie na dane wyjściowe dziennika…",
    logs_no_mpv:
        "Nie znaleziono dziennika mpv — kliknij 'Wybierz dziennik mpv', aby załadować",
    logs_lines: "wierszy",
    logs_loading_older: "Ładowanie starszych dzienników…",
    logs_scroll_older: "Przewiń w górę, aby załadować starsze dzienniki",
    logs_open_folder: "Otwórz folder dzienników",
    logs_pick_mpv: "Wybierz dziennik mpv",
    logs_reset_mpv: "Resetuj do domyślnego",
    logs_reset_mpv_title: "Wróć do domyślnego dziennika mpv w folderze dzienników",
    logs_anon: "Anonimowo",
    logs_anon_title:
        "Ukrywa identyfikatory urządzeń, tokeny, IP, identyfikatory użytkowników, adresy URL hostów i nazwy użytkowników Bangumi / Trakt tylko w widoku, wygodne do udostępniania zrzutów ekranu; plik dziennika nie jest zmieniany — cenzurowanie pliku jest nadal kontrolowane przez przełącznik 'Wrażliwy tekst'",

    // About modal
    about_thanks: "Podziękowania",
    about_thanks_desc: "za nieskończoną inspirację",
    about_version_label: "Wersja",

    // Autostart toasts
    autostart_on: "Uruchamianie przy logowaniu włączone",
    autostart_off: "Uruchamianie przy logowaniu wyłączone",

    // Font size options
    font_12: "12px (kompaktowy)",
    font_13: "13px (domyślny)",
    font_14: "14px (wygodny)",
    font_15: "15px (duży)",
    font_16: "16px (bardzo duży)",

    // Download
    page_download: "Pobieranie",
    dl_folder: "Folder pobierania",
    dl_folder_desc: "Pozostaw puste dla systemowego folderu domyślnego",
    dl_browse: "Przeglądaj…",
    dl_placeholder: "",
    dl_path_error: "Ścieżka nie istnieje, sprawdź dane wejściowe",

    // Bangumi duplicate throttle
    sys_bangumi_dup: "Zezwól na zduplikowane oznaczenia",
    sys_bangumi_dup_desc:
        "Po włączeniu ponownie oznacza ten sam odcinek/film za każdym razem, gdy skończysz go oglądać; po wyłączeniu aktywuje deduplikację z ograniczeniem: ten sam wpis jest oznaczany tylko raz w oknie ograniczenia ustawionym poniżej",
    sys_bangumi_dup_throttle: "Czas ograniczenia zduplikowanych oznaczeń (sekundy)",
    sys_bangumi_dup_throttle_desc:
        "Aktywny gdy Zezwól na zduplikowane oznaczenia jest wyłączone: ten sam wpis jest zapisywany co najwyżej raz w ciągu tej liczby sekund; minimum 120 sekund",
    sys_bangumi_dup_throttle_floored:
        "Ograniczenie nie może być mniejsze niż 120 s — poprawione do 120",

    // TMDB
    sys_tmdb: "Integracja TMDB",
    sys_tmdb_key: "Klucz API",
    sys_tmdb_key_desc:
        "Klucz API TMDB do pobierania brakujących metadanych z serwera mediów podczas synchronizacji.",
    sys_tmdb_api_link: "Utwórz klucz API",
    sys_tmdb_key_placeholder: "",
};
