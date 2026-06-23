import { zhCN } from "./zh-CN";

export const it: typeof zhCN = {
    ...zhCN,

    // App
    app_name: "Genshin",

    // Nav
    nav_overview: "Panoramica",
    nav_player: "Lettore",
    nav_version_prefer: "Versione",
    nav_network: "Rete",
    nav_config: "Configurazione",
    nav_system: "Sistema",
    nav_logs: "Log",
    nav_sec_play: "Riproduzione",
    nav_sec_settings: "Impostazioni",
    nav_sec_sync: "Sincronizzazione",
    nav_bangumi: "Bangumi",
    nav_trakt: "Trakt",
    nav_sec_debug: "Debug",

    // Common
    add: "Aggiungi",
    add_placeholder: "Digita e premi Invio per aggiungere",
    open_dir: "Apri cartella",
    loading: "Caricamento configurazione…",

    // Overview
    page_overview: "Panoramica",
    ov_service: "Servizio locale",
    ov_running: "In esecuzione",
    ov_stopped: "Arrestato",
    ov_port: "Porta",
    ov_port_desc: "Indirizzo di ascolto locale",
    ov_uptime: "Tempo di attività",
    ov_uptime_desc: "Dall'avvio del servizio",
    ov_address: "Indirizzo",
    ov_address_desc: "Solo localhost",
    ov_config: "Impostazioni",
    ov_config_file: "File di configurazione",
    ov_config_file_desc: "Visualizza o apri in un editor esterno",
    ov_edit_config: "Modifica configurazione",
    ov_restart: "Riavvia servizio",
    ov_restart_desc:
        "Arresta il servizio, libera le risorse e riavvia con la configurazione più recente",
    ov_about: "Informazioni",
    ov_about_desc: "Informazioni sulla versione e crediti open source",
    ov_view: "Visualizza",
    ov_start: "Avvia",
    ov_stop: "Arresta",

    // Toasts
    toast_started: "Servizio avviato sulla porta {port}",
    toast_stopped: "Servizio arrestato",
    toast_restarted: "Servizio riavviato sulla porta {port}",
    toast_start_failed: "Impossibile avviare il servizio",
    toast_stop_failed: "Impossibile arrestare il servizio",
    toast_restart_failed: "Impossibile riavviare il servizio",
    toast_open_failed: "Apertura non riuscita",
    sync_not_configured: "Non ancora configurato — compila prima i campi",

    // Player
    page_player: "Lettore",
    pl_type: "Tipo di lettore",
    pl_type_desc: "Scegli un lettore multimediale locale",
    pl_startup: "Opzioni di avvio",
    pl_fullscreen: "Schermo intero",
    pl_fullscreen_desc: "Avvia il lettore in modalità schermo intero",
    pl_mute: "Avvia muto",
    pl_mute_desc: "Avvia con l'audio disabilitato (mpv --no-audio)",
    pl_pretty_title: "Titolo elegante",
    pl_pretty_title_desc:
        "Anteponi il nome del server al titolo della finestra del lettore",
    pl_kill_start: "Termina all'avvio",
    pl_kill_start_desc: "Termina i processi del lettore esistenti all'avvio",
    pl_path: "Percorso del lettore",
    pl_path_desc: "Facoltativo — lascia vuoto per usare il lettore dal PATH di sistema",
    pl_browse: "Sfoglia…",
    pl_path_error: "Percorso non trovato — controlla l'inserimento",
    pl_progress_support:
        "Segnalazione progresso: mpv / IINA sono pienamente supportati — aggiornamenti in tempo reale durante la riproduzione, posizione di ripresa riscritta all'uscita, contrassegno come visto, sincronizzazione Trakt / Bangumi e tracciamento per episodio. Gli altri lettori scrivono solo la posizione finale e sincronizzano all'uscita, senza segnalazione in tempo reale durante la riproduzione; VLC riproduce l'intera stagione di seguito, MPC e dandanplay sono a singolo episodio e la rilettura della posizione di PotPlayer è solo per Windows",

    // Version prefer
    page_vp: "Preferenze versione",
    vp_priority: "Priorità versione",
    vp_keywords: "Parole chiave versione",
    vp_keywords_desc:
        "Abbina le parole chiave di versione del file in ordine — vincono le voci precedenti",
    vp_keywords_placeholder: "es. VCB-Studio, ANi, DBD-Raws",
    vp_playlist: "Applica alla playlist",
    vp_playlist_desc: "Usa la priorità di versione durante la creazione della playlist",
    vp_subtitle: "Preferenze sottotitoli",
    vp_sub_priority: "Priorità sottotitoli",
    vp_sub_priority_desc:
        "Abbina le parole chiave delle tracce dei sottotitoli in ordine",
    vp_sub_priority_placeholder: "es. Semplificato, CHS",
    vp_sub_extract: "Estrazione sottotitoli tra versioni",
    vp_sub_extract_desc:
        "Estrai i sottotitoli da altre versioni quando non ne vengono trovati nella corrente",
    vp_sub_extract_placeholder: "es. CHS, Semplificato",
    vp_limits: "Limiti playlist",
    vp_max_eps: "Episodi massimi per sessione",
    vp_max_eps_desc:
        "Gli episodi vengono troncati al raggiungimento di questo limite; 0 o vuoto significa illimitato (consigliato: 10–100)",
    vp_last_ep: "Disattiva all'ultimo episodio",
    vp_last_ep_desc:
        "Attivo: riproducendo l'ultimo episodio della stagione, non crea alcuna playlist e apre solo quell'episodio (nulla lo segue); Disattivo: crea sempre la playlist (episodio corrente + successivi)",
    vp_filter: "Regex filtro versione",
    vp_filter_desc:
        "Solo le versioni che corrispondono a questa regex vengono aggiunte alla playlist (vuoto = nessun filtro)",
    vp_filter_placeholder: "es. |VCB-Studio|ANi|Semplificato",
    vp_filter_valid: "Regex valida",
    vp_filter_invalid: "Regex non valida",

    // Network
    page_network: "Rete",
    net_proxy: "Proxy HTTP",
    net_proxy_desc: "Formato: host:port (lascia vuoto per disabilitare)",
    net_skip_tls: "Salta verifica TLS",
    net_skip_tls_desc: "Per server Emby autofirmati — non sicuro",
    net_redirect: "Rilevamento reindirizzamenti",
    net_redirect_hosts: "Host da sondare per i reindirizzamenti",
    net_redirect_hosts_desc:
        "Gli URL di streaming di questi host vengono sondati per reindirizzamenti 30x prima di passarli al lettore (vuoto per impostazione predefinita)",

    // System
    page_system: "Sistema",
    sys_appearance: "Aspetto",
    sys_theme: "Tema",
    sys_theme_desc: "Chiaro, scuro o segui il sistema",
    sys_lang: "Lingua",
    sys_lang_desc: "Lingua di visualizzazione dell'interfaccia",
    sys_theme_system: "Sistema",
    sys_theme_light: "Chiaro",
    sys_theme_dark: "Scuro",
    sys_lang_system: "Sistema",
    sys_display: "Schermo",
    sys_font_size: "Dimensione carattere",
    sys_font_size_desc: "Regola la dimensione del testo dell'interfaccia",
    sys_zoom: "Scala interfaccia",
    sys_zoom_desc: "Zoom complessivo HiDPI / alta risoluzione — DPR attuale: {dpr}",
    sys_font: "Carattere interfaccia",
    sys_font_desc: "Scegli il carattere dell'interfaccia",
    sys_font_default: "Predefinito (system-ui)",
    sys_startup: "Avvio",
    sys_autostart: "Avvia all'accesso",
    sys_autostart_desc: "Avvia automaticamente l'app dopo l'accesso",
    sys_silent_start: "Avvio silenzioso",
    sys_silent_start_desc:
        "Avvia nascosto nella barra delle applicazioni senza mostrare la finestra principale (più discreto con l'avvio all'accesso)",
    sys_logs_title: "Log",
    sys_log_level: "Livello di log",
    sys_log_level_desc:
        "Imposta su Debug per un output più dettagliato durante la risoluzione dei problemi",
    sys_log_max_size: "Dimensione massima log (MB)",
    sys_log_max_size_desc:
        "Ruota verso un nuovo file quando quello corrente supera questa dimensione (20–200 MB)",
    sys_log_max_size_capped: "Limitato al massimo di 200 MB",
    sys_log_max_size_floored: "Aumentato al minimo di 20 MB",
    sys_log_max_files: "Numero massimo di file di log",
    sys_log_max_files_desc:
        "Numero di file di log ruotati da conservare (1–14); il più vecchio viene rimosso",
    sys_log_max_files_capped: "Limitato al massimo di 14 file",
    sys_log_mask: "Maschera token sensibili",
    sys_log_mask_desc: "Sostituisci il testo sensibile nei log con segnaposto",
    sys_cache: "Cache",
    sys_cache_size: "Dimensione cache attuale",
    sys_cache_size_desc: "Spazio su disco usato dai log e da altra cache di runtime",
    sys_cache_clear: "Svuota cache",
    sys_cache_clear_desc: "Svuota i file di log per liberare spazio su disco",
    cache_confirm_title: "Svuota cache",
    cache_confirm_message:
        "Il servizio deve essere arrestato prima di svuotare la cache, altrimenti i log in fase di scrittura potrebbero risultare incoerenti. Confermi che il servizio è arrestato e procedi?",
    cache_confirm_ok: "Svuota",
    cache_confirm_cancel: "Annulla",
    cache_stop_first: "Arresta il servizio prima di svuotare la cache",
    cache_cleared: "Cache svuotata, liberati {size}",
    sys_general: "Generale",
    sys_about: "Informazioni",
    sys_about_desc: "Informazioni sulla versione e crediti open source",
    sys_download: "Download",
    sys_speed_limit: "Limite di velocità (MiB/s)",
    sys_speed_limit_desc:
        "Limita la banda usata da download e precaricamento (MiB/s); 0 = illimitato",
    sys_download_note:
        "Il precaricamento e la modalità download sono attivati dai comandi dello userscript del browser, non commutati qui: «cache durante la riproduzione» dello script è il precaricamento e «solo download» è la modalità download; la modalità download richiede inoltre che l'account del tuo media server consenta i download delle risorse",
    sys_trakt: "Scrobbling Trakt.tv",
    sys_trakt_sync_note:
        "Al termine della riproduzione, la tua visione viene sincronizzata automaticamente con Trakt: raggiungendo circa l'80% o più l'episodio viene contrassegnato come visto, al di sotto resta non contrassegnato; vengono contrassegnati anche gli altri episodi della stessa stagione già completati nel tuo media server, senza duplicare quelli già presenti. Sotto l'80% la tua posizione viene memorizzata per riprendere più tardi e l'episodio successivo appare in «Continua a guardare»; riguardare lo stesso episodio lo registra di nuovo — se sia consentito un breve intervallo è controllato dall'interruttore «consenti duplicati» qui sotto.",
    sys_trakt_dashboard: "Apri la dashboard di Trakt",
    sys_trakt_setup_title: "Configurazione",
    sys_trakt_setup_step1: "1. Crea un'app su Trakt: ",
    sys_trakt_setup_link: "trakt.tv/oauth/applications",
    sys_trakt_setup_step2:
        "2. Imposta la «Redirect uri» dell'app sull'indirizzo seguente:",
    sys_trakt_setup_copy: "Copia",
    sys_trakt_setup_copied: "URI di reindirizzamento copiato",
    sys_trakt_setup_copy_failed: "Copia non riuscita — seleziona e copia manualmente",
    sys_trakt_id: "Client ID",
    sys_trakt_id_desc:
        "Ottenuto dopo aver creato un'app su trakt.tv — lascia vuoto per disabilitare",
    sys_trakt_id_placeholder: "Lascia vuoto per disabilitare Trakt",
    sys_trakt_secret: "Client Secret",
    sys_trakt_secret_desc:
        "Ottenuto dopo aver creato un'app su trakt.tv — lascia vuoto per disabilitare",
    sys_trakt_secret_placeholder: "Lascia vuoto per disabilitare Trakt",
    sys_trakt_user: "Nome utente",
    sys_trakt_user_desc: "Il tuo nome utente Trakt (non il soprannome visualizzato)",
    sys_trakt_user_placeholder: "es. your_trakt_user",
    sys_trakt_host: "Abilita host",
    sys_trakt_host_desc:
        "Parole chiave host separate da virgole; lascia vuoto per disabilitare, un singolo punto abilita tutto",
    sys_trakt_host_placeholder: "es. localhost, 192.168., emby.example.com",
    sys_trakt_dup: "Consenti contrassegno duplicato",
    sys_trakt_dup_desc:
        "Se attivo, ogni completamento ricontrassegna lo stesso episodio/film; se disattivo, si applica la deduplicazione con limitazione: lo stesso elemento completato di nuovo entro la finestra di limitazione impostata qui sotto viene contrassegnato una sola volta (gli episodi precedenti recuperati sono sempre deduplicati)",
    sys_trakt_dup_throttle: "Limitazione contrassegno duplicato (secondi)",
    sys_trakt_dup_throttle_desc:
        "Attiva quando «Consenti contrassegno duplicato» è disattivo: lo stesso elemento completato di nuovo entro questi secondi viene registrato una sola volta. Minimo 120 s",
    sys_trakt_dup_throttle_floored:
        "La limitazione non può essere inferiore a 120 secondi; corretta a 120",
    sys_bangumi: "Tracciamento Bangumi.tv",
    sys_bangumi_sync_note:
        "Al termine della riproduzione, la tua visione viene sincronizzata automaticamente con Bangumi: raggiungendo circa l'80% o più l'episodio viene contrassegnato come visto, al di sotto resta non contrassegnato; vengono contrassegnati anche gli altri episodi della stessa stagione già completati nel tuo media server, senza duplicare quelli già presenti. Contrassegnarlo come visto imposta inoltre l'opera su «in visione».",
    sys_bangumi_host: "Abilita host",
    sys_bangumi_host_desc:
        "Parole chiave host separate da virgole; lascia vuoto per disabilitare, un singolo punto abilita tutto",
    sys_bangumi_host_placeholder: "es. localhost, 192.168., emby.example.com",
    sys_bangumi_user: "Nome utente / UID",
    sys_bangumi_user_desc: "Nome utente bgm.tv o le cifre in bgm.tv/user/123456",
    sys_bangumi_user_placeholder: "es. 123456",
    sys_bangumi_token: "Token di accesso",
    sys_bangumi_token_desc:
        "Generato su next.bgm.tv/demo/access-token — lascia vuoto per disabilitare",
    sys_bangumi_token_placeholder: "Lascia vuoto per disabilitare Bangumi",
    sys_bangumi_private: "Collezione privata",
    sys_bangumi_private_desc:
        "Nascondi le voci appena sincronizzate dal tuo profilo pubblico",
    sys_bangumi_genres: "Filtro generi",
    sys_bangumi_genres_desc:
        "Regex confrontata con i generi della serie; vengono sincronizzate solo le serie corrispondenti",
    sys_bangumi_genres_placeholder: "动画|anime",
    sys_bangumi_map: "Mappatura ID",
    sys_bangumi_map_desc:
        "Fissa una serie o un film tmdb/imdb/tvdb a un soggetto Bangumi esatto; ha la massima priorità. Tre formati di stagione: stagione intera (S4), intervallo episodi chiuso (S5E1~S5E50, solo episodi 1–50), intervallo aperto (S5E51++, dall'episodio 51 in poi). E±N a destra sposta l'indice episodio locale al numero di ordinamento Bangumi. Esempi: tmdb:10000|type:tv|S4 -> bgm:20000|E+59; tmdb:10000|type:tv|S5E1~S5E50 -> bgm:20001; tmdb:10000|type:tv|S5E51++ -> bgm:20002; tmdb:10001|type:movie -> bgm:30000. Senza type viene dedotto dalla stagione (una stagione indica TV, altrimenti film)",
    map_placeholder: "tmdb:10000|type:tv|S4 -> bgm:20000|E+59",
    map_check: "Verifica e aggiungi",
    map_remove: "Rimuovi",
    map_err_empty: "Inserisci una mappatura",
    map_err_format: "Formato errato — atteso «LHS -> RHS»",
    map_err_provider: "Origine sconosciuta; sono supportati solo tmdb / imdb / tvdb",
    map_err_provider_id: "ID errato (tmdb/tvdb numerico, imdb inizia con tt)",
    map_err_type: "type deve essere tv o movie",
    map_err_season: "Stagione errata; atteso un intero positivo come S4",
    map_err_ep_range:
        "Intervallo episodi non valido; usa S5E106~S5E157 (chiuso) o S5E158++ (aperto); l'inizio non può essere maggiore della fine",
    map_err_subject: "ID soggetto Bangumi errato; atteso un intero positivo",
    map_err_offset: "Scostamento episodi errato; atteso un intero come E+59 o E-3",
    map_err_movie_season: "Un film non può avere uno scostamento di stagione o episodi",
    map_err_duplicate: "Esiste già una mappatura identica",
    sync_refresh: "Aggiorna autorizzazione",
    sync_refreshing: "Aggiornamento…",
    sync_authorize_opened: "Pagina di autorizzazione aperta — completala nel browser",
    sync_auth_valid: "L'autorizzazione è valida",
    sync_start_service_first: "Avvia prima il servizio",
    sync_refresh_confirm_title: "Aggiorna autorizzazione",
    sync_refresh_confirm_message:
        "Aggiornare ora manualmente l'autorizzazione? Se il token corrente non è valido, la pagina di autorizzazione si aprirà nel browser.",
    sync_refresh_confirm_ok: "Aggiorna",
    sync_test: "Verifica autorizzazione",
    sync_test_desc: "Verifica se le credenziali correnti funzionano",
    sync_testing: "Verifica in corso…",
    sync_test_ok: "L'autorizzazione funziona",
    sync_test_fail:
        "Autorizzazione non riuscita — la configurazione potrebbe essere errata o non ancora autorizzata. Fai clic su «Aggiorna autorizzazione» in alto a destra.",
    sync_incomplete:
        "Configurazione incompleta — compila i campi obbligatori prima di verificare",

    // Config tab (config file + backup / restore / reset / update)
    page_config: "Configurazione",
    cfg_file_title: "File di configurazione",
    cfg_backup_title: "Backup e ripristino",
    cfg_backup_now: "Esegui backup ora",
    cfg_backup_now_desc:
        "Comprimi la configurazione corrente in un backup zip con marca temporale",
    cfg_backup_done: "Configurazione salvata",
    cfg_backup_list: "Backup",
    cfg_backup_list_desc: "Vengono conservati fino a 5 backup — ora {count}",
    cfg_backup_empty: "Ancora nessun backup",
    cfg_view: "Visualizza",
    cfg_restore: "Ripristina",
    cfg_delete: "Elimina",
    cfg_import: "Importa backup",
    cfg_import_desc: "Importa e ripristina la configurazione da un file zip esterno",
    cfg_restore_done: "Configurazione ripristinata",
    cfg_restore_confirm_title: "Ripristina configurazione",
    cfg_restore_confirm_message:
        "Sovrascrivere la configurazione corrente con il backup «{name}»? Questa operazione non può essere annullata.",
    cfg_import_confirm_title: "Importa e ripristina configurazione",
    cfg_import_confirm_message:
        "Importare questo backup e sovrascrivere la configurazione corrente? Questa operazione non può essere annullata.",
    cfg_delete_confirm_title: "Elimina backup",
    cfg_delete_confirm_message: "Eliminare il backup «{name}»?",
    cfg_reset_title: "Ripristino",
    cfg_reset: "Ripristina valori predefiniti",
    cfg_reset_desc: "Ripristina tutte le impostazioni ai valori predefiniti",
    cfg_reset_done: "Configurazione ripristinata ai valori predefiniti",
    cfg_reset_confirm_title: "Ripristina configurazione",
    cfg_reset_confirm_message:
        "Ripristinare la configurazione predefinita? La configurazione corrente verrà sovrascritta — questa operazione non può essere annullata.",
    cfg_update_title: "Aggiornamento",
    cfg_update_auto: "Controlla automaticamente gli aggiornamenti",
    cfg_update_auto_desc:
        "All'avvio controlla su GitHub nuove versioni e mostra un suggerimento nella panoramica",
    cfg_update_check: "Controlla ora",
    cfg_update_check_desc:
        "Controlla ora su GitHub se è disponibile una versione più recente",
    cfg_update_checking: "Controllo in corso…",
    cfg_update_available:
        "Trovata nuova versione v{version} — apertura della pagina di rilascio",
    cfg_update_latest: "Stai usando la versione più recente v{version}",

    // Update banner (overview)
    ov_update_available: "Nuova versione v{version} disponibile",
    ov_update_action: "Aggiorna",
    ov_update_dismiss: "Ignora questa versione",
    sys_privacy: "Privacy",
    sys_no_progress: "Disabilita segnalazione progresso",
    sys_no_progress_desc:
        "Non segnalare il progresso di riproduzione al server Emby/Jellyfin",
    sys_accent: "Colore accento",
    sys_accent_desc:
        "Colore di evidenziazione dell'interfaccia — influisce su pulsanti, stati attivi e badge",
    sys_center_nav: "Centra barra laterale",
    sys_center_nav_desc:
        "Centra verticalmente le schede della barra laterale come gruppo",

    // Log levels
    log_error: "Error — solo arresti anomali",
    log_warn: "Warn — condizioni anomale",
    log_info: "Info — predefinito, funzionamento quotidiano",
    log_debug: "Debug — risoluzione problemi",
    log_trace: "Trace — dettaglio completo",

    // Logs page
    page_logs: "Log",
    logs_app: "Log app",
    logs_mpv: "Log mpv",
    logs_filter: "Filtra…",
    logs_clear: "Svuota",
    logs_bottom: "↓ In fondo",
    logs_empty: "In attesa dell'output dei log…",
    logs_no_mpv:
        "Nessun log mpv trovato — fai clic su «Scegli log mpv» per caricarne uno",
    logs_lines: "righe",
    logs_loading_older: "Caricamento log meno recenti…",
    logs_scroll_older: "Scorri verso l'alto per caricare i log meno recenti",
    logs_open_folder: "Apri cartella log",
    logs_pick_mpv: "Scegli log mpv",
    logs_reset_mpv: "Ripristina predefinito",
    logs_reset_mpv_title: "Torna al log mpv predefinito nella cartella dei log",
    logs_anon: "Anonimo",
    logs_anon_title:
        "Nascondi ID dispositivo, token, IP, ID utente, host dell'URL e nomi utente Bangumi / Trakt solo nella vista, utile per condividere screenshot; il file di log non viene toccato — l'oscuramento del file segue ancora l'interruttore «Testo sensibile»",

    // About modal
    about_thanks: "Crediti",
    about_thanks_desc: "per l'ispirazione infinita",
    about_version_label: "Versione",

    // Autostart toasts
    autostart_on: "Avvio all'accesso abilitato",
    autostart_off: "Avvio all'accesso disabilitato",

    // Font size options
    font_12: "12px (compatto)",
    font_13: "13px (predefinito)",
    font_14: "14px (comodo)",
    font_15: "15px (grande)",
    font_16: "16px (molto grande)",
};
