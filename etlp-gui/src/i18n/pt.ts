import { zhCN } from "./zh-CN";

export const pt: typeof zhCN = {
    ...zhCN,

    // App
    app_name: "Genshin",

    // Nav
    nav_overview: "Visão geral",
    nav_player: "Reprodutor",
    nav_version_prefer: "Versão",
    nav_network: "Rede",
    nav_config: "Configuração",
    nav_system: "Sistema",
    nav_logs: "Registos",
    nav_sec_play: "Reprodução",
    nav_sec_settings: "Definições",
    nav_sec_sync: "Sincronização",
    nav_bangumi: "Bangumi",
    nav_trakt: "Trakt",
    nav_sec_debug: "Depuração",

    // Common
    add: "Adicionar",
    add_placeholder: "Escreva e prima Enter para adicionar",
    open_dir: "Abrir pasta",
    loading: "A carregar configuração…",

    // Overview
    page_overview: "Visão geral",
    ov_service: "Serviço local",
    ov_running: "Em execução",
    ov_stopped: "Parado",
    ov_port: "Porta",
    ov_port_desc: "Endereço de escuta local",
    ov_uptime: "Tempo ativo",
    ov_uptime_desc: "Desde o início do serviço",
    ov_address: "Endereço",
    ov_address_desc: "Apenas localhost",
    ov_config: "Definições",
    ov_config_file: "Ficheiro de configuração",
    ov_config_file_desc: "Visualizar ou abrir num editor externo",
    ov_edit_config: "Editar configuração",
    ov_restart: "Reiniciar serviço",
    ov_restart_desc:
        "Parar o serviço, libertar recursos e reiniciar com a configuração mais recente",
    ov_about: "Sobre",
    ov_about_desc: "Informações da versão e créditos de código aberto",
    ov_view: "Ver",
    ov_start: "Iniciar",
    ov_stop: "Parar",

    // Toasts
    toast_started: "Serviço iniciado na porta {port}",
    toast_stopped: "Serviço parado",
    toast_restarted: "Serviço reiniciado na porta {port}",
    toast_start_failed: "Falha ao iniciar o serviço",
    toast_stop_failed: "Falha ao parar o serviço",
    toast_restart_failed: "Falha ao reiniciar o serviço",
    toast_open_failed: "Falha ao abrir",
    sync_not_configured: "Ainda não configurado — preencha primeiro os campos",

    // Player
    page_player: "Reprodutor",
    pl_type: "Tipo de reprodutor",
    pl_type_desc: "Escolha um reprodutor multimédia local",
    pl_startup: "Opções de arranque",
    pl_fullscreen: "Ecrã inteiro",
    pl_fullscreen_desc: "Iniciar o reprodutor em modo de ecrã inteiro",
    pl_mute: "Iniciar sem som",
    pl_mute_desc: "Iniciar sem som (mpv --mute=yes)",
    pl_pretty_title: "Título elegante",
    pl_pretty_title_desc: "Antepor o nome do servidor ao título da janela do reprodutor",
    pl_kill_start: "Terminar ao arrancar",
    pl_kill_start_desc: "Terminar os processos do reprodutor existentes ao arrancar",
    pl_path: "Caminho do reprodutor",
    pl_path_desc: "Opcional — deixe vazio para usar o reprodutor do PATH do sistema",
    pl_browse: "Procurar…",
    pl_path_error: "Caminho não encontrado — verifique a entrada",
    pl_progress_support:
        "Relatório de progresso: mpv / IINA são totalmente suportados — atualizações em direto durante a reprodução, posição de retoma reescrita ao sair, marcação como visto, sincronização com Trakt / Bangumi e rastreio por episódio. Outros reprodutores apenas escrevem a posição final e sincronizam ao sair, sem relatório em direto durante a reprodução; o VLC reproduz toda a temporada de seguida, MPC e dandanplay são de episódio único, e a releitura de posição do PotPlayer é só para Windows",

    // Version prefer
    page_vp: "Preferência de versão",
    vp_priority: "Prioridade de versão",
    vp_keywords: "Palavras-chave de versão",
    vp_keywords_desc:
        "Corresponder as palavras-chave de versão do média por ordem — as entradas anteriores prevalecem",
    vp_keywords_placeholder: "ex. VCB-Studio, ANi, DBD-Raws",
    vp_playlist: "Aplicar à lista de reprodução",
    vp_playlist_desc: "Usar a prioridade de versão ao criar a lista de reprodução",
    vp_subtitle: "Preferência de legendas",
    vp_sub_priority: "Prioridade de legendas",
    vp_sub_priority_desc:
        "Corresponder as palavras-chave das faixas de legendas por ordem",
    vp_sub_priority_placeholder: "ex. Simplificado, CHS",
    vp_sub_extract: "Extração de legendas entre versões",
    vp_sub_extract_desc:
        "Extrair legendas de outras versões quando não forem encontradas na atual",
    vp_sub_extract_placeholder: "ex. CHS, Simplificado",
    vp_limits: "Limites da lista de reprodução",
    vp_max_eps: "Máx. de episódios por sessão",
    vp_max_eps_desc:
        "Os episódios são truncados ao atingir este limite; 0 ou vazio significa ilimitado (recomendado: 10–100)",
    vp_last_ep: "Desativar no último episódio",
    vp_last_ep_desc:
        "Ligado: ao reproduzir o último episódio da temporada, não cria lista de reprodução e abre apenas esse episódio (nada o segue); Desligado: cria sempre a lista de reprodução (episódio atual + posteriores)",
    vp_filter: "Regex de filtro de versão",
    vp_filter_desc:
        "Só as versões que correspondem a esta regex são adicionadas à lista de reprodução (vazio = sem filtro)",
    vp_filter_placeholder: "ex. |VCB-Studio|ANi|Simplificado",
    vp_filter_valid: "Regex válida",
    vp_filter_invalid: "Regex inválida",

    // Network
    page_network: "Rede",
    net_proxy: "Proxy HTTP",
    net_proxy_desc: "Formato: host:port (deixe vazio para desativar)",
    net_skip_tls: "Ignorar verificação TLS",
    net_skip_tls_desc: "Para servidores Emby autoassinados — inseguro",
    net_redirect: "Deteção de redirecionamentos",
    net_redirect_hosts: "Anfitriões a sondar para redirecionamentos",
    net_redirect_hosts_desc:
        "Os URL de transmissão destes anfitriões são sondados para redirecionamentos 30x antes de serem entregues ao reprodutor (vazio por predefinição)",

    // System
    page_system: "Sistema",
    sys_appearance: "Aspeto",
    sys_theme: "Tema",
    sys_theme_desc: "Claro, escuro ou seguir o sistema",
    sys_lang: "Idioma",
    sys_lang_desc: "Idioma de apresentação da interface",
    sys_theme_system: "Sistema",
    sys_theme_light: "Claro",
    sys_theme_dark: "Escuro",
    sys_lang_system: "Sistema",
    sys_display: "Ecrã",
    sys_font_size: "Tamanho da letra",
    sys_font_size_desc: "Ajustar o tamanho do texto da interface",
    sys_zoom: "Escala da interface",
    sys_zoom_desc: "Zoom global HiDPI / alta resolução — DPR atual: {dpr}",
    sys_font: "Tipo de letra da interface",
    sys_font_desc: "Escolher o tipo de letra da interface",
    sys_font_default: "Predefinido (system-ui)",
    sys_startup: "Arranque",
    sys_autostart: "Iniciar ao iniciar sessão",
    sys_autostart_desc: "Iniciar a aplicação automaticamente após iniciar sessão",
    sys_silent_start: "Arranque silencioso",
    sys_silent_start_desc:
        "Iniciar oculto na bandeja sem mostrar a janela principal (mais discreto com o arranque ao iniciar sessão)",
    sys_logs_title: "Registos",
    sys_log_level: "Nível de registo",
    sys_log_level_desc:
        "Defina como Debug para uma saída mais detalhada na resolução de problemas",
    sys_log_max_size: "Tamanho máx. do registo (MB)",
    sys_log_max_size_desc:
        "Rodar para um novo ficheiro quando o atual exceder este tamanho (20–200 MB)",
    sys_log_max_size_capped: "Limitado ao máximo de 200 MB",
    sys_log_max_size_floored: "Aumentado para o mínimo de 20 MB",
    sys_log_max_files: "Máx. de ficheiros de registo",
    sys_log_max_files_desc:
        "Número de ficheiros de registo rodados a manter (1–14); o mais antigo é removido",
    sys_log_max_files_capped: "Limitado ao máximo de 14 ficheiros",
    sys_log_mask: "Mascarar tokens sensíveis",
    sys_log_mask_desc:
        "Substituir o texto sensível nos registos por marcadores de posição",
    sys_cache: "Cache",
    sys_cache_size: "Tamanho atual da cache",
    sys_cache_size_desc: "Espaço em disco usado pelos registos e outra cache de execução",
    sys_cache_clear: "Limpar cache",
    sys_cache_clear_desc:
        "Esvaziar os ficheiros de registo para libertar espaço em disco",
    cache_confirm_title: "Limpar cache",
    cache_confirm_message:
        "O serviço tem de ser parado antes de limpar a cache, caso contrário os registos a serem escritos podem ficar inconsistentes. Confirma que o serviço está parado e prossegue?",
    cache_confirm_ok: "Limpar",
    cache_confirm_cancel: "Cancelar",
    cache_stop_first: "Pare o serviço antes de limpar a cache",
    cache_cleared: "Cache limpa, libertados {size}",
    sys_general: "Geral",
    sys_about: "Sobre",
    sys_about_desc: "Informações da versão e créditos de código aberto",
    sys_download: "Transferências",
    sys_speed_limit: "Limite de velocidade (MiB/s)",
    sys_speed_limit_desc:
        "Limita a largura de banda usada por transferências e cache de pré-carregamento (MiB/s); 0 = ilimitado",
    sys_download_note:
        "O pré-carregamento e o modo de transferência são acionados pelos comandos do userscript do navegador, não alternados aqui: «colocar em cache durante a reprodução» do script é o pré-carregamento e «só transferir» é o modo de transferência; o modo de transferência também requer que a conta do seu servidor multimédia permita transferências de recursos",
    sys_trakt: "Scrobbling do Trakt.tv",
    sys_trakt_sync_note:
        "Quando a reprodução termina, a sua visualização é sincronizada automaticamente com o Trakt: atingir cerca de 80 % ou mais marca o episódio como visto, abaixo disso permanece sem marca; outros episódios da mesma temporada que já concluiu no seu servidor multimédia também são marcados, sem duplicar os já existentes. Abaixo de 80 %, a sua posição é memorizada para retomar mais tarde, e o episódio seguinte aparece em «Continuar a ver»; rever o mesmo episódio regista-o novamente — se é permitido um curto intervalo é controlado pelo interruptor «permitir duplicados» abaixo.",
    sys_trakt_dashboard: "Abrir o painel do Trakt",
    sys_trakt_setup_title: "Configuração",
    sys_trakt_setup_step1: "1. Crie uma aplicação no Trakt: ",
    sys_trakt_setup_link: "trakt.tv/oauth/applications",
    sys_trakt_setup_step2:
        "2. Defina o «Redirect uri» da aplicação para o endereço abaixo:",
    sys_trakt_setup_copy: "Copiar",
    sys_trakt_setup_copied: "URI de redirecionamento copiado",
    sys_trakt_setup_copy_failed: "Falha ao copiar — selecione e copie manualmente",
    sys_trakt_id: "Client ID",
    sys_trakt_id_desc:
        "Obtido após criar uma aplicação no trakt.tv — deixe vazio para desativar",
    sys_trakt_id_placeholder: "Deixe vazio para desativar o Trakt",
    sys_trakt_secret: "Client Secret",
    sys_trakt_secret_desc:
        "Obtido após criar uma aplicação no trakt.tv — deixe vazio para desativar",
    sys_trakt_secret_placeholder: "Deixe vazio para desativar o Trakt",
    sys_trakt_user: "Nome de utilizador",
    sys_trakt_user_desc: "O seu nome de utilizador do Trakt (não a alcunha apresentada)",
    sys_trakt_user_placeholder: "ex. your_trakt_user",
    sys_trakt_host: "Ativar anfitrião",
    sys_trakt_host_desc:
        "Palavras-chave de anfitrião separadas por vírgulas; deixe vazio para desativar, um único ponto ativa todos",
    sys_trakt_host_placeholder: "ex. localhost, 192.168., emby.example.com",
    sys_trakt_dup: "Permitir marcação duplicada",
    sys_trakt_dup_desc:
        "Se ativado, cada conclusão volta a marcar o mesmo episódio/filme; se desativado, aplica-se a desduplicação limitada: o mesmo item terminado novamente dentro da janela de limitação definida abaixo é marcado apenas uma vez (os episódios anteriores preenchidos são sempre desduplicados)",
    sys_trakt_dup_throttle: "Limitação de marcação duplicada (segundos)",
    sys_trakt_dup_throttle_desc:
        "Eficaz quando «Permitir marcação duplicada» está desativado: o mesmo item terminado novamente dentro destes segundos é registado apenas uma vez. Mínimo 120 s",
    sys_trakt_dup_throttle_floored:
        "A limitação não pode ser inferior a 120 segundos; corrigida para 120",
    sys_bangumi: "Rastreio do Bangumi.tv",
    sys_bangumi_sync_note:
        "Quando a reprodução termina, a sua visualização é sincronizada automaticamente com o Bangumi: atingir cerca de 80 % ou mais marca o episódio como visto, abaixo disso permanece sem marca; outros episódios da mesma temporada que já concluiu no seu servidor multimédia também são marcados, sem duplicar os já existentes. Marcá-lo como visto também define a obra como «a ver».",
    sys_bangumi_host: "Ativar anfitrião",
    sys_bangumi_host_desc:
        "Palavras-chave de anfitrião separadas por vírgulas; deixe vazio para desativar, um único ponto ativa todos",
    sys_bangumi_host_placeholder: "ex. localhost, 192.168., emby.example.com",
    sys_bangumi_user: "Nome de utilizador / UID",
    sys_bangumi_user_desc:
        "Nome de utilizador do bgm.tv ou os dígitos em bgm.tv/user/123456",
    sys_bangumi_user_placeholder: "ex. 123456",
    sys_bangumi_token: "Token de acesso",
    sys_bangumi_token_desc:
        "Gerado em next.bgm.tv/demo/access-token — deixe vazio para desativar",
    sys_bangumi_token_placeholder: "Deixe vazio para desativar o Bangumi",
    sys_bangumi_private: "Coleção privada",
    sys_bangumi_private_desc:
        "Ocultar as entradas recém-sincronizadas do seu perfil público",
    sys_bangumi_genres: "Filtro de géneros",
    sys_bangumi_genres_desc:
        "Regex comparada com os géneros da série; só as séries correspondentes são sincronizadas",
    sys_bangumi_genres_placeholder: "动画|anime",
    sys_bangumi_map: "Mapeamento de ID",
    sys_bangumi_map_desc:
        "Fixar uma série ou filme tmdb/imdb/tvdb a um assunto exato do Bangumi; tem prioridade máxima. Três formatos de temporada: temporada completa (S4), intervalo de episódios fechado (S5E1~S5E50, apenas episódios 1–50), intervalo aberto (S5E51++, a partir do episódio 51). E±N à direita desloca o índice do episódio local para o número de ordenação do Bangumi. Exemplos: tmdb:10000|type:tv|S4 -> bgm:20000|E+59; tmdb:10000|type:tv|S5E1~S5E50 -> bgm:20001; tmdb:10000|type:tv|S5E51++ -> bgm:20002; tmdb:10001|type:movie -> bgm:30000. Sem type, é inferido a partir da temporada (uma temporada significa TV, caso contrário filme)",
    map_placeholder: "tmdb:10000|type:tv|S4 -> bgm:20000|E+59",
    map_check: "Verificar e adicionar",
    map_remove: "Remover",
    map_err_empty: "Introduza um mapeamento",
    map_err_format: "Mal formado — esperado «LHS -> RHS»",
    map_err_provider: "Origem desconhecida; só tmdb / imdb / tvdb são suportados",
    map_err_provider_id: "ID incorreto (tmdb/tvdb numérico, imdb começa por tt)",
    map_err_type: "type tem de ser tv ou movie",
    map_err_season: "Temporada incorreta; esperado um inteiro positivo como S4",
    map_err_ep_range:
        "Intervalo de episódios inválido; use S5E106~S5E157 (fechado) ou S5E158++ (aberto); o início não pode ser maior que o fim",
    map_err_subject: "ID de assunto do Bangumi incorreto; esperado um inteiro positivo",
    map_err_offset: "Desvio de episódio incorreto; esperado um inteiro como E+59 ou E-3",
    map_err_movie_season: "Um filme não pode ter desvio de temporada ou episódio",
    map_err_duplicate: "Já existe um mapeamento idêntico",
    sync_refresh: "Atualizar autorização",
    sync_refreshing: "A atualizar…",
    sync_authorize_opened: "Página de autorização aberta — conclua-a no seu navegador",
    sync_auth_valid: "A autorização é válida",
    sync_start_service_first: "Inicie primeiro o serviço",
    sync_refresh_confirm_title: "Atualizar autorização",
    sync_refresh_confirm_message:
        "Atualizar a autorização manualmente agora? Se o token atual for inválido, a página de autorização abrirá no seu navegador.",
    sync_refresh_confirm_ok: "Atualizar",
    sync_test: "Verificar autorização",
    sync_test_desc: "Verificar se as credenciais atuais funcionam",
    sync_testing: "A verificar…",
    sync_test_ok: "A autorização funciona",
    sync_test_fail:
        "Falha na autorização — a configuração pode estar errada ou ainda não autorizada. Clique em «Atualizar autorização» no canto superior direito.",
    sync_incomplete:
        "Configuração incompleta — preencha os campos obrigatórios antes de verificar",

    // Config tab (config file + backup / restore / reset / update)
    page_config: "Configuração",
    cfg_file_title: "Ficheiro de configuração",
    cfg_backup_title: "Cópia de segurança e restauro",
    cfg_backup_now: "Fazer cópia agora",
    cfg_backup_now_desc: "Empacotar a configuração atual numa cópia zip com data e hora",
    cfg_backup_done: "Configuração copiada",
    cfg_backup_list: "Cópias de segurança",
    cfg_backup_list_desc: "São mantidas até 5 cópias — {count} agora",
    cfg_backup_empty: "Ainda sem cópias de segurança",
    cfg_view: "Ver",
    cfg_restore: "Restaurar",
    cfg_delete: "Eliminar",
    cfg_import: "Importar cópia",
    cfg_import_desc:
        "Importar e restaurar a configuração a partir de um ficheiro zip externo",
    cfg_restore_done: "Configuração restaurada",
    cfg_restore_confirm_title: "Restaurar configuração",
    cfg_restore_confirm_message:
        "Substituir a configuração atual pela cópia «{name}»? Isto não pode ser anulado.",
    cfg_import_confirm_title: "Importar e restaurar configuração",
    cfg_import_confirm_message:
        "Importar esta cópia e substituir a configuração atual? Isto não pode ser anulado.",
    cfg_delete_confirm_title: "Eliminar cópia de segurança",
    cfg_delete_confirm_message: "Eliminar a cópia «{name}»?",
    cfg_reset_title: "Repor",
    cfg_reset: "Repor predefinições",
    cfg_reset_desc: "Restaurar todas as definições para os valores predefinidos",
    cfg_reset_done: "Configuração reposta para as predefinições",
    cfg_reset_confirm_title: "Repor configuração",
    cfg_reset_confirm_message:
        "Repor a configuração predefinida? A configuração atual será substituída — isto não pode ser anulado.",
    cfg_update_title: "Atualização",
    cfg_update_auto: "Procurar atualizações automaticamente",
    cfg_update_auto_desc:
        "Procurar novas versões no GitHub ao arrancar e mostrar uma dica na visão geral",
    cfg_update_check: "Procurar agora",
    cfg_update_check_desc: "Procurar agora no GitHub uma versão mais recente",
    cfg_update_checking: "A procurar…",
    cfg_update_available: "Nova versão v{version} encontrada",
    cfg_update_latest: "Está na versão mais recente v{version}",
    cfg_update_current_ver: "Atual: v{version}",
    cfg_update_latest_ver: "Mais recente: v{version}",
    cfg_update_up_to_date: "Atualizado",
    cfg_update_install: "Baixar e instalar",

    // Update banner (overview)
    ov_update_available: "Nova versão v{version} disponível",
    ov_update_action: "Instalar atualização",
    ov_update_dismiss: "Ignorar esta versão",
    ov_update_downloading: "Baixando atualização…",
    ov_update_failed: "Falha na atualização",
    sys_privacy: "Privacidade",
    sys_no_progress: "Desativar relatório de progresso",
    sys_no_progress_desc:
        "Não comunicar o progresso de reprodução ao servidor Emby/Jellyfin",
    sys_accent: "Cor de destaque",
    sys_accent_desc:
        "Cor de realce da interface — afeta botões, estados ativos e emblemas",
    sys_center_nav: "Centrar barra lateral",
    sys_center_nav_desc:
        "Centrar verticalmente os separadores da barra lateral como grupo",

    // Log levels
    log_error: "Error — apenas falhas",
    log_warn: "Warn — condições anómalas",
    log_info: "Info — predefinido, funcionamento diário",
    log_debug: "Debug — resolução de problemas",
    log_trace: "Trace — detalhe completo",

    // Logs page
    page_logs: "Registos",
    logs_app: "Registo da app",
    logs_mpv: "Registo do mpv",
    logs_filter: "Filtrar…",
    logs_clear: "Limpar",
    logs_bottom: "↓ Fundo",
    logs_empty: "À espera da saída do registo…",
    logs_no_mpv:
        "Nenhum registo do mpv encontrado — clique em «Escolher registo do mpv» para carregar um",
    logs_lines: "linhas",
    logs_loading_older: "A carregar registos mais antigos…",
    logs_scroll_older: "Desloque para cima para carregar registos mais antigos",
    logs_open_folder: "Abrir pasta de registos",
    logs_pick_mpv: "Escolher registo do mpv",
    logs_reset_mpv: "Repor predefinição",
    logs_reset_mpv_title: "Voltar ao registo do mpv predefinido na pasta de registos",
    logs_anon: "Anónimo",
    logs_anon_title:
        "Ocultar ID do dispositivo, tokens, IP, ID de utilizador, anfitrião do URL e nomes de utilizador do Bangumi / Trakt apenas na vista, útil para partilhar capturas de ecrã; o ficheiro de registo não é alterado — a censura do ficheiro continua a seguir o interruptor «Texto sensível»",

    // About modal
    about_thanks: "Créditos",
    about_thanks_desc: "pela inspiração inesgotável",
    about_version_label: "Versão",

    // Autostart toasts
    autostart_on: "Arranque ao iniciar sessão ativado",
    autostart_off: "Arranque ao iniciar sessão desativado",

    // Font size options
    font_12: "12px (compacto)",
    font_13: "13px (predefinido)",
    font_14: "14px (confortável)",
    font_15: "15px (grande)",
    font_16: "16px (muito grande)",
};
