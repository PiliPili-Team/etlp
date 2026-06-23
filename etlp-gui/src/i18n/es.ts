import { zhCN } from "./zh-CN";

export const es: typeof zhCN = {
    ...zhCN,

    // App
    app_name: "Genshin",

    // Nav
    nav_overview: "Resumen",
    nav_player: "Reproductor",
    nav_version_prefer: "Versión",
    nav_network: "Red",
    nav_config: "Configuración",
    nav_system: "Sistema",
    nav_logs: "Registros",
    nav_sec_play: "Reproducción",
    nav_sec_settings: "Ajustes",
    nav_sec_sync: "Sincronización",
    nav_bangumi: "Bangumi",
    nav_trakt: "Trakt",
    nav_sec_debug: "Depuración",

    // Common
    add: "Añadir",
    add_placeholder: "Escribe y pulsa Intro para añadir",
    open_dir: "Abrir carpeta",
    loading: "Cargando configuración…",

    // Overview
    page_overview: "Resumen",
    ov_service: "Servicio local",
    ov_running: "En ejecución",
    ov_stopped: "Detenido",
    ov_port: "Puerto",
    ov_port_desc: "Dirección de escucha local",
    ov_uptime: "Tiempo activo",
    ov_uptime_desc: "Desde que se inició el servicio",
    ov_address: "Dirección",
    ov_address_desc: "Solo localhost",
    ov_config: "Ajustes",
    ov_config_file: "Archivo de configuración",
    ov_config_file_desc: "Ver o abrir en un editor externo",
    ov_edit_config: "Editar configuración",
    ov_restart: "Reiniciar servicio",
    ov_restart_desc:
        "Detener el servicio, liberar recursos y reiniciar con la configuración más reciente",
    ov_about: "Acerca de",
    ov_about_desc: "Información de la versión y créditos de código abierto",
    ov_view: "Ver",
    ov_start: "Iniciar",
    ov_stop: "Detener",

    // Toasts
    toast_started: "Servicio iniciado en el puerto {port}",
    toast_stopped: "Servicio detenido",
    toast_restarted: "Servicio reiniciado en el puerto {port}",
    toast_start_failed: "No se pudo iniciar el servicio",
    toast_stop_failed: "No se pudo detener el servicio",
    toast_restart_failed: "No se pudo reiniciar el servicio",
    toast_open_failed: "No se pudo abrir",
    sync_not_configured: "Aún no configurado: completa primero los campos",

    // Player
    page_player: "Reproductor",
    pl_type: "Tipo de reproductor",
    pl_type_desc: "Elige un reproductor multimedia local",
    pl_startup: "Opciones de inicio",
    pl_fullscreen: "Pantalla completa",
    pl_fullscreen_desc: "Iniciar el reproductor en modo de pantalla completa",
    pl_mute: "Iniciar silenciado",
    pl_mute_desc: "Iniciar con el audio desactivado (mpv --no-audio)",
    pl_pretty_title: "Título elegante",
    pl_pretty_title_desc:
        "Anteponer el nombre del servidor al título de la ventana del reproductor",
    pl_kill_start: "Cerrar al iniciar",
    pl_kill_start_desc: "Cerrar los procesos del reproductor existentes al iniciar",
    pl_path: "Ruta del reproductor",
    pl_path_desc: "Opcional: déjalo vacío para usar el reproductor del PATH del sistema",
    pl_browse: "Examinar…",
    pl_path_error: "Ruta no encontrada: comprueba la entrada",
    pl_progress_support:
        "Informe de progreso: mpv / IINA son totalmente compatibles: actualizaciones en directo durante la reproducción, posición de reanudación reescrita al salir, marcado como visto, sincronización con Trakt / Bangumi y seguimiento por episodio. Otros reproductores solo escriben la posición final y sincronizan al salir, sin informe en directo durante la reproducción; VLC reproduce toda la temporada de forma continua, MPC y dandanplay son de un solo episodio, y la relectura de posición de PotPlayer es solo para Windows",

    // Version prefer
    page_vp: "Preferencia de versión",
    vp_priority: "Prioridad de versión",
    vp_keywords: "Palabras clave de versión",
    vp_keywords_desc:
        "Coincidir las palabras clave de versión del medio en orden: las entradas anteriores ganan",
    vp_keywords_placeholder: "p. ej. VCB-Studio, ANi, DBD-Raws",
    vp_playlist: "Aplicar a la lista de reproducción",
    vp_playlist_desc: "Usar la prioridad de versión al crear la lista de reproducción",
    vp_subtitle: "Preferencia de subtítulos",
    vp_sub_priority: "Prioridad de subtítulos",
    vp_sub_priority_desc:
        "Coincidir las palabras clave de las pistas de subtítulos en orden",
    vp_sub_priority_placeholder: "p. ej. Simplificado, CHS",
    vp_sub_extract: "Extracción de subtítulos entre versiones",
    vp_sub_extract_desc:
        "Extraer subtítulos de otras versiones cuando no se encuentran en la actual",
    vp_sub_extract_placeholder: "p. ej. CHS, Simplificado",
    vp_limits: "Límites de la lista de reproducción",
    vp_max_eps: "Episodios máximos por sesión",
    vp_max_eps_desc:
        "Los episodios se truncan al alcanzar este límite; 0 o vacío significa ilimitado (recomendado: 10–100)",
    vp_last_ep: "Desactivar en el último episodio",
    vp_last_ep_desc:
        "Activado: al reproducir el último episodio de la temporada, no crea lista de reproducción y abre solo ese episodio (no le sigue nada); Desactivado: crea siempre la lista de reproducción (episodio actual + posteriores)",
    vp_filter: "Regex de filtro de versión",
    vp_filter_desc:
        "Solo se añaden a la lista de reproducción las versiones que coinciden con esta regex (vacío = sin filtro)",
    vp_filter_placeholder: "p. ej. |VCB-Studio|ANi|Simplificado",
    vp_filter_valid: "Regex válida",
    vp_filter_invalid: "Regex no válida",

    // Network
    page_network: "Red",
    net_proxy: "Proxy HTTP",
    net_proxy_desc: "Formato: host:port (déjalo vacío para desactivar)",
    net_skip_tls: "Omitir verificación TLS",
    net_skip_tls_desc: "Para servidores Emby autofirmados: no es seguro",
    net_redirect: "Detección de redirecciones",
    net_redirect_hosts: "Hosts que comprobar para redirecciones",
    net_redirect_hosts_desc:
        "Las URL de transmisión de estos hosts se comprueban en busca de redirecciones 30x antes de pasarlas al reproductor (vacío de forma predeterminada)",

    // System
    page_system: "Sistema",
    sys_appearance: "Apariencia",
    sys_theme: "Tema",
    sys_theme_desc: "Claro, oscuro o seguir al sistema",
    sys_lang: "Idioma",
    sys_lang_desc: "Idioma de visualización de la interfaz",
    sys_theme_system: "Sistema",
    sys_theme_light: "Claro",
    sys_theme_dark: "Oscuro",
    sys_lang_system: "Sistema",
    sys_display: "Pantalla",
    sys_font_size: "Tamaño de fuente",
    sys_font_size_desc: "Ajustar el tamaño del texto de la interfaz",
    sys_zoom: "Escala de la interfaz",
    sys_zoom_desc: "Zoom global HiDPI / alta resolución: DPR actual: {dpr}",
    sys_font: "Fuente de la interfaz",
    sys_font_desc: "Elegir la fuente de la interfaz",
    sys_font_default: "Predeterminada (system-ui)",
    sys_startup: "Inicio",
    sys_autostart: "Iniciar al iniciar sesión",
    sys_autostart_desc: "Iniciar la aplicación automáticamente tras iniciar sesión",
    sys_silent_start: "Inicio silencioso",
    sys_silent_start_desc:
        "Iniciar oculto en la bandeja sin mostrar la ventana principal (más discreto con el inicio al iniciar sesión)",
    sys_logs_title: "Registros",
    sys_log_level: "Nivel de registro",
    sys_log_level_desc:
        "Ponlo en Debug para una salida más detallada al solucionar problemas",
    sys_log_max_size: "Tamaño máx. de registro (MB)",
    sys_log_max_size_desc:
        "Rotar a un archivo nuevo cuando el actual supere este tamaño (20–200 MB)",
    sys_log_max_size_capped: "Limitado al máximo de 200 MB",
    sys_log_max_size_floored: "Elevado al mínimo de 20 MB",
    sys_log_max_files: "Máx. de archivos de registro",
    sys_log_max_files_desc:
        "Número de archivos de registro rotados que conservar (1–14); se elimina el más antiguo",
    sys_log_max_files_capped: "Limitado al máximo de 14 archivos",
    sys_log_mask: "Enmascarar tokens sensibles",
    sys_log_mask_desc:
        "Reemplazar el texto sensible en los registros con marcadores de posición",
    sys_cache: "Caché",
    sys_cache_size: "Tamaño actual de la caché",
    sys_cache_size_desc:
        "Espacio en disco usado por los registros y otra caché de tiempo de ejecución",
    sys_cache_clear: "Borrar caché",
    sys_cache_clear_desc: "Vaciar los archivos de registro para liberar espacio en disco",
    cache_confirm_title: "Borrar caché",
    cache_confirm_message:
        "El servicio debe detenerse antes de borrar la caché; de lo contrario, los registros que se estén escribiendo podrían quedar incoherentes. ¿Confirmas que el servicio está detenido y continúas?",
    cache_confirm_ok: "Borrar",
    cache_confirm_cancel: "Cancelar",
    cache_stop_first: "Detén el servicio antes de borrar la caché",
    cache_cleared: "Caché borrada, se liberaron {size}",
    sys_general: "General",
    sys_about: "Acerca de",
    sys_about_desc: "Información de la versión y créditos de código abierto",
    sys_download: "Descargas",
    sys_speed_limit: "Límite de velocidad (MiB/s)",
    sys_speed_limit_desc:
        "Limita el ancho de banda usado por las descargas y la caché de precarga (MiB/s); 0 = ilimitado",
    sys_download_note:
        "La precarga y el modo de descarga se activan mediante los comandos del userscript del navegador, no se alternan aquí: «almacenar en caché durante la reproducción» del script es la precarga y «solo descargar» es el modo de descarga; el modo de descarga también requiere que tu cuenta del servidor multimedia permita descargas de recursos",
    sys_trakt: "Scrobbling de Trakt.tv",
    sys_trakt_sync_note:
        "Cuando termina la reproducción, tu visionado se sincroniza automáticamente con Trakt: alcanzar alrededor del 80 % o más marca el episodio como visto, por debajo permanece sin marcar; también se marcan otros episodios de la misma temporada que ya completaste en tu servidor multimedia, sin duplicar los ya presentes. Por debajo del 80 % se recuerda tu posición para retomar más tarde, y el siguiente episodio aparece en «Continuar viendo»; volver a ver el mismo episodio lo registra de nuevo: el interruptor «permitir duplicados» de abajo controla si se admite un intervalo breve.",
    sys_trakt_dashboard: "Abrir el panel de Trakt",
    sys_trakt_setup_title: "Configuración",
    sys_trakt_setup_step1: "1. Crea una app en Trakt: ",
    sys_trakt_setup_link: "trakt.tv/oauth/applications",
    sys_trakt_setup_step2:
        "2. Establece la «Redirect uri» de la app en la dirección siguiente:",
    sys_trakt_setup_copy: "Copiar",
    sys_trakt_setup_copied: "URI de redirección copiada",
    sys_trakt_setup_copy_failed: "Error al copiar: selecciona y copia manualmente",
    sys_trakt_id: "ID de cliente",
    sys_trakt_id_desc:
        "Se obtiene tras crear una app en trakt.tv: déjalo vacío para desactivar",
    sys_trakt_id_placeholder: "Déjalo vacío para desactivar Trakt",
    sys_trakt_secret: "Secreto de cliente",
    sys_trakt_secret_desc:
        "Se obtiene tras crear una app en trakt.tv: déjalo vacío para desactivar",
    sys_trakt_secret_placeholder: "Déjalo vacío para desactivar Trakt",
    sys_trakt_user: "Nombre de usuario",
    sys_trakt_user_desc: "Tu nombre de usuario de Trakt (no el apodo mostrado)",
    sys_trakt_user_placeholder: "p. ej. your_trakt_user",
    sys_trakt_host: "Activar host",
    sys_trakt_host_desc:
        "Palabras clave de host separadas por comas; déjalo vacío para desactivar, un solo punto activa todos",
    sys_trakt_host_placeholder: "p. ej. localhost, 192.168., emby.example.com",
    sys_trakt_dup: "Permitir marcado duplicado",
    sys_trakt_dup_desc:
        "Si está activado, cada finalización vuelve a marcar el mismo episodio/película; si está desactivado, se aplica deduplicación con límite: el mismo elemento terminado de nuevo dentro de la ventana de límite definida abajo se marca solo una vez (los episodios anteriores rellenados siempre se deduplican)",
    sys_trakt_dup_throttle: "Límite de marcado duplicado (segundos)",
    sys_trakt_dup_throttle_desc:
        "Efectivo cuando «Permitir marcado duplicado» está desactivado: el mismo elemento terminado de nuevo dentro de estos segundos se registra solo una vez. Mínimo 120 s",
    sys_trakt_dup_throttle_floored:
        "El límite no puede ser inferior a 120 segundos; corregido a 120",
    sys_bangumi: "Seguimiento de Bangumi.tv",
    sys_bangumi_sync_note:
        "Cuando termina la reproducción, tu visionado se sincroniza automáticamente con Bangumi: alcanzar alrededor del 80 % o más marca el episodio como visto, por debajo permanece sin marcar; también se marcan otros episodios de la misma temporada que ya completaste en tu servidor multimedia, sin duplicar los ya presentes. Marcarlo como visto también establece la obra como «viendo».",
    sys_bangumi_host: "Activar host",
    sys_bangumi_host_desc:
        "Palabras clave de host separadas por comas; déjalo vacío para desactivar, un solo punto activa todos",
    sys_bangumi_host_placeholder: "p. ej. localhost, 192.168., emby.example.com",
    sys_bangumi_user: "Nombre de usuario / UID",
    sys_bangumi_user_desc:
        "Nombre de usuario de bgm.tv o los dígitos en bgm.tv/user/123456",
    sys_bangumi_user_placeholder: "p. ej. 123456",
    sys_bangumi_token: "Token de acceso",
    sys_bangumi_token_desc:
        "Generado en next.bgm.tv/demo/access-token: déjalo vacío para desactivar",
    sys_bangumi_token_placeholder: "Déjalo vacío para desactivar Bangumi",
    sys_bangumi_private: "Colección privada",
    sys_bangumi_private_desc:
        "Ocultar las entradas recién sincronizadas de tu perfil público",
    sys_bangumi_genres: "Filtro de géneros",
    sys_bangumi_genres_desc:
        "Regex comparada con los géneros de la serie; solo se sincronizan las series coincidentes",
    sys_bangumi_genres_placeholder: "动画|anime",
    sys_bangumi_map: "Asignación de ID",
    sys_bangumi_map_desc:
        "Fijar una serie o película de tmdb/imdb/tvdb a un sujeto exacto de Bangumi; tiene la máxima prioridad. Tres formatos de temporada: temporada completa (S4), rango de episodios cerrado (S5E1~S5E50, solo episodios 1–50), rango abierto (S5E51++, desde el episodio 51 en adelante). E±N a la derecha desplaza el índice de episodio local al número de orden de Bangumi. Ejemplos: tmdb:10000|type:tv|S4 -> bgm:20000|E+59; tmdb:10000|type:tv|S5E1~S5E50 -> bgm:20001; tmdb:10000|type:tv|S5E51++ -> bgm:20002; tmdb:10001|type:movie -> bgm:30000. Sin type se deduce de la temporada (una temporada significa TV, de lo contrario película)",
    map_placeholder: "tmdb:10000|type:tv|S4 -> bgm:20000|E+59",
    map_check: "Comprobar y añadir",
    map_remove: "Quitar",
    map_err_empty: "Introduce una asignación",
    map_err_format: "Formato incorrecto: se esperaba «LHS -> RHS»",
    map_err_provider: "Origen desconocido; solo se admiten tmdb / imdb / tvdb",
    map_err_provider_id: "ID incorrecto (tmdb/tvdb numérico, imdb empieza por tt)",
    map_err_type: "type debe ser tv o movie",
    map_err_season: "Temporada incorrecta; se esperaba un entero positivo como S4",
    map_err_ep_range:
        "Rango de episodios incorrecto; usa S5E106~S5E157 (cerrado) o S5E158++ (abierto); el inicio no puede ser mayor que el final",
    map_err_subject: "ID de sujeto de Bangumi incorrecto; se esperaba un entero positivo",
    map_err_offset:
        "Desplazamiento de episodio incorrecto; se esperaba un entero como E+59 o E-3",
    map_err_movie_season:
        "Una película no puede llevar desplazamiento de temporada o episodio",
    map_err_duplicate: "Ya existe una asignación idéntica",
    sync_refresh: "Actualizar autorización",
    sync_refreshing: "Actualizando…",
    sync_authorize_opened: "Página de autorización abierta: complétala en tu navegador",
    sync_auth_valid: "La autorización es válida",
    sync_start_service_first: "Inicia primero el servicio",
    sync_refresh_confirm_title: "Actualizar autorización",
    sync_refresh_confirm_message:
        "¿Actualizar la autorización manualmente ahora? Si el token actual no es válido, la página de autorización se abrirá en tu navegador.",
    sync_refresh_confirm_ok: "Actualizar",
    sync_test: "Comprobar autorización",
    sync_test_desc: "Comprobar si las credenciales actuales funcionan",
    sync_testing: "Comprobando…",
    sync_test_ok: "La autorización funciona",
    sync_test_fail:
        "Error de autorización: la configuración puede ser incorrecta o aún no estar autorizada. Haz clic en «Actualizar autorización» en la parte superior derecha.",
    sync_incomplete:
        "Configuración incompleta: completa los campos obligatorios antes de comprobar",

    // Config tab (config file + backup / restore / reset / update)
    page_config: "Configuración",
    cfg_file_title: "Archivo de configuración",
    cfg_backup_title: "Copia de seguridad y restauración",
    cfg_backup_now: "Hacer copia ahora",
    cfg_backup_now_desc:
        "Empaquetar la configuración actual en una copia zip con marca de tiempo",
    cfg_backup_done: "Configuración respaldada",
    cfg_backup_list: "Copias de seguridad",
    cfg_backup_list_desc: "Se conservan hasta 5 copias: {count} ahora",
    cfg_backup_empty: "Aún no hay copias de seguridad",
    cfg_view: "Ver",
    cfg_restore: "Restaurar",
    cfg_delete: "Eliminar",
    cfg_import: "Importar copia",
    cfg_import_desc: "Importar y restaurar la configuración desde un archivo zip externo",
    cfg_restore_done: "Configuración restaurada",
    cfg_restore_confirm_title: "Restaurar configuración",
    cfg_restore_confirm_message:
        "¿Sobrescribir la configuración actual con la copia «{name}»? Esto no se puede deshacer.",
    cfg_import_confirm_title: "Importar y restaurar configuración",
    cfg_import_confirm_message:
        "¿Importar esta copia y sobrescribir la configuración actual? Esto no se puede deshacer.",
    cfg_delete_confirm_title: "Eliminar copia de seguridad",
    cfg_delete_confirm_message: "¿Eliminar la copia «{name}»?",
    cfg_reset_title: "Restablecer",
    cfg_reset: "Restablecer a los valores predeterminados",
    cfg_reset_desc: "Restaurar todos los ajustes a sus valores predeterminados",
    cfg_reset_done: "Configuración restablecida a los valores predeterminados",
    cfg_reset_confirm_title: "Restablecer configuración",
    cfg_reset_confirm_message:
        "¿Restablecer a la configuración predeterminada? La configuración actual se sobrescribirá: esto no se puede deshacer.",
    cfg_update_title: "Actualización",
    cfg_update_auto: "Buscar actualizaciones automáticamente",
    cfg_update_auto_desc:
        "Buscar nuevas versiones en GitHub al iniciar y mostrar una sugerencia en el resumen",
    cfg_update_check: "Buscar ahora",
    cfg_update_check_desc: "Buscar ahora en GitHub una versión más reciente",
    cfg_update_checking: "Buscando…",
    cfg_update_available:
        "Nueva versión v{version} encontrada: abriendo la página de la versión",
    cfg_update_latest: "Tienes la versión más reciente v{version}",

    // Update banner (overview)
    ov_update_available: "Nueva versión v{version} disponible",
    ov_update_action: "Actualizar",
    ov_update_dismiss: "Descartar esta versión",
    sys_privacy: "Privacidad",
    sys_no_progress: "Desactivar informe de progreso",
    sys_no_progress_desc:
        "No informar del progreso de reproducción al servidor Emby/Jellyfin",
    sys_accent: "Color de acento",
    sys_accent_desc:
        "Color de resalte de la interfaz: afecta a botones, estados activos e insignias",
    sys_center_nav: "Centrar barra lateral",
    sys_center_nav_desc:
        "Centrar verticalmente las pestañas de la barra lateral como grupo",

    // Log levels
    log_error: "Error: solo fallos",
    log_warn: "Warn: condiciones anómalas",
    log_info: "Info: predeterminado, funcionamiento diario",
    log_debug: "Debug: solución de problemas",
    log_trace: "Trace: detalle completo",

    // Logs page
    page_logs: "Registros",
    logs_app: "Registro de la app",
    logs_mpv: "Registro de mpv",
    logs_filter: "Filtrar…",
    logs_clear: "Borrar",
    logs_bottom: "↓ Abajo",
    logs_empty: "Esperando la salida del registro…",
    logs_no_mpv:
        "No se encontró ningún registro de mpv: haz clic en «Elegir registro de mpv» para cargar uno",
    logs_lines: "líneas",
    logs_loading_older: "Cargando registros más antiguos…",
    logs_scroll_older: "Desplázate hacia arriba para cargar registros más antiguos",
    logs_open_folder: "Abrir carpeta de registros",
    logs_pick_mpv: "Elegir registro de mpv",
    logs_reset_mpv: "Restablecer al predeterminado",
    logs_reset_mpv_title:
        "Volver al registro de mpv predeterminado en la carpeta de registros",
    logs_anon: "Anónimo",
    logs_anon_title:
        "Ocultar el ID del dispositivo, los tokens, las IP, el ID de usuario, el host de la URL y los nombres de usuario de Bangumi / Trakt solo en la vista, útil para compartir capturas; el archivo de registro no se modifica: la censura del archivo sigue rigiéndose por el interruptor «Texto sensible»",

    // About modal
    about_thanks: "Créditos",
    about_thanks_desc: "por la inspiración inagotable",
    about_version_label: "Versión",

    // Autostart toasts
    autostart_on: "Inicio al iniciar sesión activado",
    autostart_off: "Inicio al iniciar sesión desactivado",

    // Font size options
    font_12: "12px (compacto)",
    font_13: "13px (predeterminado)",
    font_14: "14px (cómodo)",
    font_15: "15px (grande)",
    font_16: "16px (muy grande)",
};
