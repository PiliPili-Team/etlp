import { zhCN } from "./zh-CN";

export const sr: typeof zhCN = {
    ...zhCN,

    // App
    app_name: "Genshin",

    // Nav
    nav_overview: "Преглед",
    nav_player: "Плејер",
    nav_version_prefer: "Верзија",
    nav_network: "Мрежа",
    nav_config: "Конфигурација",
    nav_system: "Систем",
    nav_logs: "Логови",
    nav_sec_play: "Репродукција",
    nav_sec_settings: "Подешавања",
    nav_sec_sync: "Синхронизација",
    nav_bangumi: "Bangumi",
    nav_trakt: "Trakt",
    nav_sec_debug: "Отклањање грешака",
    nav_download: "Преузимања",

    // Common
    add: "Додај",
    add_placeholder: "Унесите и притисните Enter за додавање",
    open_dir: "Отвори директоријум",
    loading: "Учитавање конфигурације…",

    // Overview
    page_overview: "Преглед",
    ov_service: "Локални сервис",
    ov_running: "Покренут",
    ov_stopped: "Заустављен",
    ov_port: "Порт",
    ov_port_desc: "Локална адреса за слушање",
    ov_uptime: "Време рада",
    ov_uptime_desc: "Од покретања сервиса",
    ov_address: "Адреса",
    ov_address_desc: "Само localhost",
    ov_config: "Подешавање",
    ov_config_file: "Конфигурациони фајл",
    ov_config_file_desc: "Прегледај или отвори у спољном едитору",
    ov_edit_config: "Уреди конфигурацију",
    ov_restart: "Поново покрени сервис",
    ov_restart_desc:
        "Заустави сервис, ослободи ресурсе и поново покрени са најновијом конфигурацијом",
    ov_about: "О програму",
    ov_about_desc: "Информације о верзији и захвалнице за отворени код",
    ov_view: "Прикажи",
    ov_start: "Покрени",
    ov_stop: "Заустави",

    // Toasts
    toast_started: "Сервис покренут на порту {port}",
    toast_stopped: "Сервис заустављен",
    toast_restarted: "Сервис поново покренут на порту {port}",
    toast_start_failed: "Покретање сервиса није успело",
    toast_stop_failed: "Заустављање сервиса није успело",
    toast_restart_failed: "Поновно покретање сервиса није успело",
    toast_open_failed: "Отварање није успело",
    sync_not_configured: "Још није подешено — прво попуните поља",

    // Player
    page_player: "Плејер",
    pl_type: "Тип плејера",
    pl_type_desc: "Изаберите локални медија плејер",
    pl_startup: "Опције покретања",
    pl_fullscreen: "Пун екран",
    pl_fullscreen_desc: "Покрени плејер у режиму пуног екрана",
    pl_mute: "Покрени без звука",
    pl_mute_desc: "Покрени са утишаним звуком (mpv --mute=yes)",
    pl_pretty_title: "Леп наслов",
    pl_pretty_title_desc: "Додај назив сервера на почетак наслова прозора плејера",
    pl_kill_start: "Заустави при покретању",
    pl_kill_start_desc: "Заустави постојеће процесе плејера при покретању",
    pl_path: "Путања плејера",
    pl_path_desc: "Опционално — оставите празно за употребу плејера из системског PATH",
    pl_browse: "Прегледај…",
    pl_path_error: "Путања није пронађена — проверите унос",
    pl_progress_support:
        "Извештавање о напретку: mpv / IINA су потпуно подржани — ажурирања у реалном времену током репродукције, бележење позиције за наставак при изласку, означавање као одгледано, синхронизација са Trakt / Bangumi и праћење по епизодама. Остали плејери бележе само финалну позицију и синхронизују при изласку, без извештавања у реалном времену; VLC репродукује цео сезон, MPC и dandanplay — за једну епизоду, а читање позиције у PotPlayer-у ради само на Windows-у",

    // Version prefer
    page_vp: "Предност верзије",
    vp_priority: "Редослед приоритета верзија",
    vp_keywords: "Ознаке верзије",
    vp_keywords_desc:
        "Када за исту епизоду постоји више датотека, бира се она чија путања одговара ознаци највише у листи. Пример: «TeamX → GroupA → StreamB» — ако су доступне све три верзије, бира се TeamX; ако не — GroupA; итако даље",
    vp_keywords_placeholder: "нпр. TeamX, GroupA, StreamB",
    vp_playlist: "Примени на листу за репродукцију",
    vp_playlist_desc: "Користи предност верзије при прављењу листе за репродукцију",
    vp_subtitle: "Предност титлова",
    vp_sub_priority: "Приоритет титлова",
    vp_sub_priority_desc: "Упоредите кључне речи стаза титлова по редоследу",
    vp_sub_priority_placeholder: "нпр. Simplified, CHS",
    vp_sub_extract: "Извлачење титлова из других верзија",
    vp_sub_extract_desc: "Извлачи титлове из других верзија кад у тренутној нема",
    vp_sub_extract_placeholder: "нпр. CHS, Simplified",
    vp_limits: "Ограничења листе за репродукцију",
    vp_max_eps: "Макс. епизода по сесији",
    vp_max_eps_desc:
        "Епизоде се сечу кад се достигне овај лимит; 0 или празно значи без ограничења (препорука: 10–100)",
    vp_last_ep: "Заустави на последњој епизоди",
    vp_last_ep_desc:
        "Укључено: при репродукцији последње епизоде сезоне листа се не прави и отвара се само та епизода (нема следеће); Искључено: листа се увек прави (тренутна + следеће епизоде)",
    vp_filter: "Отисак верзије",
    vp_filter_desc:
        "Извлачи карактеристике верзије из путање тренутно пуштане датотеке као «отисак». У листу репродукције додају се само епизоде чије путање садрже потпуно исти скуп карактеристика, закључавајући целу сезону на исту верзију. Пример: ако regex одговара «TeamX» и «1080p» у тренутној датотеци, укључују се само епизоде које садрже обе речи (празно = искључено)",
    vp_filter_placeholder: "нпр. |TeamX|1080p|CHS",
    vp_filter_valid: "Важећи регуларни израз",
    vp_filter_invalid: "Неважећи регуларни израз",

    // Network
    page_network: "Мрежа",
    net_proxy_http: "HTTP прокси",
    net_proxy_https: "HTTPS прокси",
    net_proxy_socks5: "SOCKS5 прокси",
    net_proxy_desc:
        "Само host:port; налепите пун URL за аутоматско откривање схеме; оставите празно за онемогућавање",
    net_proxy_https_desc:
        "Користи се за шифроване (HTTPS) везе; ако је празно, враћа се на HTTP прокси; исти формат као HTTP",
    net_proxy_socks5_desc:
        "Проксира сав протоколни саобраћај; идеално за мреже без HTTP тунела; оставите празно за онемогућавање",
    net_proxy_enabled: "Омогући прокси",
    net_proxy_enabled_desc:
        "Када је искључено, URL се чува али су све конекције директне; приватне IP адресе (192.168.x, 10.x) увек аутоматски заобилазе прокси",
    net_skip_tls: "Прескочи TLS верификацију",
    net_skip_tls_desc: "За медијске сервере са самопотписаним сертификатима — небезбедно",
    net_redirect: "Откривање преусмеравања",
    net_redirect_hosts: "Хостови за проверу преусмеравања",
    net_redirect_hosts_desc:
        "URL-ови стримова ових хостова проверавају се на 30x преусмеравање пре него што се пошаљу плејеру (подразумевано празно)",

    // System
    page_system: "Систем",
    sys_appearance: "Изглед",
    sys_theme: "Тема",
    sys_theme_desc: "Светла, тамна или по систему",
    sys_lang: "Језик",
    sys_lang_desc: "Језик интерфејса",
    sys_theme_system: "Системска",
    sys_theme_light: "Светла",
    sys_theme_dark: "Тамна",
    sys_lang_system: "Системски",
    sys_display: "Екран",
    sys_font_size: "Величина фонта",
    sys_font_size_desc: "Подеси величину текста интерфејса",
    sys_zoom: "Зум интерфејса",
    sys_zoom_desc: "Опште скалирање за HiDPI / висока резолуција — тренутни DPR: {dpr}",
    sys_font: "Фонт интерфејса",
    sys_font_desc: "Изаберите фонт интерфејса",
    sys_font_default: "Подразумевани (system-ui)",
    sys_startup: "Покретање",
    sys_autostart: "Покрени при пријави",
    sys_autostart_desc: "Аутоматски покрени апликацију при пријави на систем",
    sys_silent_start: "Тихо покретање",
    sys_silent_start_desc:
        "Покрени скривено у системској касети без приказивања главног прозора (тишe са покретањем при пријави)",
    sys_service: "Local Service",
    sys_listen_port: "Listen Port",
    sys_listen_port_desc:
        "Port used by the browser userscript to reach the local service. Changing it restarts the service automatically and must match the userscript port.",
    sys_listen_port_invalid: "Port must be between 1 and 65535; corrected automatically",
    sys_logs_title: "Логови",
    sys_log_level: "Ниво лога",
    sys_log_level_desc: "Поставите Debug за детаљнији испис при отклањању грешака",
    sys_log_max_size: "Макс. величина лога (МБ)",
    sys_log_max_size_desc:
        "Прелазак на нови фајл кад тренутни прекорачи ову величину (20–200 МБ)",
    sys_log_max_size_capped: "Ограничено на максимум од 200 МБ",
    sys_log_max_size_floored: "Повећано на минимум од 20 МБ",
    sys_log_max_files: "Макс. фајлова лога",
    sys_log_max_files_desc:
        "Број ротираних фајлова лога за чување (1–14); најстарији се брише",
    sys_log_max_files_capped: "Ограничено на максимум од 14 фајлова",
    sys_log_mask: "Маскирај осетљиве токене",
    sys_log_mask_desc: "Замени осетљиви текст у логовима заменским знаковима",
    sys_cache: "Кеш",
    sys_cache_size: "Тренутна величина кеша",
    sys_cache_size_desc:
        "Простор на диску који користе логови и остали кеш за извршавање",
    sys_cache_clear: "Очисти кеш",
    sys_cache_clear_desc: "Испразни фајлове лога ради ослобађања простора на диску",
    cache_confirm_title: "Очисти кеш",
    cache_confirm_message:
        "Сервис мора бити заустављен пре чишћења кеша, иначе логови који се пишу могу постати неконзистентни. Потврђујете да је сервис заустављен и желите да наставите?",
    cache_confirm_ok: "Очисти",
    cache_confirm_cancel: "Откажи",
    cache_stop_first: "Зауставите сервис пре чишћења кеша",
    cache_cleared: "Кеш очишћен, ослобођено {size}",
    sys_general: "Опште",
    sys_about: "О програму",
    sys_about_desc: "Информације о верзији и захвалнице за отворени код",
    sys_download: "Преузимање",
    sys_speed_limit: "Ограничење брзине (МиБ/с)",
    sys_speed_limit_desc:
        "Ограничава пропусни опсег преузимања и предкеширања (МиБ/с); 0 = без ограничења",
    sys_download_note:
        "Предкеширање и режим преузимања покрећу се командама корисничке скрипте прегледача, а не овде: 'кешируј током репродукције' у скрипти је предкеширање, а 'само преузми' је режим преузимања; режим преузимања захтева да ваш налог медија сервера дозвољава преузимање средстава",
    sys_trakt: "Trakt.tv скробловање",
    sys_trakt_sync_note:
        "Кад се репродукција заврши, ваше гледање се аутоматски синхронизује са Trakt-ом: достизање приближно 80 % или више означава епизоду као одгледану, мање остаје необележено; остале епизоде истог сезона које сте завршили на медија серверу такође се означавају, без дупликата постојећих. Испод 80 % ваша позиција се памти за наставак касније, а следећа епизода се појављује у 'Nastavi gledanje'; поновна репродукција исте епизоде бележи је поново — да ли је кратак интервал дозвољен контролише прекидач 'дозволи дупликате' испод.",
    sys_trakt_dashboard: "Отвори Trakt таблу",
    sys_trakt_setup_title: "Подешавање",
    sys_trakt_setup_step1: "1. Направите апликацију на Trakt-у: ",
    sys_trakt_setup_link: "trakt.tv/oauth/applications",
    sys_trakt_setup_step2: "2. Поставите 'Redirect uri' апликације на адресу испод:",
    sys_trakt_setup_copy: "Копирај",
    sys_trakt_setup_copied: "URI преусмеравања је копиран",
    sys_trakt_setup_copy_failed: "Копирање није успело — означите и копирајте ручно",
    sys_trakt_id: "Client ID",
    sys_trakt_id_desc:
        "Добављено при прављењу апликације на trakt.tv — оставите празно за онемогућавање",
    sys_trakt_id_placeholder: "Оставите празно за онемогућавање Trakt-а",
    sys_trakt_secret: "Client Secret",
    sys_trakt_secret_desc:
        "Добављено при прављењу апликације на trakt.tv — оставите празно за онемогућавање",
    sys_trakt_secret_placeholder: "Оставите празно за онемогућавање Trakt-а",
    sys_trakt_user: "Корисничко име",
    sys_trakt_user_desc: "Ваше Trakt корисничко ime (а не приказно надимак)",
    sys_trakt_user_placeholder: "нпр. your_trakt_user",
    sys_trakt_host: "Укључи хост",
    sys_trakt_host_desc:
        'Кључне речи хоста раздвојене зарезом, оставите празно за онемогућавање；нпр. emby.local, 192.168.1；унесите "." за укључивање свих',
    sys_trakt_host_placeholder: "нпр. localhost, 192.168., emby.example.com",
    sys_trakt_dup: "Дозволи поновно означавање",
    sys_trakt_dup_desc:
        "Кад је укључено, свако завршавање поново означава исту епизоду/филм; кад је искључено, примењује се дедупликација са ограничењем: исти ставку завршена поново у оквиру прозора ограничења испод бележи се само једном (ранији backfill епизоди увек се дедуплицирају)",
    sys_trakt_dup_throttle: "Ограничење поновног означавања (секунде)",
    sys_trakt_dup_throttle_desc:
        "Активно кад је 'Дозволи поновно означавање' искључено: исти ставку завршена поново у оквиру овог броја секунди бележи се само једном. Минимум 120 с",
    sys_trakt_dup_throttle_floored:
        "Ограничење не може бити мање од 120 секунди; постављено на 120",
    sys_bangumi: "Bangumi.tv праћење",
    sys_bangumi_sync_note:
        "Кад се репродукција заврши, ваше гледање се аутоматски синхронизује са Bangumi: достизање ≥ 80 % означава тренутну епизоду као одгледану, мање остаје необележено; остале епизоде истог сезона завршене на медија серверу такође се додају без дупликата. Ако нема шта да се означи (< 80 % и без историје), дело се поставља у стање 'гледам' само ако је ефективно трајање гледања ≥ 20 секунди, иначе се прескаче да би се избегло случајно додавање.",
    sys_bangumi_host: "Укључи хост",
    sys_bangumi_host_desc:
        'Кључне речи хоста раздвојене зарезом, оставите празно за онемогућавање；нпр. emby.local, 192.168.1；унесите "." за укључивање свих',
    sys_bangumi_host_placeholder: "нпр. localhost, 192.168., emby.example.com",
    sys_bangumi_user: "Корисничко ime / UID",
    sys_bangumi_user_desc: "Корисничко ime bgm.tv или бројеви у bgm.tv/user/123456",
    sys_bangumi_user_placeholder: "нпр. 123456",
    sys_bangumi_token: "Приступни токен",
    sys_bangumi_token_desc:
        "Генерисан на next.bgm.tv/demo/access-token — оставите празно за онемогућавање",
    sys_bangumi_token_placeholder: "Оставите празно за онемогућавање Bangumi",
    sys_bangumi_private: "Приватна колекција",
    sys_bangumi_private_desc: "Сакриј новосинхронизоване уносе са вашег јавног профила",
    sys_bangumi_genres: "Филтер жанрова",
    sys_bangumi_genres_desc:
        "Регуларни израз упарен са жанровима серијала; синхронизују се само одговарајући серијали",
    sys_bangumi_genres_placeholder: "动画|anime",
    sys_bangumi_map: "Мапирање ID-јева",
    sys_bangumi_map_desc:
        "Закачи серијал или филм tmdb/imdb/tvdb за тачан Bangumi субјекат; има највиши приоритет. Три формата сезоне: цела сезона (S4), затворен опсег епизода (S5E1~S5E50, само епизоде 1–50), отворен опсег (S5E51++, од епизоде 51 надаље). E±N с десне стране помера локални индекс епизоде на редни број Bangumi. Примери: tmdb:10000|type:tv|S4 -> bgm:20000|E+59; tmdb:10000|type:tv|S5E1~S5E50 -> bgm:20001; tmdb:10000|type:tv|S5E51++ -> bgm:20002; tmdb:10001|type:movie -> bgm:30000. Без type закључује се из сезоне (присуство сезоне значи TV, иначе филм)",
    map_placeholder: "tmdb:10000|type:tv|S4 -> bgm:20000|E+59",
    map_check: "Провери и додај",
    map_remove: "Уклони",
    map_copy: "Копирај",
    map_group_add: "Нова група",
    map_group_name_placeholder: "Назив групе",
    map_group_add_confirm: "Креирај",
    map_group_delete: "Уклони групу",
    map_group_delete_confirm: 'Уклонити групу „{name}" и све њене уносе?',
    map_item_delete_title: "Уклони везу",
    map_item_delete_confirm: "Уклонити овај унос?\n{entry}",
    map_group_default_label: "Подразумевано",
    map_export: "Извези",
    map_export_done: "Мапирања извезена",
    map_import: "Увези",
    map_import_prefer: "Предност увоза (преписи локалне конфликте)",
    map_import_done: "Увоз завршен: додато {added}, замењено {replaced}",
    map_import_url: "Увези са URL",
    map_import_url_placeholder: "https://example.com/bangumi_map.json",
    map_import_url_confirm: "Увези",
    cfg_backup_busy: "Прављење резервне копије…",
    cfg_importing: "Увожење…",
    bgm_auto_mark_subject_watched: "Аутоматски означи као одгледано",
    bgm_auto_mark_subject_watched_desc:
        "Аутоматски означава цео унос као одгледан када су све његове главне епизоде означене као одгледане",
    bgm_history_follow_media_server: "Историја прати медијски сервер",
    bgm_history_follow_media_server_desc:
        "Када сезона медијског сервера одговара већем броју Bangumi колекција, допуњавају се и раније колекције које сервер означава као одгледане. Када је искључено, допуњава се само колекција епизоде коју гледате.",
    bgm_mark_watching: "Означи као у гледању",
    bgm_mark_watching_desc:
        "Укључено: делимично гледање означава дело као у гледању. Искључено: статус се ажурира само после потпуно одгледане епизоде.",
    map_err_empty: "Унесите мапирање",
    map_err_format: "Погрешан формат — очекује се 'LHS -> RHS'",
    map_err_provider: "Непознати провајдер; подржани су само tmdb / imdb / tvdb",
    map_err_provider_id: "Погрешан ID (tmdb/tvdb нумерички, imdb почиње са tt)",
    map_err_type: "type мора бити tv или movie",
    map_err_season: "Погрешна сезона; очекује се позитиван цео број, нпр. S4",
    map_err_ep_range:
        "Погрешан опсег епизода; користите S5E106~S5E157 (затворен) или S5E158++ (отворен); почетак не може бити већи од краја",
    map_err_subject: "Погрешан ID Bangumi субјекта; очекује се позитиван цео број",
    map_err_offset: "Погрешан помак епизода; очекује се цео број, нпр. E+59 или E-3",
    map_err_movie_season: "Филм не може имати помак сезоне или епизоде",
    map_err_duplicate: "Идентично мапирање већ постоји",
    sync_refresh: "Освежи ауторизацију",
    sync_refreshing: "Освежавање…",
    sync_authorize_opened: "Страница ауторизације је отворена — завршите у прегледачу",
    sync_auth_valid: "Ауторизација је важећа",
    sync_start_service_first: "Прво покрените сервис",
    sync_refresh_confirm_title: "Освежи ауторизацију",
    sync_refresh_confirm_message:
        "Освежити ауторизацију ручно сада? Ако је тренутни токен неважећи, страница ауторизације ће се отворити у прегледачу.",
    sync_refresh_confirm_ok: "Освежи",
    sync_test: "Тест ауторизације",
    sync_test_desc: "Провери да ли тренутне акредитиве функционишу",
    sync_testing: "Тестирање…",
    sync_test_ok: "Ауторизација ради",
    sync_test_fail:
        "Ауторизација није успела — конфигурација може бити погрешна или још није ауторизована. Притисните 'Освежи ауторизацију' у горњем десном углу.",
    sync_incomplete: "Конфигурација је непотпуна — попуните обавезна поља пре тестирања",

    // Config tab
    page_config: "Конфигурација",
    cfg_file_title: "Конфигурациони фајл",
    cfg_backup_title: "Резервна копија и враћање",
    cfg_backup_now: "Направи копију сада",
    cfg_backup_now_desc:
        "Упакуј тренутну конфигурацију у zip копију са временском ознаком",
    cfg_backup_done: "Конфигурација је сачувана",
    cfg_backup_list: "Резервне копије",
    cfg_backup_list_desc: "Чува до 5 копија — тренутно {count}",
    cfg_backup_empty: "Нема резервних копија",
    cfg_view: "Прикажи",
    cfg_restore: "Врати",
    cfg_delete: "Обриши",
    cfg_import: "Увези копију",
    cfg_import_desc: "Увези и врати конфигурацију из спољног zip фајла",
    cfg_restore_done: "Конфигурација враћена",
    cfg_restore_confirm_title: "Врати конфигурацију",
    cfg_restore_confirm_message:
        "Преписати тренутну конфигурацију копијом '{name}'? Ово се не може поништити.",
    cfg_import_confirm_title: "Увези и врати конфигурацију",
    cfg_import_confirm_message:
        "Увести ову копију и преписати тренутну конфигурацију? Ово се не може поништити.",
    cfg_delete_confirm_title: "Обриши резервну копију",
    cfg_delete_confirm_message: "Обрисати копију '{name}'?",
    cfg_reset_title: "Ресетовање",
    cfg_reset: "Ресетуј на подразумевано",
    cfg_reset_desc: "Врати сва подешавања на подразумеване вредности",
    cfg_reset_done: "Конфигурација ресетована на подразумевано",
    cfg_reset_confirm_title: "Ресетуј конфигурацију",
    cfg_reset_confirm_message:
        "Ресетовати на подразумевану конфигурацију? Тренутна конфигурација ће бити преписана — ово се не може поништити.",
    cfg_update_title: "Ажурирање",
    cfg_update_auto: "Аутоматски провери ажурирања",
    cfg_update_auto_desc:
        "Провери нова издања на GitHub-у при покретању и прикажи обавештење у прегледу",
    cfg_update_check: "Провери сада",
    cfg_update_check_desc: "Провери новију верзију на GitHub-у одмах",
    cfg_update_checking: "Провера…",
    cfg_update_available: "Пронађена нова верзија v{version}",
    cfg_update_latest: "Имате најновију верзију v{version}",
    cfg_update_current_ver: "Тренутна верзија",
    cfg_update_latest_ver: "Последња верзија",
    cfg_update_up_to_date: "Актуелна верзија",
    cfg_update_install: "Преузми и инсталирај",

    // Update banner
    ov_update_available: "Доступна је нова верзија v{version}",
    ov_update_action: "Инсталирај ажурирање",
    ov_update_dismiss: "Прескочи ову верзију",
    ov_update_downloading: "Преузимање ажурирања…",
    ov_update_failed: "Ажурирање неуспешно",
    ov_update_extracting: "Распакивање ажурирања…",
    ov_update_installing: "Инсталирање нове верзије…",
    sys_privacy: "Приватност",
    sys_no_progress: "Онемогући извештавање о напретку",
    sys_no_progress_desc: "Не пријављуј напредак репродукције Emby/Jellyfin серверу",
    sys_accent: "Боја акцента",
    sys_accent_desc: "Боја истицања интерфејса — утиче на дугмад, активна стања и иконе",
    sys_center_nav: "Центрирај бочну траку",
    sys_center_nav_desc: "Вертикално центрирај картице бочне траке као групу",

    // Log levels
    log_error: "Error — само грешке",
    log_warn: "Warn — аномална стања",
    log_info: "Info — нормалне, свакодневне операције",
    log_debug: "Debug — отклањање грешака",
    log_trace: "Trace — пуна детаљност",

    // Logs page
    page_logs: "Логови",
    logs_app: "Лог апликације",
    logs_mpv: "mpv лог",
    logs_filter: "Филтрирај…",
    logs_clear: "Очисти",
    logs_bottom: "↓ На дно",
    logs_empty: "Чека се испис лога…",
    logs_no_mpv: "mpv лог није пронађен — кликните 'Изабери mpv лог' за учитавање",
    logs_lines: "редова",
    logs_loading_older: "Учитавање старијих логова…",
    logs_scroll_older: "Померите нагоре за учитавање старијих логова",
    logs_open_folder: "Отвори фасциклу логова",
    logs_pick_mpv: "Изабери mpv лог",
    logs_reset_mpv: "Ресетуј на подразумевано",
    logs_reset_mpv_title: "Врати се на подразумевани mpv лог у фасцикли логова",
    logs_anon: "Анонимно",
    logs_anon_title:
        "Сакрива ID уређаја, токене, IP адресе, корисничке ID-јеве, хост URL-ове и Bangumi / Trakt корисничка имена само у приказу, згодно за дељење снимака екрана; лог фајл се не мења — цензурисање фајла и даље се управља прекидачем 'Осетљиви текст'",

    // About modal
    about_thanks: "Захвалнице",
    about_thanks_desc: "за бескрајну инспирацију",
    about_version_label: "Верзија",

    // Autostart toasts
    autostart_on: "Покретање при пријави је укључено",
    autostart_off: "Покретање при пријави је искључено",

    // Font size options
    font_12: "12px (компактно)",
    font_13: "13px (подразумевано)",
    font_14: "14px (удобно)",
    font_15: "15px (велико)",
    font_16: "16px (веома велико)",

    // Download
    page_download: "Преузимања",
    dl_folder: "Фасцикла за преузимање",
    dl_folder_desc: "Оставите празно за подразумевани директоријум система",
    dl_browse: "Прегледај…",
    dl_placeholder: "",
    dl_path_error: "Путања не постоји, проверите унос",

    // Bangumi duplicate throttle
    sys_bangumi_dup: "Дозволи дупликат ознаке",
    sys_bangumi_dup_desc:
        "Када је укључено, поново означава исту епизоду/филм сваки пут када завршите са гледањем; када је искључено, активира се дедупликација са ограничењем: иста ставка се означава само једном у оквиру прозора ограничења постављеног испод",
    sys_bangumi_dup_throttle: "Време ограничења дупликат ознака (секунде)",
    sys_bangumi_dup_throttle_desc:
        "Активно када је Дозволи дупликат ознаке искључено: иста ставка се бележи највише једном у оквиру овог броја секунди; минимум 120 секунди",
    sys_bangumi_dup_throttle_floored:
        "Ограничење не може бити мање од 120 с — исправљено на 120",

    // TMDB
    sys_tmdb: "TMDB интеграција",
    sys_tmdb_key: "API кључ",
    sys_tmdb_key_desc:
        "TMDB API кључ за преузимање метаподатака који недостају на медијском серверу током синхронизације.",
    sys_tmdb_api_link: "Направите API кључ",
    sys_tmdb_key_placeholder: "",
};
