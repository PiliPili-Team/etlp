import { zhCN } from "./zh-CN";

export const fr: typeof zhCN = {
    ...zhCN,

    // App
    app_name: "Genshin",

    // Nav
    nav_overview: "Aperçu",
    nav_player: "Lecteur",
    nav_version_prefer: "Version",
    nav_network: "Réseau",
    nav_config: "Configuration",
    nav_system: "Système",
    nav_logs: "Journaux",
    nav_sec_play: "Lecture",
    nav_sec_settings: "Paramètres",
    nav_sec_sync: "Synchronisation",
    nav_bangumi: "Bangumi",
    nav_trakt: "Trakt",
    nav_sec_debug: "Débogage",
    nav_download: "Téléchargements",

    // Common
    add: "Ajouter",
    add_placeholder: "Saisissez puis appuyez sur Entrée pour ajouter",
    open_dir: "Ouvrir le dossier",
    loading: "Chargement de la configuration…",

    // Overview
    page_overview: "Aperçu",
    ov_service: "Service local",
    ov_running: "En cours",
    ov_stopped: "Arrêté",
    ov_port: "Port",
    ov_port_desc: "Adresse d'écoute locale",
    ov_uptime: "Temps de fonctionnement",
    ov_uptime_desc: "Depuis le démarrage du service",
    ov_address: "Adresse",
    ov_address_desc: "Localhost uniquement",
    ov_config: "Paramètres",
    ov_config_file: "Fichier de configuration",
    ov_config_file_desc: "Afficher ou ouvrir dans un éditeur externe",
    ov_edit_config: "Modifier la configuration",
    ov_restart: "Redémarrer le service",
    ov_restart_desc:
        "Arrêter le service, libérer les ressources, puis redémarrer avec la dernière configuration",
    ov_about: "À propos",
    ov_about_desc: "Informations de version et crédits open source",
    ov_view: "Afficher",
    ov_start: "Démarrer",
    ov_stop: "Arrêter",

    // Toasts
    toast_started: "Service démarré sur le port {port}",
    toast_stopped: "Service arrêté",
    toast_restarted: "Service redémarré sur le port {port}",
    toast_start_failed: "Échec du démarrage du service",
    toast_stop_failed: "Échec de l'arrêt du service",
    toast_restart_failed: "Échec du redémarrage du service",
    toast_open_failed: "Échec de l'ouverture",
    sync_not_configured: "Pas encore configuré — renseignez d'abord les champs",

    // Player
    page_player: "Lecteur",
    pl_type: "Type de lecteur",
    pl_type_desc: "Choisir un lecteur multimédia local",
    pl_startup: "Options de lancement",
    pl_fullscreen: "Plein écran",
    pl_fullscreen_desc: "Démarrer le lecteur en mode plein écran",
    pl_mute: "Démarrer en silence",
    pl_mute_desc: "Lancer en sourdine (mpv --mute=yes)",
    pl_pretty_title: "Titre élégant",
    pl_pretty_title_desc: "Ajouter le nom du serveur au titre de la fenêtre du lecteur",
    pl_kill_start: "Quitter au démarrage",
    pl_kill_start_desc: "Terminer les processus du lecteur existants au démarrage",
    pl_path: "Chemin du lecteur",
    pl_path_desc: "Facultatif — laissez vide pour utiliser le lecteur du PATH système",
    pl_browse: "Parcourir…",
    pl_path_error: "Chemin introuvable — veuillez vérifier la saisie",
    pl_progress_support:
        "Rapport de progression : mpv / IINA sont entièrement pris en charge — mises à jour en direct pendant la lecture, position de reprise réécrite à la sortie, marquage comme vu, synchronisation Trakt / Bangumi et suivi par épisode. Les autres lecteurs n'écrivent que la position finale et synchronisent à la sortie, sans rapport en direct pendant la lecture ; VLC lit toute la saison en continu, MPC et dandanplay sont à épisode unique, et la relecture de position de PotPlayer est réservée à Windows",

    // Version prefer
    page_vp: "Préférence de version",
    vp_priority: "Ordre de priorité des versions",
    vp_keywords: "Étiquettes de version",
    vp_keywords_desc:
        "Lorsque plusieurs fichiers existent pour le même épisode, celui dont le chemin correspond à l'étiquette la plus haute dans la liste est sélectionné. Exemple : « TeamX → GroupA → StreamB » — si les trois versions sont disponibles, TeamX est choisi ; sinon GroupA ; etc.",
    vp_keywords_placeholder: "ex. TeamX, GroupA, StreamB",
    vp_playlist: "Appliquer à la liste de lecture",
    vp_playlist_desc:
        "Utiliser la priorité de version lors de la création de la liste de lecture",
    vp_subtitle: "Préférence de sous-titres",
    vp_sub_priority: "Priorité des sous-titres",
    vp_sub_priority_desc:
        "Faire correspondre les mots-clés des pistes de sous-titres dans l'ordre",
    vp_sub_priority_placeholder: "ex. Simplifié, CHS",
    vp_sub_extract: "Extraction de sous-titres entre versions",
    vp_sub_extract_desc:
        "Extraire les sous-titres d'autres versions lorsqu'aucun n'est trouvé dans la version actuelle",
    vp_sub_extract_placeholder: "ex. CHS, Simplifié",
    vp_limits: "Limites de la liste de lecture",
    vp_max_eps: "Épisodes max par session",
    vp_max_eps_desc:
        "Les épisodes sont tronqués une fois cette limite atteinte ; 0 ou vide signifie illimité (recommandé : 10–100)",
    vp_last_ep: "Désactiver au dernier épisode",
    vp_last_ep_desc:
        "Activé : lors de la lecture du dernier épisode de la saison, ne crée aucune liste de lecture et ouvre uniquement cet épisode (rien ne suit) ; Désactivé : crée toujours la liste de lecture (épisode actuel + suivants)",
    vp_filter: "Empreinte de version",
    vp_filter_desc:
        "Extrait les caractéristiques de version du chemin du fichier en cours de lecture comme « empreinte ». Seuls les épisodes dont le chemin correspond exactement au même ensemble de caractéristiques sont ajoutés à la liste de lecture, fixant ainsi toute la saison à la même version. Exemple : si le regex correspond à « TeamX » et « 1080p » dans le fichier actuel, seuls les épisodes contenant ces deux mots sont inclus (vide = désactivé)",
    vp_filter_placeholder: "ex. |TeamX|1080p|CHS",
    vp_filter_valid: "Regex valide",
    vp_filter_invalid: "Regex invalide",

    // Network
    page_network: "Réseau",
    net_proxy_http: "Proxy HTTP",
    net_proxy_https: "Proxy HTTPS",
    net_proxy_socks5: "Proxy SOCKS5",
    net_proxy_desc:
        "Saisir uniquement host:port ; coller une URL complète pour détection auto du schéma ; vide = désactivé",
    net_proxy_https_desc:
        "Utilisé pour les connexions chiffrées (HTTPS) ; si vide, se replie sur le proxy HTTP ; même format que HTTP",
    net_proxy_socks5_desc:
        "Achemine tout le trafic de protocoles ; idéal pour les réseaux sans tunnel HTTP ; laisser vide pour désactiver",
    net_proxy_enabled: "Activer le proxy",
    net_proxy_enabled_desc:
        "Désactivé, l'URL est conservée mais toutes les connexions sont directes ; les IP privées (192.168.x, 10.x…) sont toujours contournées automatiquement",
    net_skip_tls: "Ignorer la vérification TLS",
    net_skip_tls_desc: "Pour les serveurs multimédias auto-signés — non sécurisé",
    net_redirect: "Détection des redirections",
    net_redirect_hosts: "Hôtes à sonder pour les redirections",
    net_redirect_hosts_desc:
        "Les URL de flux de ces hôtes sont sondées pour les redirections 30x avant d'être transmises au lecteur (vide par défaut)",

    // System
    page_system: "Système",
    sys_appearance: "Apparence",
    sys_theme: "Thème",
    sys_theme_desc: "Clair, sombre ou suivre le système",
    sys_lang: "Langue",
    sys_lang_desc: "Langue d'affichage de l'interface",
    sys_theme_system: "Système",
    sys_theme_light: "Clair",
    sys_theme_dark: "Sombre",
    sys_lang_system: "Système",
    sys_display: "Affichage",
    sys_font_size: "Taille de police",
    sys_font_size_desc: "Ajuster la taille du texte de l'interface",
    sys_zoom: "Échelle de l'interface",
    sys_zoom_desc: "Zoom global HiDPI / haute résolution — DPR actuel : {dpr}",
    sys_font: "Police de l'interface",
    sys_font_desc: "Choisir la police de l'interface",
    sys_font_default: "Par défaut (system-ui)",
    sys_startup: "Démarrage",
    sys_autostart: "Lancer à la connexion",
    sys_autostart_desc: "Démarrer automatiquement l'application après la connexion",
    sys_silent_start: "Démarrage silencieux",
    sys_silent_start_desc:
        "Démarrer masqué dans la barre d'état sans afficher la fenêtre principale (plus discret avec le lancement à la connexion)",
    sys_logs_title: "Journaux",
    sys_log_level: "Niveau de journalisation",
    sys_log_level_desc:
        "Réglez sur Debug pour une sortie plus détaillée lors du dépannage",
    sys_log_max_size: "Taille max du journal (Mo)",
    sys_log_max_size_desc:
        "Basculer vers un nouveau fichier dès que le fichier actuel dépasse cette taille (20–200 Mo)",
    sys_log_max_size_capped: "Limité au maximum de 200 Mo",
    sys_log_max_size_floored: "Augmenté au minimum de 20 Mo",
    sys_log_max_files: "Nombre max de fichiers journaux",
    sys_log_max_files_desc:
        "Nombre de fichiers journaux à conserver (1–14) ; le plus ancien est supprimé",
    sys_log_max_files_capped: "Limité au maximum de 14 fichiers",
    sys_log_mask: "Masquer les jetons sensibles",
    sys_log_mask_desc:
        "Remplacer le texte sensible dans les journaux par des espaces réservés",
    sys_cache: "Cache",
    sys_cache_size: "Taille actuelle du cache",
    sys_cache_size_desc:
        "Espace disque utilisé par les journaux et autre cache d'exécution",
    sys_cache_clear: "Vider le cache",
    sys_cache_clear_desc: "Vider les fichiers journaux pour libérer de l'espace disque",
    cache_confirm_title: "Vider le cache",
    cache_confirm_message:
        "Le service doit être arrêté avant de vider le cache, sinon les journaux en cours d'écriture risquent de devenir incohérents. Confirmez que le service est arrêté et continuer ?",
    cache_confirm_ok: "Vider",
    cache_confirm_cancel: "Annuler",
    cache_stop_first: "Arrêtez le service avant de vider le cache",
    cache_cleared: "Cache vidé, {size} libérés",
    sys_general: "Général",
    sys_about: "À propos",
    sys_about_desc: "Informations de version et crédits open source",
    sys_download: "Téléchargements",
    sys_speed_limit: "Limite de vitesse (Mio/s)",
    sys_speed_limit_desc:
        "Limite la bande passante utilisée par les téléchargements et la mise en cache de préchargement (Mio/s) ; 0 = illimité",
    sys_download_note:
        "Le préchargement et le mode téléchargement sont déclenchés par les commandes du userscript du navigateur, et non basculés ici : « mettre en cache pendant la lecture » du script correspond au préchargement et « télécharger uniquement » au mode téléchargement ; le mode téléchargement nécessite également que le compte de votre serveur multimédia autorise les téléchargements de ressources",
    sys_trakt: "Scrobbling Trakt.tv",
    sys_trakt_sync_note:
        "À la fin de la lecture, votre visionnage est synchronisé automatiquement avec Trakt : atteindre environ 80 % ou plus marque l'épisode comme vu, en dessous il reste non marqué ; les autres épisodes de la même saison déjà terminés sur votre serveur multimédia sont également marqués, sans dupliquer ceux déjà présents. En dessous de 80 %, votre position est mémorisée pour reprendre plus tard, et l'épisode suivant apparaît dans « Continuer à regarder » ; revoir le même épisode l'enregistre à nouveau — l'autorisation d'un court intervalle est contrôlée par le commutateur « autoriser les doublons » ci-dessous.",
    sys_trakt_dashboard: "Ouvrir le tableau de bord Trakt",
    sys_trakt_setup_title: "Configuration",
    sys_trakt_setup_step1: "1. Créez une application sur Trakt : ",
    sys_trakt_setup_link: "trakt.tv/oauth/applications",
    sys_trakt_setup_step2:
        "2. Définissez la « Redirect uri » de l'application sur l'adresse ci-dessous :",
    sys_trakt_setup_copy: "Copier",
    sys_trakt_setup_copied: "URI de redirection copié",
    sys_trakt_setup_copy_failed:
        "Échec de la copie — veuillez sélectionner et copier manuellement",
    sys_trakt_id: "ID client",
    sys_trakt_id_desc:
        "Obtenu après avoir créé une application sur trakt.tv — laissez vide pour désactiver",
    sys_trakt_id_placeholder: "Laissez vide pour désactiver Trakt",
    sys_trakt_secret: "Secret client",
    sys_trakt_secret_desc:
        "Obtenu après avoir créé une application sur trakt.tv — laissez vide pour désactiver",
    sys_trakt_secret_placeholder: "Laissez vide pour désactiver Trakt",
    sys_trakt_user: "Nom d'utilisateur",
    sys_trakt_user_desc: "Votre nom d'utilisateur Trakt (pas le pseudo affiché)",
    sys_trakt_user_placeholder: "ex. your_trakt_user",
    sys_trakt_host: "Activer l'hôte",
    sys_trakt_host_desc:
        'Mots-clés d\'hôtes séparés par des virgules, vide pour désactiver；ex. emby.local, 192.168.1；entrez "." pour tout activer',
    sys_trakt_host_placeholder: "ex. localhost, 192.168., emby.example.com",
    sys_trakt_dup: "Autoriser le marquage en double",
    sys_trakt_dup_desc:
        "Activé, chaque achèvement re-marque le même épisode/film ; désactivé, une déduplication limitée s'applique : le même élément terminé à nouveau dans la fenêtre de limitation définie ci-dessous n'est marqué qu'une fois (les épisodes antérieurs rattrapés sont toujours dédupliqués)",
    sys_trakt_dup_throttle: "Limitation du marquage en double (secondes)",
    sys_trakt_dup_throttle_desc:
        "Actif lorsque « Autoriser le marquage en double » est désactivé : le même élément terminé à nouveau dans ce nombre de secondes n'est enregistré qu'une fois. Minimum 120 s",
    sys_trakt_dup_throttle_floored:
        "La limitation ne peut être inférieure à 120 secondes ; corrigée à 120",
    sys_bangumi: "Suivi Bangumi.tv",
    sys_bangumi_sync_note:
        "À la fin de la lecture, votre visionnage est synchronisé automatiquement avec Bangumi : atteindre ≥ 80 % marque l'épisode en cours comme vu, en dessous il reste non marqué ; les autres épisodes de la même saison déjà terminés sur votre serveur multimédia sont également ajoutés, sans doublon. S'il n'y a rien à marquer (< 80 % et aucun historique), l'œuvre est mise en « en cours de visionnage » uniquement si la durée de lecture effective est ≥ 20 secondes, sinon elle est ignorée pour éviter les ajouts accidentels.",
    sys_bangumi_host: "Activer l'hôte",
    sys_bangumi_host_desc:
        'Mots-clés d\'hôtes séparés par des virgules, vide pour désactiver；ex. emby.local, 192.168.1；entrez "." pour tout activer',
    sys_bangumi_host_placeholder: "ex. localhost, 192.168., emby.example.com",
    sys_bangumi_user: "Nom d'utilisateur / UID",
    sys_bangumi_user_desc:
        "Nom d'utilisateur bgm.tv ou les chiffres dans bgm.tv/user/123456",
    sys_bangumi_user_placeholder: "ex. 123456",
    sys_bangumi_token: "Jeton d'accès",
    sys_bangumi_token_desc:
        "Généré sur next.bgm.tv/demo/access-token — laissez vide pour désactiver",
    sys_bangumi_token_placeholder: "Laissez vide pour désactiver Bangumi",
    sys_bangumi_private: "Collection privée",
    sys_bangumi_private_desc:
        "Masquer les entrées nouvellement synchronisées de votre profil public",
    sys_bangumi_genres: "Filtre de genres",
    sys_bangumi_genres_desc:
        "Regex comparée aux genres de la série ; seules les séries correspondantes sont synchronisées",
    sys_bangumi_genres_placeholder: "动画|anime",
    sys_bangumi_map: "Mappage d'ID",
    sys_bangumi_map_desc:
        "Épingler une série ou un film tmdb/imdb/tvdb à un sujet Bangumi précis ; priorité absolue. Trois formats de saison : saison entière (S4), plage d'épisodes fermée (S5E1~S5E50, épisodes 1–50 uniquement), plage ouverte (S5E51++, à partir de l'épisode 51). E±N à droite décale l'index local vers le numéro de tri Bangumi. Exemples : tmdb:10000|type:tv|S4 -> bgm:20000|E+59 ; tmdb:10000|type:tv|S5E1~S5E50 -> bgm:20001 ; tmdb:10000|type:tv|S5E51++ -> bgm:20002 ; tmdb:10001|type:movie -> bgm:30000. Sans type, déduit de la saison (une saison signifie TV, sinon film)",
    map_placeholder: "tmdb:10000|type:tv|S4 -> bgm:20000|E+59",
    map_check: "Vérifier et ajouter",
    map_remove: "Supprimer",
    map_copy: "Copier",
    map_group_add: "Nouveau groupe",
    map_group_name_placeholder: "Nom du groupe",
    map_group_add_confirm: "Créer",
    map_group_delete: "Supprimer le groupe",
    map_group_delete_confirm:
        "Supprimer le groupe « {name} » et toutes ses correspondances ?",
    map_item_delete_title: "Supprimer l'entrée",
    map_item_delete_confirm: "Supprimer cette entrée ?\n{entry}",
    map_group_default_label: "Par défaut",
    map_export: "Exporter",
    map_export_done: "Correspondances exportées",
    map_import: "Importer",
    map_import_prefer: "Préférer l'importé (écraser les conflits locaux)",
    map_import_done: "Import terminé : {added} ajoutées, {replaced} remplacées",
    map_import_url: "Importer depuis une URL",
    map_import_url_placeholder: "https://example.com/bangumi_map.json",
    map_import_url_confirm: "Importer",
    cfg_backup_busy: "Sauvegarde en cours…",
    cfg_importing: "Importation…",
    bgm_auto_mark_subject_watched: "Marquer comme vu automatiquement",
    bgm_auto_mark_subject_watched_desc:
        "Marque automatiquement toute la fiche comme vue lorsque tous ses épisodes principaux sont marqués comme vus",
    bgm_mark_watching: "Marquer comme en cours",
    bgm_mark_watching_desc:
        "Activé : un visionnage partiel marque l'œuvre comme en cours. Désactivé : le statut se met à jour uniquement après un épisode complet.",
    map_err_empty: "Saisissez un mappage",
    map_err_format: "Mal formé — attendu « LHS -> RHS »",
    map_err_provider: "Source inconnue ; seuls tmdb / imdb / tvdb sont pris en charge",
    map_err_provider_id: "ID incorrect (tmdb/tvdb numérique, imdb commence par tt)",
    map_err_type: "type doit être tv ou movie",
    map_err_season: "Saison incorrecte ; un entier positif comme S4 est attendu",
    map_err_ep_range:
        "Plage d'épisodes incorrecte ; utilisez S5E106~S5E157 (fermée) ou S5E158++ (ouverte) ; le début ne peut pas dépasser la fin",
    map_err_subject: "ID de sujet Bangumi incorrect ; un entier positif est attendu",
    map_err_offset:
        "Décalage d'épisode incorrect ; un entier comme E+59 ou E-3 est attendu",
    map_err_movie_season: "Un film ne peut pas porter de décalage de saison ou d'épisode",
    map_err_duplicate: "Un mappage identique existe déjà",
    sync_refresh: "Actualiser l'autorisation",
    sync_refreshing: "Actualisation…",
    sync_authorize_opened:
        "Page d'autorisation ouverte — terminez-la dans votre navigateur",
    sync_auth_valid: "L'autorisation est valide",
    sync_start_service_first: "Démarrez d'abord le service",
    sync_refresh_confirm_title: "Actualiser l'autorisation",
    sync_refresh_confirm_message:
        "Actualiser manuellement l'autorisation maintenant ? Si le jeton actuel est invalide, la page d'autorisation s'ouvrira dans votre navigateur.",
    sync_refresh_confirm_ok: "Actualiser",
    sync_test: "Vérifier l'autorisation",
    sync_test_desc: "Vérifier si les identifiants actuels fonctionnent",
    sync_testing: "Vérification…",
    sync_test_ok: "L'autorisation fonctionne",
    sync_test_fail:
        "Échec de l'autorisation — la configuration est peut-être incorrecte ou pas encore autorisée. Cliquez sur « Actualiser l'autorisation » en haut à droite.",
    sync_incomplete:
        "Configuration incomplète — renseignez les champs requis avant de vérifier",

    // Config tab (config file + backup / restore / reset / update)
    page_config: "Configuration",
    cfg_file_title: "Fichier de configuration",
    cfg_backup_title: "Sauvegarde et restauration",
    cfg_backup_now: "Sauvegarder maintenant",
    cfg_backup_now_desc:
        "Empaqueter la configuration actuelle dans une sauvegarde zip horodatée",
    cfg_backup_done: "Configuration sauvegardée",
    cfg_backup_list: "Sauvegardes",
    cfg_backup_list_desc: "Jusqu'à 5 sauvegardes sont conservées — {count} actuellement",
    cfg_backup_empty: "Aucune sauvegarde pour l'instant",
    cfg_view: "Afficher",
    cfg_restore: "Restaurer",
    cfg_delete: "Supprimer",
    cfg_import: "Importer une sauvegarde",
    cfg_import_desc:
        "Importer et restaurer la configuration depuis un fichier zip externe",
    cfg_restore_done: "Configuration restaurée",
    cfg_restore_confirm_title: "Restaurer la configuration",
    cfg_restore_confirm_message:
        "Écraser la configuration actuelle avec la sauvegarde « {name} » ? Cette action est irréversible.",
    cfg_import_confirm_title: "Importer et restaurer la configuration",
    cfg_import_confirm_message:
        "Importer cette sauvegarde et écraser la configuration actuelle ? Cette action est irréversible.",
    cfg_delete_confirm_title: "Supprimer la sauvegarde",
    cfg_delete_confirm_message: "Supprimer la sauvegarde « {name} » ?",
    cfg_reset_title: "Réinitialisation",
    cfg_reset: "Réinitialiser aux valeurs par défaut",
    cfg_reset_desc: "Rétablir tous les paramètres à leurs valeurs par défaut",
    cfg_reset_done: "Configuration réinitialisée aux valeurs par défaut",
    cfg_reset_confirm_title: "Réinitialiser la configuration",
    cfg_reset_confirm_message:
        "Réinitialiser à la configuration par défaut ? La configuration actuelle sera écrasée — cette action est irréversible.",
    cfg_update_title: "Mise à jour",
    cfg_update_auto: "Vérifier automatiquement les mises à jour",
    cfg_update_auto_desc:
        "Vérifier les nouvelles versions sur GitHub au démarrage et afficher un indice dans l'aperçu",
    cfg_update_check: "Vérifier maintenant",
    cfg_update_check_desc:
        "Vérifier dès maintenant sur GitHub si une version plus récente est disponible",
    cfg_update_checking: "Vérification…",
    cfg_update_available: "Nouvelle version v{version} trouvée",
    cfg_update_latest: "Vous utilisez la dernière version v{version}",
    cfg_update_current_ver: "Version actuelle",
    cfg_update_latest_ver: "Dernière version",
    cfg_update_up_to_date: "À jour",
    cfg_update_install: "Télécharger et installer",

    // Update banner (overview)
    ov_update_available: "Nouvelle version v{version} disponible",
    ov_update_action: "Installer la mise à jour",
    ov_update_dismiss: "Ignorer cette version",
    ov_update_downloading: "Téléchargement de la mise à jour…",
    ov_update_failed: "Échec de la mise à jour",
    sys_privacy: "Confidentialité",
    sys_no_progress: "Désactiver le rapport de progression",
    sys_no_progress_desc:
        "Ne pas signaler la progression de lecture au serveur Emby/Jellyfin",
    sys_accent: "Couleur d'accentuation",
    sys_accent_desc:
        "Couleur de mise en évidence de l'interface — affecte les boutons, états actifs et badges",
    sys_center_nav: "Centrer la barre latérale",
    sys_center_nav_desc:
        "Centrer verticalement les onglets de la barre latérale en groupe",

    // Log levels
    log_error: "Error — plantages uniquement",
    log_warn: "Warn — conditions anormales",
    log_info: "Info — par défaut, fonctionnement courant",
    log_debug: "Debug — dépannage",
    log_trace: "Trace — détail complet",

    // Logs page
    page_logs: "Journaux",
    logs_app: "Journal de l'app",
    logs_mpv: "Journal mpv",
    logs_filter: "Filtrer…",
    logs_clear: "Effacer",
    logs_bottom: "↓ Bas",
    logs_empty: "En attente de la sortie des journaux…",
    logs_no_mpv:
        "Aucun journal mpv trouvé — cliquez sur « Choisir le journal mpv » pour en charger un",
    logs_lines: "lignes",
    logs_loading_older: "Chargement des journaux plus anciens…",
    logs_scroll_older:
        "Faites défiler vers le haut pour charger les journaux plus anciens",
    logs_open_folder: "Ouvrir le dossier des journaux",
    logs_pick_mpv: "Choisir le journal mpv",
    logs_reset_mpv: "Réinitialiser par défaut",
    logs_reset_mpv_title:
        "Revenir au journal mpv par défaut dans le dossier des journaux",
    logs_anon: "Anonyme",
    logs_anon_title:
        "Masquer l'ID d'appareil, les jetons, les IP, l'ID utilisateur, l'hôte de l'URL et les noms d'utilisateur Bangumi / Trakt uniquement dans la vue, pratique pour partager des captures d'écran ; le fichier journal n'est pas modifié — l'expurgation du fichier suit toujours le commutateur « Texte sensible »",

    // About modal
    about_thanks: "Crédits",
    about_thanks_desc: "pour une inspiration sans fin",
    about_version_label: "Version",

    // Autostart toasts
    autostart_on: "Lancement à la connexion activé",
    autostart_off: "Lancement à la connexion désactivé",

    // Font size options
    font_12: "12px (compact)",
    font_13: "13px (par défaut)",
    font_14: "14px (confortable)",
    font_15: "15px (grand)",
    font_16: "16px (très grand)",

    // Download
    page_download: "Téléchargements",
    dl_folder: "Dossier de téléchargement",
    dl_folder_desc: "Laisser vide pour utiliser le dossier système par défaut",
    dl_browse: "Parcourir…",
    dl_placeholder: "",
    dl_path_error: "Le chemin n'existe pas, vérifiez la saisie",

    // Bangumi duplicate throttle
    sys_bangumi_dup: "Autoriser les marquages en double",
    sys_bangumi_dup_desc:
        "Quand activé, re-marque le même épisode/film chaque fois que vous finissez de le regarder ; quand désactivé, la déduplication avec limitation est active : le même élément n'est marqué qu'une fois dans la fenêtre de limitation définie ci-dessous",
    sys_bangumi_dup_throttle: "Délai de limitation des marquages en double (secondes)",
    sys_bangumi_dup_throttle_desc:
        "Actif quand Autoriser les marquages en double est désactivé : le même élément est enregistré au maximum une fois pendant ce nombre de secondes ; minimum 120 secondes",
    sys_bangumi_dup_throttle_floored:
        "La limitation ne peut pas être inférieure à 120 s — corrigée à 120",

    // TMDB
    sys_tmdb: "Intégration TMDB",
    sys_tmdb_key: "Clé API",
    sys_tmdb_key_desc:
        "Clé API TMDB pour récupérer les métadonnées manquantes du serveur multimédia lors de la synchronisation.",
    sys_tmdb_api_link: "Créer une clé API",
    sys_tmdb_key_placeholder: "",
};
