import { zhCN } from "./zh-CN";

export const id: typeof zhCN = {
    ...zhCN,

    // App
    app_name: "Genshin",

    // Nav
    nav_overview: "Ikhtisar",
    nav_player: "Pemutar",
    nav_version_prefer: "Versi",
    nav_network: "Jaringan",
    nav_config: "Konfigurasi",
    nav_system: "Sistem",
    nav_logs: "Log",
    nav_sec_play: "Pemutaran",
    nav_sec_settings: "Pengaturan",
    nav_sec_sync: "Sinkronisasi",
    nav_bangumi: "Bangumi",
    nav_trakt: "Trakt",
    nav_sec_debug: "Debug",
    nav_download: "Unduhan",

    // Common
    add: "Tambah",
    add_placeholder: "Ketik dan tekan Enter untuk menambahkan",
    open_dir: "Buka Folder",
    loading: "Memuat konfigurasi…",

    // Overview
    page_overview: "Ikhtisar",
    ov_service: "Layanan Lokal",
    ov_running: "Berjalan",
    ov_stopped: "Berhenti",
    ov_port: "Port",
    ov_port_desc: "Alamat pendengaran lokal",
    ov_uptime: "Waktu Aktif",
    ov_uptime_desc: "Sejak layanan dimulai",
    ov_address: "Alamat",
    ov_address_desc: "Hanya localhost",
    ov_config: "Konfigurasi",
    ov_config_file: "File Konfigurasi",
    ov_config_file_desc: "Lihat atau buka di editor eksternal",
    ov_edit_config: "Edit Konfigurasi",
    ov_restart: "Mulai Ulang Layanan",
    ov_restart_desc:
        "Hentikan layanan, bebaskan sumber daya, dan mulai ulang dengan konfigurasi terbaru",
    ov_about: "Tentang",
    ov_about_desc: "Informasi versi dan ucapan terima kasih open source",
    ov_view: "Lihat",
    ov_start: "Mulai",
    ov_stop: "Hentikan",

    // Toasts
    toast_started: "Layanan dimulai di port {port}",
    toast_stopped: "Layanan dihentikan",
    toast_restarted: "Layanan dimulai ulang di port {port}",
    toast_start_failed: "Gagal memulai layanan",
    toast_stop_failed: "Gagal menghentikan layanan",
    toast_restart_failed: "Gagal memulai ulang layanan",
    toast_open_failed: "Gagal membuka",
    sync_not_configured: "Belum dikonfigurasi — isi kolom terlebih dahulu",

    // Player
    page_player: "Pemutar",
    pl_type: "Jenis Pemutar",
    pl_type_desc: "Pilih pemutar media lokal",
    pl_startup: "Opsi Startup",
    pl_fullscreen: "Layar Penuh",
    pl_fullscreen_desc: "Buka pemutar dalam mode layar penuh",
    pl_mute: "Mulai Dalam Mode Senyap",
    pl_mute_desc: "Mulai dalam mode senyap (mpv --mute=yes)",
    pl_pretty_title: "Judul Cantik",
    pl_pretty_title_desc: "Tambahkan nama server di awal judul jendela pemutar",
    pl_kill_start: "Tutup Saat Startup",
    pl_kill_start_desc: "Tutup proses pemutar yang ada saat startup",
    pl_path: "Path Pemutar",
    pl_path_desc: "Opsional — biarkan kosong untuk menggunakan pemutar dari PATH sistem",
    pl_browse: "Telusuri…",
    pl_path_error: "Path tidak ditemukan — periksa input",
    pl_progress_support:
        "Pelaporan kemajuan: mpv / IINA didukung penuh — pembaruan real-time selama pemutaran, simpan posisi lanjut saat keluar, tandai sebagai ditonton, sinkronisasi dengan Trakt / Bangumi, dan pelacakan per episode. Pemutar lain hanya menyimpan posisi akhir dan menyinkronkan saat keluar, tanpa pelaporan real-time; VLC memutar seluruh musim secara berurutan, MPC dan dandanplay untuk satu episode, pembacaan posisi di PotPlayer hanya berfungsi di Windows",

    // Version prefer
    page_vp: "Preferensi Versi",
    vp_priority: "Urutan Prioritas Versi",
    vp_keywords: "Label Versi",
    vp_keywords_desc:
        "Jika ada beberapa file untuk episode yang sama, file yang jalurnya cocok dengan label paling atas dalam daftar akan dipilih. Contoh: «TeamX → GroupA → StreamB» — jika ketiga versi tersedia, TeamX dipilih; jika tidak, GroupA; dan seterusnya",
    vp_keywords_placeholder: "mis. TeamX, GroupA, StreamB",
    vp_playlist: "Terapkan ke Playlist",
    vp_playlist_desc: "Gunakan preferensi versi saat membuat playlist",
    vp_subtitle: "Preferensi Subtitle",
    vp_sub_priority: "Prioritas Subtitle",
    vp_sub_priority_desc: "Cocokkan kata kunci track subtitle secara berurutan",
    vp_sub_priority_placeholder: "mis. Simplified, CHS",
    vp_sub_extract: "Ekstraksi Subtitle Antar Versi",
    vp_sub_extract_desc:
        "Ekstrak subtitle dari versi lain ketika tidak ditemukan di versi saat ini",
    vp_sub_extract_placeholder: "mis. CHS, Simplified",
    vp_limits: "Batas Playlist",
    vp_max_eps: "Maks. Episode Per Sesi",
    vp_max_eps_desc:
        "Episode dipotong ketika batas ini tercapai; 0 atau kosong berarti tidak terbatas (disarankan: 10–100)",
    vp_last_ep: "Berhenti di Episode Terakhir",
    vp_last_ep_desc:
        "Aktif: saat memutar episode terakhir musim, daftar tidak dibuat dan hanya episode itu yang dibuka (tidak ada lagi setelahnya); Nonaktif: daftar selalu dibuat (saat ini + episode berikutnya)",
    vp_filter: "Sidik Jari Versi",
    vp_filter_desc:
        "Mengekstrak fitur versi dari jalur file yang sedang diputar sebagai «sidik jari». Hanya episode yang jalurnya cocok dengan set fitur yang persis sama yang ditambahkan ke daftar putar, mengunci seluruh musim ke versi yang sama. Contoh: jika regex cocok dengan «TeamX» dan «1080p» di file saat ini, hanya episode yang berisi kedua kata tersebut yang disertakan (kosong = nonaktif)",
    vp_filter_placeholder: "mis. |TeamX|1080p|CHS",
    vp_filter_valid: "Regex valid",
    vp_filter_invalid: "Regex tidak valid",

    // Network
    page_network: "Jaringan",
    net_proxy_http: "Proxy HTTP",
    net_proxy_https: "Proxy HTTPS",
    net_proxy_socks5: "Proxy SOCKS5",
    net_proxy_desc:
        "Isi host:port saja; tempel URL lengkap untuk deteksi skema otomatis; kosongkan untuk menonaktifkan",
    net_proxy_https_desc:
        "Digunakan untuk koneksi terenkripsi (HTTPS); jika kosong, beralih ke proxy HTTP; format sama seperti HTTP",
    net_proxy_socks5_desc:
        "Merutekan semua lalu lintas protokol; ideal untuk jaringan tanpa terowongan HTTP; biarkan kosong untuk menonaktifkan",
    net_proxy_enabled: "Aktifkan Proxy",
    net_proxy_enabled_desc:
        "Saat dinonaktifkan, URL disimpan tetapi semua koneksi langsung; IP privat (192.168.x, 10.x dll.) selalu melewati proxy secara otomatis",
    net_skip_tls: "Lewati Verifikasi TLS",
    net_skip_tls_desc:
        "Untuk server media dengan sertifikat yang ditandatangani sendiri — tidak aman",
    net_redirect: "Deteksi Pengalihan",
    net_redirect_hosts: "Host untuk Memeriksa Pengalihan",
    net_redirect_hosts_desc:
        "URL stream dari host ini diperiksa untuk pengalihan 30x sebelum dikirim ke pemutar (kosong secara default)",

    // System
    page_system: "Sistem",
    sys_appearance: "Tampilan",
    sys_theme: "Tema",
    sys_theme_desc: "Terang, gelap, atau sesuai sistem",
    sys_lang: "Bahasa",
    sys_lang_desc: "Bahasa antarmuka",
    sys_theme_system: "Sistem",
    sys_theme_light: "Terang",
    sys_theme_dark: "Gelap",
    sys_lang_system: "Sistem",
    sys_display: "Layar",
    sys_font_size: "Ukuran Font",
    sys_font_size_desc: "Sesuaikan ukuran teks antarmuka",
    sys_zoom: "Zoom Antarmuka",
    sys_zoom_desc: "Skala umum untuk HiDPI / resolusi tinggi — DPR saat ini: {dpr}",
    sys_font: "Font Antarmuka",
    sys_font_desc: "Pilih font antarmuka",
    sys_font_default: "Default (system-ui)",
    sys_startup: "Startup",
    sys_autostart: "Mulai Saat Login",
    sys_autostart_desc: "Jalankan aplikasi secara otomatis saat login sistem",
    sys_silent_start: "Startup Senyap",
    sys_silent_start_desc:
        "Mulai tersembunyi di system tray tanpa menampilkan jendela utama (lebih senyap dengan startup saat login)",
    sys_logs_title: "Log",
    sys_log_level: "Level Log",
    sys_log_level_desc: "Atur Debug untuk output yang lebih detail saat debugging",
    sys_log_max_size: "Ukuran Log Maks. (MB)",
    sys_log_max_size_desc:
        "Beralih ke file baru ketika file saat ini melebihi ukuran ini (20–200 MB)",
    sys_log_max_size_capped: "Dibatasi maksimum 200 MB",
    sys_log_max_size_floored: "Dinaikkan ke minimum 20 MB",
    sys_log_max_files: "Jumlah File Log Maks.",
    sys_log_max_files_desc:
        "Jumlah file log yang dirotasi untuk disimpan (1–14); yang terlama dihapus",
    sys_log_max_files_capped: "Dibatasi maksimum 14 file",
    sys_log_mask: "Sembunyikan Token Sensitif",
    sys_log_mask_desc: "Ganti teks sensitif dalam log dengan placeholder",
    sys_cache: "Cache",
    sys_cache_size: "Ukuran Cache Saat Ini",
    sys_cache_size_desc: "Ruang disk yang digunakan oleh log dan cache runtime lainnya",
    sys_cache_clear: "Bersihkan Cache",
    sys_cache_clear_desc: "Kosongkan file log untuk membebaskan ruang disk",
    cache_confirm_title: "Bersihkan Cache",
    cache_confirm_message:
        "Layanan harus dihentikan sebelum membersihkan cache, jika tidak log yang sedang ditulis bisa menjadi tidak konsisten. Apakah Anda mengonfirmasi bahwa layanan telah dihentikan dan ingin melanjutkan?",
    cache_confirm_ok: "Bersihkan",
    cache_confirm_cancel: "Batal",
    cache_stop_first: "Hentikan layanan sebelum membersihkan cache",
    cache_cleared: "Cache dibersihkan, membebaskan {size}",
    sys_general: "Umum",
    sys_about: "Tentang",
    sys_about_desc: "Informasi versi dan ucapan terima kasih open source",
    sys_download: "Unduhan",
    sys_speed_limit: "Batas Kecepatan (MiB/s)",
    sys_speed_limit_desc:
        "Membatasi bandwidth yang digunakan oleh unduhan dan pra-cache (MiB/s); 0 = tidak terbatas",
    sys_download_note:
        "Pra-cache dan mode unduhan dipicu oleh perintah skrip pengguna browser, bukan di sini: 'cache saat memutar' dalam skrip adalah pra-cache, dan 'unduh saja' adalah mode unduhan; mode unduhan juga memerlukan akun server media Anda mengizinkan pengunduhan aset",
    sys_trakt: "Scrobbling Trakt.tv",
    sys_trakt_sync_note:
        "Saat pemutaran selesai, tontonan Anda secara otomatis disinkronkan dengan Trakt: mencapai sekitar 80% atau lebih menandai episode sebagai ditonton, kurang dari itu tidak ditandai; episode lain dari musim yang sama yang telah Anda selesaikan di server media juga ditandai, tanpa menduplikasi yang sudah ada. Di bawah 80% posisi Anda disimpan untuk dilanjutkan nanti, dan episode berikutnya muncul di 'Lanjutkan Menonton'; memutar ulang episode yang sama mencatatnya lagi — apakah interval singkat diizinkan dikendalikan oleh toggle 'izinkan duplikat' di bawah.",
    sys_trakt_dashboard: "Buka Dashboard Trakt",
    sys_trakt_setup_title: "Pengaturan",
    sys_trakt_setup_step1: "1. Buat aplikasi di Trakt: ",
    sys_trakt_setup_link: "trakt.tv/oauth/applications",
    sys_trakt_setup_step2: "2. Atur 'Redirect uri' aplikasi ke alamat di bawah:",
    sys_trakt_setup_copy: "Salin",
    sys_trakt_setup_copied: "Redirect URI disalin",
    sys_trakt_setup_copy_failed: "Gagal menyalin — pilih dan salin secara manual",
    sys_trakt_id: "Client ID",
    sys_trakt_id_desc:
        "Diperoleh saat membuat aplikasi di trakt.tv — biarkan kosong untuk menonaktifkan",
    sys_trakt_id_placeholder: "Biarkan kosong untuk menonaktifkan Trakt",
    sys_trakt_secret: "Client Secret",
    sys_trakt_secret_desc:
        "Diperoleh saat membuat aplikasi di trakt.tv — biarkan kosong untuk menonaktifkan",
    sys_trakt_secret_placeholder: "Biarkan kosong untuk menonaktifkan Trakt",
    sys_trakt_user: "Nama Pengguna",
    sys_trakt_user_desc: "Nama pengguna Trakt Anda (bukan nama tampilan)",
    sys_trakt_user_placeholder: "mis. your_trakt_user",
    sys_trakt_host: "Aktifkan Host",
    sys_trakt_host_desc:
        'Kata kunci host yang dipisahkan koma, kosong untuk menonaktifkan；mis. emby.local, 192.168.1；masukkan "." untuk mengaktifkan semua',
    sys_trakt_host_placeholder: "mis. localhost, 192.168., emby.example.com",
    sys_trakt_dup: "Izinkan Penandaan Ulang",
    sys_trakt_dup_desc:
        "Saat aktif, setiap penyelesaian menandai ulang episode/film yang sama; saat nonaktif, deduplikasi dengan throttle diterapkan: item yang sama diselesaikan lagi dalam jendela throttle di bawah hanya ditandai sekali (episode backfill sebelumnya selalu dideduplikasi)",
    sys_trakt_dup_throttle: "Throttle Penandaan Ulang (detik)",
    sys_trakt_dup_throttle_desc:
        "Aktif saat 'Izinkan Penandaan Ulang' dinonaktifkan: item yang sama diselesaikan lagi dalam jumlah detik ini hanya ditandai sekali. Minimum 120 detik",
    sys_trakt_dup_throttle_floored:
        "Throttle tidak boleh kurang dari 120 detik; diperbaiki menjadi 120",
    sys_bangumi: "Pelacakan Bangumi.tv",
    sys_bangumi_sync_note:
        "Saat pemutaran selesai, tontonan Anda secara otomatis disinkronkan dengan Bangumi: mencapai ≥ 80% menandai episode saat ini sebagai ditonton, kurang dari itu tidak ditandai; episode lain dari musim yang sama yang telah Anda selesaikan di server media juga ditambahkan, tanpa menduplikasi yang sudah ada. Jika tidak ada yang ditandai (< 80% dan tidak ada riwayat), karya diatur ke status 'sedang menonton' hanya jika durasi pemutaran efektif ≥ 20 detik, jika tidak dilewati untuk mencegah penambahan tidak sengaja.",
    sys_bangumi_host: "Aktifkan Host",
    sys_bangumi_host_desc:
        'Kata kunci host yang dipisahkan koma, kosong untuk menonaktifkan；mis. emby.local, 192.168.1；masukkan "." untuk mengaktifkan semua',
    sys_bangumi_host_placeholder: "mis. localhost, 192.168., emby.example.com",
    sys_bangumi_user: "Nama Pengguna / UID",
    sys_bangumi_user_desc: "Nama pengguna bgm.tv atau angka di bgm.tv/user/123456",
    sys_bangumi_user_placeholder: "mis. 123456",
    sys_bangumi_token: "Token Akses",
    sys_bangumi_token_desc:
        "Dibuat di next.bgm.tv/demo/access-token — biarkan kosong untuk menonaktifkan",
    sys_bangumi_token_placeholder: "Biarkan kosong untuk menonaktifkan Bangumi",
    sys_bangumi_private: "Koleksi Privat",
    sys_bangumi_private_desc:
        "Sembunyikan entri yang baru disinkronkan dari profil publik Anda",
    sys_bangumi_genres: "Filter Genre",
    sys_bangumi_genres_desc:
        "Regex yang dicocokkan dengan genre serial; hanya serial yang cocok yang disinkronkan",
    sys_bangumi_genres_placeholder: "动画|anime",
    sys_bangumi_map: "Pemetaan ID",
    sys_bangumi_map_desc:
        "Sematkan serial atau film dari tmdb/imdb/tvdb ke objek Bangumi yang tepat; memiliki prioritas tertinggi. Tiga format musim: seluruh musim (S4), rentang episode tertutup (S5E1~S5E50, hanya episode 1–50), rentang terbuka (S5E51++, dari episode 51 dan seterusnya). E±N di sisi kanan menggeser indeks episode lokal ke nomor urut Bangumi. Contoh: tmdb:10000|type:tv|S4 -> bgm:20000|E+59; tmdb:10000|type:tv|S5E1~S5E50 -> bgm:20001; tmdb:10000|type:tv|S5E51++ -> bgm:20002; tmdb:10001|type:movie -> bgm:30000. Tanpa type disimpulkan dari musim (kehadiran musim = TV, jika tidak film)",
    map_placeholder: "tmdb:10000|type:tv|S4 -> bgm:20000|E+59",
    map_check: "Periksa dan Tambah",
    map_remove: "Hapus",
    map_group_add: "Grup Baru",
    map_group_name_placeholder: "Nama grup",
    map_group_add_confirm: "Buat",
    map_group_delete: "Hapus Grup",
    map_group_delete_confirm: 'Hapus grup "{name}" beserta semua pemetaannya?',
    map_item_delete_title: "Hapus Pemetaan",
    map_item_delete_confirm: "Hapus entri ini?\n{entry}",
    map_group_default_label: "Default",
    map_export: "Ekspor",
    map_export_done: "Pemetaan diekspor",
    map_import: "Impor",
    map_import_prefer: "Utamakan yang diimpor (timpa konflik lokal)",
    map_import_done: "Impor selesai: {added} ditambahkan, {replaced} diganti",
    map_import_url: "Impor dari URL",
    map_import_url_placeholder: "https://example.com/bangumi_map.json",
    map_import_url_confirm: "Impor",
    cfg_backup_busy: "Sedang mencadangkan…",
    cfg_importing: "Sedang mengimpor…",
    bgm_mark_watching: "Tandai Sedang Ditonton",
    bgm_mark_watching_desc:
        "Aktif: penayangan sebagian menandai karya sebagai sedang ditonton. Nonaktif: status diperbarui hanya setelah episode selesai ditonton.",
    map_err_empty: "Masukkan pemetaan",
    map_err_format: "Format salah — 'LHS -> RHS' diharapkan",
    map_err_provider: "Penyedia tidak dikenal; hanya tmdb / imdb / tvdb yang didukung",
    map_err_provider_id: "ID salah (tmdb/tvdb numerik, imdb dimulai dengan tt)",
    map_err_type: "type harus tv atau movie",
    map_err_season: "Musim salah; bilangan bulat positif seperti S4 diharapkan",
    map_err_ep_range:
        "Rentang episode tidak valid; gunakan S5E106~S5E157 (tertutup) atau S5E158++ (terbuka); awal tidak boleh lebih besar dari akhir",
    map_err_subject: "ID objek Bangumi salah; bilangan bulat positif diharapkan",
    map_err_offset:
        "Offset episode salah; bilangan bulat seperti E+59 atau E-3 diharapkan",
    map_err_movie_season: "Film tidak boleh memiliki offset musim atau episode",
    map_err_duplicate: "Pemetaan identik sudah ada",
    sync_refresh: "Perbarui Otorisasi",
    sync_refreshing: "Memperbarui…",
    sync_authorize_opened: "Halaman otorisasi dibuka — selesaikan di browser",
    sync_auth_valid: "Otorisasi valid",
    sync_start_service_first: "Mulai layanan terlebih dahulu",
    sync_refresh_confirm_title: "Perbarui Otorisasi",
    sync_refresh_confirm_message:
        "Perbarui otorisasi secara manual sekarang? Jika token saat ini tidak valid, halaman otorisasi akan terbuka di browser Anda.",
    sync_refresh_confirm_ok: "Perbarui",
    sync_test: "Uji Otorisasi",
    sync_test_desc: "Periksa apakah kredensial saat ini berfungsi",
    sync_testing: "Menguji…",
    sync_test_ok: "Otorisasi berfungsi",
    sync_test_fail:
        "Otorisasi gagal — konfigurasi mungkin salah atau belum diotorisasi. Tekan 'Perbarui Otorisasi' di kanan atas.",
    sync_incomplete:
        "Konfigurasi tidak lengkap — isi kolom yang diperlukan sebelum menguji",

    // Config tab
    page_config: "Konfigurasi",
    cfg_file_title: "File Konfigurasi",
    cfg_backup_title: "Cadangan dan Pemulihan",
    cfg_backup_now: "Cadangkan Sekarang",
    cfg_backup_now_desc:
        "Kemas konfigurasi saat ini ke dalam cadangan zip dengan cap waktu",
    cfg_backup_done: "Konfigurasi dicadangkan",
    cfg_backup_list: "Cadangan",
    cfg_backup_list_desc: "Menyimpan hingga 5 cadangan — saat ini {count}",
    cfg_backup_empty: "Belum ada cadangan",
    cfg_view: "Lihat",
    cfg_restore: "Pulihkan",
    cfg_delete: "Hapus",
    cfg_import: "Impor Cadangan",
    cfg_import_desc: "Impor dan pulihkan konfigurasi dari file zip eksternal",
    cfg_restore_done: "Konfigurasi dipulihkan",
    cfg_restore_confirm_title: "Pulihkan Konfigurasi",
    cfg_restore_confirm_message:
        "Timpa konfigurasi saat ini dengan cadangan '{name}'? Tindakan ini tidak dapat dibatalkan.",
    cfg_import_confirm_title: "Impor dan Pulihkan Konfigurasi",
    cfg_import_confirm_message:
        "Impor cadangan ini dan timpa konfigurasi saat ini? Tindakan ini tidak dapat dibatalkan.",
    cfg_delete_confirm_title: "Hapus Cadangan",
    cfg_delete_confirm_message: "Hapus cadangan '{name}'?",
    cfg_reset_title: "Reset",
    cfg_reset: "Reset ke Default",
    cfg_reset_desc: "Pulihkan semua pengaturan ke nilai default",
    cfg_reset_done: "Konfigurasi direset ke default",
    cfg_reset_confirm_title: "Reset Konfigurasi",
    cfg_reset_confirm_message:
        "Reset ke konfigurasi default? Konfigurasi saat ini akan ditimpa — tindakan ini tidak dapat dibatalkan.",
    cfg_update_title: "Pembaruan",
    cfg_update_auto: "Periksa Pembaruan Otomatis",
    cfg_update_auto_desc:
        "Periksa versi baru di GitHub saat startup dan tampilkan prompt di ikhtisar",
    cfg_update_check: "Periksa Sekarang",
    cfg_update_check_desc: "Periksa versi yang lebih baru di GitHub sekarang",
    cfg_update_checking: "Memeriksa…",
    cfg_update_available: "Versi baru v{version} ditemukan",
    cfg_update_latest: "Anda memiliki versi terbaru v{version}",
    cfg_update_current_ver: "Versi saat ini",
    cfg_update_latest_ver: "Versi terbaru",
    cfg_update_up_to_date: "Sudah terkini",
    cfg_update_install: "Unduh & Pasang",

    // Update banner
    ov_update_available: "Versi baru v{version} tersedia",
    ov_update_action: "Pasang pembaruan",
    ov_update_dismiss: "Lewati Versi Ini",
    ov_update_downloading: "Mengunduh pembaruan…",
    ov_update_failed: "Pembaruan gagal",
    sys_privacy: "Privasi",
    sys_no_progress: "Nonaktifkan Pelaporan Kemajuan",
    sys_no_progress_desc: "Jangan laporkan kemajuan pemutaran ke server Emby/Jellyfin",
    sys_accent: "Warna Aksen",
    sys_accent_desc:
        "Warna sorotan antarmuka — memengaruhi tombol, status aktif, dan ikon",
    sys_center_nav: "Pusatkan Sidebar",
    sys_center_nav_desc: "Pusatkan tab sidebar secara vertikal sebagai grup",

    // Log levels
    log_error: "Error — hanya kegagalan",
    log_warn: "Warn — kondisi tidak normal",
    log_info: "Info — operasi normal sehari-hari",
    log_debug: "Debug — debugging",
    log_trace: "Trace — detail lengkap",

    // Logs page
    page_logs: "Log",
    logs_app: "Log Aplikasi",
    logs_mpv: "Log mpv",
    logs_filter: "Filter…",
    logs_clear: "Bersihkan",
    logs_bottom: "↓ Ke Bawah",
    logs_empty: "Menunggu output log…",
    logs_no_mpv: "Log mpv tidak ditemukan — klik 'Pilih Log mpv' untuk memuat",
    logs_lines: "baris",
    logs_loading_older: "Memuat log yang lebih lama…",
    logs_scroll_older: "Gulir ke atas untuk memuat log yang lebih lama",
    logs_open_folder: "Buka Folder Log",
    logs_pick_mpv: "Pilih Log mpv",
    logs_reset_mpv: "Reset ke Default",
    logs_reset_mpv_title: "Kembali ke log mpv default di folder log",
    logs_anon: "Anonim",
    logs_anon_title:
        "Menyembunyikan ID perangkat, token, IP, ID pengguna, URL host, dan nama pengguna Bangumi / Trakt hanya dalam tampilan, berguna untuk berbagi tangkapan layar; file log tidak diubah — sensor file masih dikendalikan oleh toggle 'Teks Sensitif'",

    // About modal
    about_thanks: "Terima Kasih",
    about_thanks_desc: "atas inspirasi tanpa henti",
    about_version_label: "Versi",

    // Autostart toasts
    autostart_on: "Startup saat login diaktifkan",
    autostart_off: "Startup saat login dinonaktifkan",

    // Font size options
    font_12: "12px (kompak)",
    font_13: "13px (default)",
    font_14: "14px (nyaman)",
    font_15: "15px (besar)",
    font_16: "16px (sangat besar)",

    // Download
    page_download: "Unduhan",
    dl_folder: "Folder Unduhan",
    dl_folder_desc: "Biarkan kosong untuk menggunakan folder default sistem",
    dl_browse: "Jelajahi…",
    dl_placeholder: "",
    dl_path_error: "Jalur tidak ada, periksa input Anda",

    // Bangumi duplicate throttle
    sys_bangumi_dup: "Izinkan Penandaan Duplikat",
    sys_bangumi_dup_desc:
        "Saat diaktifkan, menandai ulang episode/film yang sama setiap kali Anda selesai menontonnya; saat dinonaktifkan, deduplikasi dengan pembatasan aktif: entri yang sama hanya ditandai sekali dalam jendela pembatasan yang diatur di bawah",
    sys_bangumi_dup_throttle: "Waktu pembatasan penandaan duplikat (detik)",
    sys_bangumi_dup_throttle_desc:
        "Aktif saat Izinkan Penandaan Duplikat dinonaktifkan: entri yang sama dicatat maksimal sekali dalam detik ini; minimal 120 detik",
    sys_bangumi_dup_throttle_floored:
        "Pembatasan tidak boleh kurang dari 120 detik — dikoreksi menjadi 120",

    // TMDB
    sys_tmdb: "Integrasi TMDB",
    sys_tmdb_key: "Kunci API",
    sys_tmdb_key_desc:
        "Kunci API TMDB untuk mengambil metadata yang hilang dari server media saat sinkronisasi.",
    sys_tmdb_api_link: "Buat kunci API",
    sys_tmdb_key_placeholder: "",
};
