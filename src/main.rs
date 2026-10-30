#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use iced::widget::image::Handle;
use iced::widget::{
    button, canvas, column, container, pick_list, progress_bar, row, rule, scrollable, slider,
    text, text_input,
};
use iced::{Alignment, Background, Border, Color, ContentFit, Element, Fill, Padding, Theme};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use thiserror::Error;
use tokio::io::{AsyncReadExt, BufReader};
use tokio::sync::watch;
use url::Url;

const APP_BG: Color = Color::from_rgb8(0x10, 0x11, 0x14);
const SIDEBAR_BG: Color = Color::from_rgb8(0x16, 0x17, 0x1B);
const SURFACE: Color = Color::from_rgb8(0x1C, 0x1E, 0x24);
const SURFACE_RAISED: Color = Color::from_rgb8(0x26, 0x28, 0x30);
const INPUT_BG: Color = Color::from_rgb8(0x12, 0x13, 0x18);
const LINE: Color = Color::from_rgb8(0x34, 0x36, 0x3E);
const LINE_HOVER: Color = Color::from_rgb8(0x4E, 0x50, 0x5A);
const TEXT: Color = Color::from_rgb8(0xF3, 0xF0, 0xE8);
const MUTED: Color = Color::from_rgb8(0xA3, 0x9E, 0x93);
const FAINT: Color = Color::from_rgb8(0x6E, 0x6A, 0x62);
const ACCENT: Color = Color::from_rgb8(0xD4, 0xA6, 0x56);
const ACCENT_HOVER: Color = Color::from_rgb8(0xE3, 0xBC, 0x74);
const ACCENT_PRESS: Color = Color::from_rgb8(0xC4, 0x92, 0x3E);
const ACCENT_WASH: Color = Color::from_rgba8(212, 166, 86, 0.16);
const INK: Color = Color::from_rgb8(0x1A, 0x14, 0x0C);
const SUCCESS: Color = Color::from_rgb8(0x8F, 0xBF, 0x9F);
const DANGER: Color = Color::from_rgb8(0xE7, 0xB2, 0xA8);
const TRACK: Color = Color::from_rgb8(0x2C, 0x2E, 0x36);
#[cfg(any(target_os = "linux", test))]
const APP_ID: &str = "clipforge";
#[cfg(target_os = "linux")]
const ICON_48: &[u8] = include_bytes!("../assets/icons/clipforge-48.png");
const ICON_64: &[u8] = include_bytes!("../assets/icons/clipforge-64.png");
#[cfg(target_os = "linux")]
const ICON_128: &[u8] = include_bytes!("../assets/icons/clipforge-128.png");
const ICON_256: &[u8] = include_bytes!("../assets/icons/clipforge-256.png");
const PREVIEW_WINDOW_TITLE: &str = "ClipForge preview";

fn main() -> iced::Result {
    env_logger::Builder::from_env(
        env_logger::Env::default()
            .default_filter_or("warn,wgpu_hal=error,wgpu_core=error,naga=error"),
    )
    .init();
    #[cfg(target_os = "linux")]
    install_desktop_integration();
    iced::application(App::new, App::update, App::view)
        .title("ClipForge")
        .theme(|_: &App| clipforge_theme())
        .window(window_settings())
        .subscription(|_| {
            iced::time::every(Duration::from_millis(300)).map(|_| Message::ProgressTick)
        })
        .run()
}

fn clipforge_theme() -> Theme {
    use std::sync::LazyLock;

    static THEME: LazyLock<Theme> = LazyLock::new(|| {
        Theme::custom(
            "ClipForge",
            iced::theme::Palette {
                background: APP_BG,
                text: TEXT,
                primary: ACCENT,
                success: SUCCESS,
                warning: ACCENT,
                danger: Color::from_rgb8(0xC4, 0x54, 0x4A),
            },
        )
    });
    THEME.clone()
}

fn window_settings() -> iced::window::Settings {
    iced::window::Settings {
        size: iced::Size::new(1200.0, 820.0),
        icon: app_icon(),
        platform_specific: desktop_platform(),
        ..Default::default()
    }
}

#[cfg(target_os = "linux")]
fn desktop_platform() -> iced::window::settings::PlatformSpecific {
    iced::window::settings::PlatformSpecific {
        application_id: APP_ID.to_owned(),
        ..Default::default()
    }
}

#[cfg(not(target_os = "linux"))]
fn desktop_platform() -> iced::window::settings::PlatformSpecific {
    iced::window::settings::PlatformSpecific::default()
}

fn app_icon() -> Option<iced::window::Icon> {
    let (width, height, pixels) = decode_icon(ICON_64)?;
    iced::window::icon::from_rgba(pixels, width, height).ok()
}

fn decode_icon(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(
        png::Transformations::EXPAND | png::Transformations::ALPHA | png::Transformations::STRIP_16,
    );
    let mut reader = decoder.read_info().ok()?;
    let mut buffer = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buffer).ok()?;
    let pixels = buffer[..info.buffer_size()].to_vec();
    let expected = (info.width as usize)
        .checked_mul(info.height as usize)?
        .checked_mul(4)?;
    if info.color_type != png::ColorType::Rgba || pixels.len() != expected {
        return None;
    }
    Some((info.width, info.height, pixels))
}

#[cfg(target_os = "linux")]
fn install_desktop_integration() {
    let Some(data_home) = xdg_data_home() else {
        return;
    };
    let mut changed = false;
    let mut icon_path = None;
    for (size, bytes) in [
        (48, ICON_48),
        (64, ICON_64),
        (128, ICON_128),
        (256, ICON_256),
    ] {
        let path = data_home.join(format!("icons/hicolor/{size}x{size}/apps/{APP_ID}.png"));
        if let Some(wrote) = write_if_changed(&path, bytes) {
            changed |= wrote;
            if size == 256 {
                icon_path = Some(path);
            }
        }
    }
    let Some(icon_path) = icon_path else {
        return;
    };
    let desktop_path = data_home
        .join("applications")
        .join(format!("{APP_ID}.desktop"));
    let executable = std::env::var("APPIMAGE")
        .ok()
        .filter(|value| !value.is_empty() && !value.contains(['\n', '\r']))
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .and_then(|path| path.into_os_string().into_string().ok())
        })
        .unwrap_or_else(|| APP_ID.to_owned());
    let icon = icon_path.to_string_lossy().replace(['\n', '\r'], "");
    let Some(desktop_changed) =
        write_if_changed(&desktop_path, desktop_entry(&executable, &icon).as_bytes())
    else {
        return;
    };
    if changed || desktop_changed {
        refresh_desktop_cache(&data_home);
    }
}

#[cfg(target_os = "linux")]
fn xdg_data_home() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
}

#[cfg(target_os = "linux")]
fn write_if_changed(path: &std::path::Path, bytes: &[u8]) -> Option<bool> {
    if std::fs::read(path).ok().as_deref() == Some(bytes) {
        return Some(false);
    }
    std::fs::create_dir_all(path.parent()?).ok()?;
    std::fs::write(path, bytes).ok()?;
    Some(true)
}

#[cfg(any(target_os = "linux", test))]
fn desktop_entry(executable: &str, icon: &str) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName=ClipForge\nComment=Download an authorized video interval\nExec={}\nIcon={icon}\nStartupWMClass={APP_ID}\nTerminal=false\nCategories=AudioVideo;\n",
        desktop_exec(executable),
    )
}

#[cfg(any(target_os = "linux", test))]
fn desktop_exec(executable: &str) -> String {
    if executable.contains([' ', '\t', '"', '\\']) {
        format!(
            "\"{}\"",
            executable.replace('\\', "\\\\").replace('"', "\\\"")
        )
    } else {
        executable.to_owned()
    }
}

#[cfg(target_os = "linux")]
fn refresh_desktop_cache(data_home: &std::path::Path) {
    let _ = std::process::Command::new("update-desktop-database")
        .arg(data_home.join("applications"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = std::process::Command::new("gtk-update-icon-cache")
        .args(["-f", "-t"])
        .arg(data_home.join("icons/hicolor"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = std::process::Command::new("kbuildsycoca6")
        .arg("--noincremental")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[derive(Default)]
struct App {
    url: String,
    notice: Option<String>,
    video: Option<VideoInfo>,
    start: String,
    end: String,
    destination: String,
    loading: bool,
    exporting: bool,
    previewing: bool,
    preview_active: bool,
    preview_player: Option<PreviewPlayer>,
    preview_volume: u8,
    history_prompt: Option<HistoryPrompt>,
    cancel_export: Option<watch::Sender<bool>>,
    history: Vec<String>,
    selected_format: Option<FormatChoice>,
    output_format: ExportFormat,
    thumbnail: Option<iced::widget::image::Handle>,
    progress: Option<Arc<DownloadProgress>>,
    preview_progress: Option<Arc<PreviewProgress>>,
    retry_available: bool,
    language: Language,
    page: Page,
    thumbnail_pending: bool,
    thumbnail_generation: u64,
    toolchain: Toolchain,
    installing_tools: bool,
    install_progress: Option<Arc<InstallProgress>>,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Language {
    #[default]
    English,
    Portuguese,
}
impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::English => "English",
            Self::Portuguese => "Português",
        })
    }
}
#[derive(Default)]
struct DownloadProgress {
    percent: AtomicU8,
    stage: AtomicU8,
    details: Mutex<ProgressDetails>,
}
#[derive(Default)]
struct ProgressDetails {
    speed: Option<String>,
    eta: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum ExportStage {
    Preparing,
    Downloading,
    Processing,
    Finalizing,
    Complete,
    Failed,
    Cancelled,
}

impl ExportStage {
    fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Downloading,
            2 => Self::Processing,
            3 => Self::Finalizing,
            4 => Self::Complete,
            5 => Self::Failed,
            6 => Self::Cancelled,
            _ => Self::Preparing,
        }
    }

    fn label(self, language: Language) -> &'static str {
        match (self, language) {
            (Self::Preparing, Language::Portuguese) => "Preparando a exportação",
            (Self::Downloading, Language::Portuguese) => "Baixando o trecho",
            (Self::Processing, Language::Portuguese) => "Processando o vídeo",
            (Self::Finalizing, Language::Portuguese) => "Finalizando o arquivo",
            (Self::Complete, Language::Portuguese) => "Exportação concluída",
            (Self::Failed, Language::Portuguese) => "Falha na exportação",
            (Self::Cancelled, Language::Portuguese) => "Exportação cancelada",
            (Self::Preparing, Language::English) => "Preparing export",
            (Self::Downloading, Language::English) => "Downloading clip",
            (Self::Processing, Language::English) => "Processing video",
            (Self::Finalizing, Language::English) => "Finalizing file",
            (Self::Complete, Language::English) => "Export complete",
            (Self::Failed, Language::English) => "Export failed",
            (Self::Cancelled, Language::English) => "Export cancelled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum PreviewStage {
    Resolving,
    Buffering,
    Playing,
    Finished,
    Failed,
}

impl PreviewStage {
    fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Buffering,
            2 => Self::Playing,
            3 => Self::Finished,
            4 => Self::Failed,
            _ => Self::Resolving,
        }
    }

    fn label(self, language: Language) -> &'static str {
        match (self, language) {
            (Self::Resolving, Language::Portuguese) => "Preparando a pré-visualização. Aguarde…",
            (Self::Buffering, Language::Portuguese) => {
                "Carregando o trecho. A janela do vídeo abre em seguida…"
            }
            (Self::Playing, Language::Portuguese) => "Pré-visualização em reprodução",
            (Self::Finished, Language::Portuguese) => "Pré-visualização encerrada",
            (Self::Failed, Language::Portuguese) => "Não foi possível iniciar a pré-visualização",
            (Self::Resolving, Language::English) => "Preparing the preview. Please wait…",
            (Self::Buffering, Language::English) => {
                "Loading the clip. The video window opens next…"
            }
            (Self::Playing, Language::English) => "Preview playing",
            (Self::Finished, Language::English) => "Preview finished",
            (Self::Failed, Language::English) => "Could not start preview",
        }
    }
}

#[derive(Default)]
struct PreviewProgress {
    stage: AtomicU8,
}

impl PreviewProgress {
    fn set(&self, stage: PreviewStage) {
        self.stage.store(stage as u8, Ordering::Relaxed);
    }

    fn get(&self) -> PreviewStage {
        PreviewStage::from_u8(self.stage.load(Ordering::Relaxed))
    }
}

#[derive(Default)]
struct InstallProgress {
    current: AtomicU8,
    total: AtomicU8,
    label: Mutex<String>,
}

impl InstallProgress {
    fn new(total: u8) -> Self {
        Self {
            current: AtomicU8::new(0),
            total: AtomicU8::new(total.max(1)),
            label: Mutex::new(String::new()),
        }
    }

    fn set_waiting(&self, language: Language) {
        self.current.store(0, Ordering::Relaxed);
        if let Ok(mut label) = self.label.lock() {
            *label = match language {
                Language::Portuguese => "Preparando a instalação com o winget…".into(),
                Language::English => "Preparing the winget install…".into(),
            };
        }
    }

    fn set_package(&self, index: u8, package: &str, language: Language) {
        self.current.store(index, Ordering::Relaxed);
        if let Ok(mut label) = self.label.lock() {
            *label = match language {
                Language::Portuguese => format!("Instalando {package}…"),
                Language::English => format!("Installing {package}…"),
            };
        }
    }

    fn summary(&self, language: Language) -> String {
        let current = self.current.load(Ordering::Relaxed);
        let total = self.total.load(Ordering::Relaxed).max(1);
        let detail = self
            .label
            .lock()
            .ok()
            .map(|label| label.clone())
            .filter(|label| !label.is_empty())
            .unwrap_or_else(|| match language {
                Language::Portuguese => "Instalando…".into(),
                Language::English => "Installing…".into(),
            });
        match language {
            Language::Portuguese => format!("{detail}  ({current}/{total})"),
            Language::English => format!("{detail}  ({current}/{total})"),
        }
    }
}
impl App {
    fn new() -> Self {
        Self {
            history: load_history(),
            language: load_language(),
            preview_volume: 100,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    UrlChanged(String),
    StartChanged(String),
    EndChanged(String),
    DestinationChanged(String),
    Load,
    Loaded(Result<VideoInfo, String>),
    Export,
    Exported(Result<String, String>),
    SelectPage(Page),
    SelectFull,
    TimelineStart(u64),
    TimelineEnd(u64),
    CancelExport,
    OpenExport(String),
    Opened(Result<(), String>),
    FormatChanged(FormatChoice),
    OutputFormatChanged(ExportFormat),
    DestinationChosen(Result<Option<String>, String>),
    ChooseDestination,
    PreviewSelection,
    PreviewResult(Result<PreviewPlayer, String>),
    PreviewVolumeUp,
    PreviewVolumeDown,
    PreviewVolumeApplied(Result<(), String>),
    RequestRemoveExport(String),
    RequestClearExports,
    ConfirmHistoryAction,
    DismissHistoryAction,
    ThumbnailLoaded(u64, Result<Vec<u8>, String>),
    LanguageChanged(Language),
    ProgressTick,
    RecheckTools,
    OpenToolPage(ToolPage),
    CopyInstallCommands,
    InstallMissingTools,
    ToolsInstalled(Result<String, String>),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreviewPlayer {
    Vlc,
    Mpv,
    Ffplay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ExportFormat {
    #[default]
    Mp4,
    Mp3,
    M4a,
}
impl ExportFormat {
    fn extension(self) -> &'static str {
        match self {
            Self::Mp4 => "mp4",
            Self::Mp3 => "mp3",
            Self::M4a => "m4a",
        }
    }
}
impl std::fmt::Display for ExportFormat {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Mp4 => "MP4 video",
            Self::Mp3 => "MP3 audio",
            Self::M4a => "M4A audio",
        })
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
enum HistoryPrompt {
    Remove(String),
    Clear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Page {
    #[default]
    Create,
    Downloads,
}

impl App {
    fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::UrlChanged(value) => self.url = value,
            Message::StartChanged(value) => self.start = value,
            Message::EndChanged(value) => self.end = value,
            Message::TimelineStart(seconds) => {
                let mut seconds = seconds;
                if let Some(end) = parse_time(&self.end) {
                    seconds = seconds.min(end.saturating_sub(1));
                }
                if let Some(duration) = self.video.as_ref().map(|video| video.duration) {
                    seconds = seconds.min(duration.saturating_sub(1));
                }
                self.start = format_time(seconds);
            }
            Message::TimelineEnd(seconds) => {
                let start = parse_time(&self.start).unwrap_or(0);
                let duration = self
                    .video
                    .as_ref()
                    .map(|video| video.duration)
                    .unwrap_or(u64::MAX);
                self.end = format_time(seconds.max(start.saturating_add(1)).min(duration));
            }
            Message::SelectFull => {
                if let Some(video) = &self.video {
                    self.start = "00:00:00".into();
                    self.end = format_time(video.duration);
                }
            }
            Message::FormatChanged(value) => self.selected_format = Some(value),
            Message::OutputFormatChanged(format) => {
                self.output_format = format;
                if self.destination.ends_with(".mp4")
                    || self.destination.ends_with(".mp3")
                    || self.destination.ends_with(".m4a")
                {
                    self.destination = PathBuf::from(&self.destination)
                        .with_extension(format.extension())
                        .to_string_lossy()
                        .into_owned();
                }
            }
            Message::LanguageChanged(language) => {
                self.language = language;
                save_language(language);
            }
            Message::DestinationChanged(value) => self.destination = value,
            Message::ChooseDestination => {
                log::info!("UI event received: choose destination");
                self.notice = Some(
                    if self.language == Language::Portuguese {
                        "Abrindo o seletor de destino…"
                    } else {
                        "Opening destination chooser…"
                    }
                    .into(),
                );
                let extension = self.output_format.extension().to_owned();
                let filename = if self.destination.is_empty() {
                    format!("clip.{extension}")
                } else {
                    PathBuf::from(&self.destination)
                        .with_extension(extension)
                        .to_string_lossy()
                        .into_owned()
                };
                return iced::Task::perform(
                    choose_destination(filename),
                    Message::DestinationChosen,
                );
            }
            Message::DestinationChosen(result) => match result {
                Ok(Some(path)) => {
                    self.destination = path;
                    self.notice = Some(
                        if self.language == Language::Portuguese {
                            "Destino selecionado."
                        } else {
                            "Destination selected."
                        }
                        .into(),
                    );
                }
                Ok(None) => self.notice = None,
                Err(error) => self.notice = Some(error),
            },
            Message::ThumbnailLoaded(generation, result) => {
                if generation == self.thumbnail_generation {
                    self.thumbnail_pending = false;
                    match result {
                        Ok(bytes) => self.thumbnail = Some(Handle::from_bytes(bytes)),
                        Err(error) => log::debug!("Thumbnail unavailable: {error}"),
                    }
                }
            }
            Message::Load => {
                self.toolchain = Toolchain::probe();
                if self.toolchain.ytdlp.is_none() {
                    self.notice = export_blocker(&self.toolchain, self.language);
                } else {
                    match validate_video_url(&self.url) {
                        Ok(url) => {
                            self.loading = true;
                            self.notice = Some(
                                if self.language == Language::Portuguese {
                                    "Carregando metadados do vídeo…"
                                } else {
                                    "Loading video metadata…"
                                }
                                .into(),
                            );
                            return iced::Task::perform(fetch_metadata(url), Message::Loaded);
                        }
                        Err(error) => {
                            self.notice = Some(localized_error(&error.to_string(), self.language))
                        }
                    }
                }
            }
            Message::Loaded(result) => {
                self.loading = false;
                match result {
                    Ok(video) => {
                        self.progress = None;
                        self.preview_progress = None;
                        self.start = "00:00:00".into();
                        self.selected_format = None;
                        self.end = format_time(video.duration);
                        if self.destination.is_empty() {
                            self.destination = default_destination(&video.title)
                                .to_string_lossy()
                                .into_owned();
                        }
                        self.notice = Some(
                            if self.language == Language::Portuguese {
                                "Metadados carregados. Selecione um trecho e exporte."
                            } else {
                                "Metadata loaded. Select a clip range and export."
                            }
                            .into(),
                        );
                        self.thumbnail = None;
                        self.thumbnail_generation = self.thumbnail_generation.wrapping_add(1);
                        let generation = self.thumbnail_generation;
                        let thumbnail_url = video.thumbnail.clone();
                        self.thumbnail_pending = thumbnail_url.is_some();
                        self.video = Some(video);
                        if let Some(url) = thumbnail_url {
                            return iced::Task::perform(fetch_thumbnail(url), move |result| {
                                Message::ThumbnailLoaded(generation, result)
                            });
                        }
                    }
                    Err(error) => {
                        self.toolchain = Toolchain::probe();
                        self.notice = Some(metadata_notice(&error, self.language));
                    }
                }
            }
            Message::Export => {
                log::info!(
                    "UI event received: export (video loaded: {})",
                    self.video.is_some()
                );
                self.toolchain = Toolchain::probe();
                if let Some(notice) = export_blocker(&self.toolchain, self.language) {
                    self.notice = Some(notice);
                } else if let Some(video) = self.video.clone() {
                    let parsed = parse_range(&self.start, &self.end, video.duration);
                    match parsed {
                        Ok((start, end)) => {
                            let dest = PathBuf::from(self.destination.trim());
                            if dest.as_os_str().is_empty() {
                                self.notice = Some(
                                    if self.language == Language::Portuguese {
                                        "Escolha o caminho do arquivo de saída primeiro."
                                    } else {
                                        "Choose an output file path first."
                                    }
                                    .into(),
                                );
                            } else {
                                self.exporting = true;
                                self.notice = Some(
                                    if self.language == Language::Portuguese {
                                        "Preparando a extração do trecho…"
                                    } else {
                                        "Preparing clip extraction…"
                                    }
                                    .into(),
                                );
                                let (cancel_tx, cancel_rx) = watch::channel(false);
                                self.cancel_export = Some(cancel_tx);
                                let progress = Arc::new(DownloadProgress::default());
                                self.progress = Some(progress.clone());
                                return iced::Task::perform(
                                    export(
                                        video.url,
                                        start,
                                        end,
                                        dest,
                                        self.selected_format.clone().map(|choice| choice.id),
                                        self.output_format,
                                        progress,
                                        cancel_rx,
                                    ),
                                    Message::Exported,
                                );
                            }
                        }
                        Err(error) => {
                            self.notice = Some(localized_error(&error.to_string(), self.language))
                        }
                    }
                }
            }
            Message::Exported(result) => {
                self.exporting = false;
                self.cancel_export = None;
                match result {
                    Ok(path) => {
                        self.retry_available = false;
                        if let Some(progress) = &self.progress {
                            progress
                                .stage
                                .store(ExportStage::Complete as u8, Ordering::Relaxed);
                            progress.percent.store(100, Ordering::Relaxed);
                        }
                        self.notice = Some(format!(
                            "{}: {path}",
                            if self.language == Language::Portuguese {
                                "Exportação concluída"
                            } else {
                                "Export complete"
                            }
                        ));
                        self.history.retain(|entry| entry != &path);
                        self.history.insert(0, path);
                        self.history.truncate(20);
                        save_history(&self.history);
                    }
                    Err(error) => {
                        let cancelled = error == "Export cancelled.";
                        if let Some(progress) = &self.progress {
                            progress.stage.store(
                                if cancelled {
                                    ExportStage::Cancelled
                                } else {
                                    ExportStage::Failed
                                } as u8,
                                Ordering::Relaxed,
                            );
                        }
                        self.retry_available = !cancelled;
                        self.notice = Some(localized_error(&error, self.language));
                    }
                }
            }
            Message::CancelExport => {
                if let Some(cancel) = self.cancel_export.take() {
                    let _ = cancel.send(true);
                    self.notice = Some(localized_error("Cancelling export…", self.language));
                }
            }
            Message::PreviewSelection => {
                log::info!(
                    "UI event received: preview (video loaded: {})",
                    self.video.is_some()
                );
                if let Some(video) = self.video.clone() {
                    match parse_range(&self.start, &self.end, video.duration) {
                        Ok((start, end)) => {
                            self.previewing = true;
                            self.preview_player = None;
                            self.notice = Some(
                                if self.language == Language::Portuguese {
                                    "Aguarde. Baixando só o trecho selecionado para a pré-visualização…"
                                } else {
                                    "Please wait. Downloading only the selected range for preview…"
                                }
                                .into(),
                            );
                            let progress = Arc::new(PreviewProgress::default());
                            progress.set(PreviewStage::Resolving);
                            self.preview_progress = Some(progress.clone());
                            return iced::Task::perform(
                                launch_preview(video.url, start, end, progress),
                                Message::PreviewResult,
                            );
                        }
                        Err(error) => {
                            self.notice = Some(localized_error(&error.to_string(), self.language))
                        }
                    }
                }
            }
            Message::PreviewResult(result) => {
                self.previewing = false;
                match result {
                    Ok(player) => {
                        self.preview_active = true;
                        self.preview_player = Some(player);
                        self.notice = Some(preview_opened_notice(player, self.language));
                        if player == PreviewPlayer::Ffplay && in_app_preview_volume_supported() {
                            let volume = self.preview_volume;
                            return iced::Task::perform(
                                set_preview_volume(volume),
                                Message::PreviewVolumeApplied,
                            );
                        }
                    }
                    Err(error) => {
                        self.preview_active = false;
                        if let Some(progress) = &self.preview_progress {
                            progress.set(PreviewStage::Failed);
                        }
                        self.notice = Some(localized_error(&error, self.language));
                    }
                }
            }
            Message::PreviewVolumeUp => {
                self.preview_volume = self.preview_volume.saturating_add(10).min(100);
                if self.preview_active {
                    let volume = self.preview_volume;
                    return iced::Task::perform(
                        set_preview_volume(volume),
                        Message::PreviewVolumeApplied,
                    );
                }
            }
            Message::PreviewVolumeDown => {
                self.preview_volume = self.preview_volume.saturating_sub(10);
                if self.preview_active {
                    let volume = self.preview_volume;
                    return iced::Task::perform(
                        set_preview_volume(volume),
                        Message::PreviewVolumeApplied,
                    );
                }
            }
            Message::PreviewVolumeApplied(Ok(())) => {}
            Message::PreviewVolumeApplied(Err(error)) => {
                self.notice = Some(localized_error(&error, self.language));
            }
            Message::RequestRemoveExport(path) => {
                self.history_prompt = Some(HistoryPrompt::Remove(path));
                self.page = Page::Downloads;
            }
            Message::RequestClearExports => {
                if !self.history.is_empty() {
                    self.history_prompt = Some(HistoryPrompt::Clear);
                    self.page = Page::Downloads;
                }
            }
            Message::DismissHistoryAction => self.history_prompt = None,
            Message::ConfirmHistoryAction => match self.history_prompt.take() {
                Some(HistoryPrompt::Remove(path)) => {
                    drop_export(&mut self.history, &path);
                    save_history(&self.history);
                    self.notice = Some(
                        if self.language == Language::Portuguese {
                            "Exportação removida da lista. O arquivo permanece no disco."
                        } else {
                            "Export removed from the list. The file stays on disk."
                        }
                        .into(),
                    );
                }
                Some(HistoryPrompt::Clear) => {
                    self.history.clear();
                    save_history(&self.history);
                    self.notice = Some(
                        if self.language == Language::Portuguese {
                            "Lista de exportações limpa. Os arquivos permanecem no disco."
                        } else {
                            "Export list cleared. The files stay on disk."
                        }
                        .into(),
                    );
                }
                None => {}
            },
            Message::OpenExport(path) => {
                return iced::Task::perform(open_export(path), Message::Opened);
            }
            Message::Opened(result) => {
                if let Err(error) = result {
                    self.notice = Some(error);
                }
            }
            Message::ProgressTick => {
                let finished = self.preview_progress.as_ref().is_some_and(|progress| {
                    matches!(
                        progress.get(),
                        PreviewStage::Finished | PreviewStage::Failed
                    )
                });
                if self.preview_active && finished {
                    self.preview_active = false;
                    self.preview_player = None;
                }
                if self.installing_tools
                    && let Some(progress) = &self.install_progress
                {
                    self.notice = Some(progress.summary(self.language));
                }
            }
            Message::SelectPage(page) => self.page = page,
            Message::RecheckTools => {
                refresh_process_path();
                self.toolchain = Toolchain::probe();
                self.notice = Some(toolchain_summary(&self.toolchain, self.language));
            }
            Message::OpenToolPage(page) => {
                return iced::Task::perform(open_web_page(page.url()), Message::Opened);
            }
            Message::CopyInstallCommands => {
                let commands = install_commands_text(self.language);
                self.notice = Some(
                    self.t(
                        "Install commands copied.",
                        "Comandos de instalação copiados.",
                    )
                    .into(),
                );
                return iced::clipboard::write(commands);
            }
            Message::InstallMissingTools => {
                if self.installing_tools {
                    return iced::Task::none();
                }
                let packages = self.toolchain.packages_to_install();
                let progress = Arc::new(InstallProgress::new(packages.len() as u8));
                progress.set_waiting(self.language);
                self.install_progress = Some(progress.clone());
                self.installing_tools = true;
                self.notice = Some(progress.summary(self.language));
                let toolchain = self.toolchain.clone();
                let language = self.language;
                return iced::Task::perform(
                    install_missing_tools(toolchain, language, progress),
                    Message::ToolsInstalled,
                );
            }
            Message::ToolsInstalled(result) => {
                self.installing_tools = false;
                self.install_progress = None;
                refresh_process_path();
                self.toolchain = Toolchain::probe();
                self.notice = Some(match result {
                    Ok(message) | Err(message) => message,
                });
            }
        }
        iced::Task::none()
    }

    fn t<'a>(&self, english: &'a str, portuguese: &'a str) -> &'a str {
        if self.language == Language::Portuguese {
            portuguese
        } else {
            english
        }
    }

    fn view(&self) -> Element<'_, Message> {
        row![
            self.sidebar(),
            rule::vertical(1).style(|_theme| separator()),
            container(self.workspace())
                .width(Fill)
                .height(Fill)
                .style(|_theme| app_background()),
        ]
        .width(Fill)
        .height(Fill)
        .into()
    }

    fn sidebar(&self) -> Element<'_, Message> {
        let downloads = if self.history.is_empty() {
            self.t("Downloads", "Exportações").to_owned()
        } else {
            format!(
                "{}    {}",
                self.t("Downloads", "Exportações"),
                self.history.len()
            )
        };

        container(
            column![
                row![
                    brand_mark(),
                    column![
                        text("ClipForge").size(18).color(TEXT),
                        text(self.t("Local clips", "Clipes locais"))
                            .size(12)
                            .color(MUTED),
                    ]
                    .spacing(2),
                ]
                .spacing(12)
                .align_y(Alignment::Center),
                column![
                    text(self.t("Language", "Idioma")).size(12).color(FAINT),
                    self.language_switch(),
                ]
                .spacing(8),
                column![
                    text(self.t("Workspace", "Área de trabalho"))
                        .size(12)
                        .color(FAINT),
                    nav_button(
                        self.t("Create clip", "Criar clipe"),
                        self.page == Page::Create,
                        Message::SelectPage(Page::Create),
                    ),
                    nav_button(
                        downloads,
                        self.page == Page::Downloads,
                        Message::SelectPage(Page::Downloads),
                    ),
                ]
                .spacing(8),
                iced::widget::Space::new().height(Fill),
                text(self.t(
                    "Only download content you are authorized to save.",
                    "Baixe apenas conteúdo que você tem autorização para salvar.",
                ))
                .size(12)
                .color(FAINT),
            ]
            .spacing(28)
            .padding(22),
        )
        .width(252)
        .height(Fill)
        .style(|_theme| sidebar_style())
        .into()
    }

    fn workspace(&self) -> Element<'_, Message> {
        let (status, status_color) = if self.loading {
            (self.t("Loading", "Carregando"), ACCENT)
        } else if self.installing_tools {
            (self.t("Installing", "Instalando"), ACCENT)
        } else if self.exporting {
            (self.t("Exporting", "Exportando"), ACCENT)
        } else if self.previewing || self.preview_active {
            (self.t("Preview", "Pré-visualização"), ACCENT)
        } else if !self.toolchain.ready() {
            (self.t("Setup", "Instalação"), DANGER)
        } else {
            (self.t("Ready", "Pronto"), SUCCESS)
        };
        let title = match self.page {
            Page::Create => self.t("Create", "Criar"),
            Page::Downloads => self.t("Downloads", "Exportações"),
        };
        let mut workspace = column![
            row![
                text(self.t("Workspace", "Área de trabalho"))
                    .size(13)
                    .color(FAINT),
                text("/").size(13).color(FAINT),
                text(title).size(13).color(TEXT),
                iced::widget::Space::new().width(Fill),
                status_dot(status_color),
                text(status).size(13).color(status_color),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
        ]
        .spacing(16)
        .padding(Padding {
            top: 26.0,
            right: 32.0,
            bottom: 20.0,
            left: 32.0,
        })
        .width(Fill)
        .height(Fill);
        if let Some(notice) = self.notice_banner() {
            workspace = workspace.push(notice);
        }
        if self.preview_active
            && self.preview_player == Some(PreviewPlayer::Ffplay)
            && in_app_preview_volume_supported()
        {
            workspace = workspace.push(self.volume_controls());
        }
        let body = match self.page {
            Page::Create => self.create_page(),
            Page::Downloads => self.downloads_page(),
        };
        workspace
            .push(scrollable(body).height(Fill).width(Fill))
            .into()
    }

    fn volume_controls(&self) -> Element<'_, Message> {
        container(
            row![
                text(self.t("Preview volume", "Volume da pré-visualização"))
                    .size(14)
                    .color(TEXT),
                iced::widget::Space::new().width(Fill),
                button(text("−").size(18))
                    .on_press_maybe(
                        (self.preview_volume > 0).then_some(Message::PreviewVolumeDown),
                    )
                    .padding([6, 14])
                    .style(secondary_button),
                text(format!("{}%", self.preview_volume))
                    .size(15)
                    .color(ACCENT),
                button(text("+").size(18))
                    .on_press_maybe(
                        (self.preview_volume < 100).then_some(Message::PreviewVolumeUp),
                    )
                    .padding([6, 14])
                    .style(secondary_button),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        )
        .width(Fill)
        .padding([10, 14])
        .style(|_theme| inset_style())
        .into()
    }

    fn notice_banner(&self) -> Option<Element<'_, Message>> {
        let notice = self.notice.as_deref().filter(|notice| !notice.is_empty())?;
        Some(
            container(text(notice).size(13).color(TEXT))
                .width(Fill)
                .padding([12, 16])
                .style(|_theme| notice_style())
                .into(),
        )
    }

    fn create_page(&self) -> Element<'_, Message> {
        let mut page = column![].spacing(16).width(Fill);
        if !self.toolchain.ready() {
            page = page.push(self.setup_card());
        } else {
            if self.toolchain.deno.is_none() {
                page = page.push(self.deno_note());
            }
            if !self.toolchain.has_preview_player() {
                page = page.push(self.preview_player_note());
            }
        }
        if self.video.is_none() {
            page = page.push(self.hero());
        }
        page = page.push(self.step_row());
        page = page.push(self.url_card());
        if let Some(video) = &self.video {
            page = page.push(self.selection_card(video));
        }
        page.padding(Padding {
            bottom: 28.0,
            ..Padding::default()
        })
        .into()
    }

    fn hero(&self) -> Element<'_, Message> {
        column![
            text(self.t(
                "Turn any moment into a clip.",
                "Transforme qualquer momento em um clipe.",
            ))
            .size(32)
            .color(TEXT),
            text(self.t(
                "Paste a video link, choose the range, and export only that section.",
                "Cole o link, escolha o trecho e exporte só essa parte.",
            ))
            .size(15)
            .color(MUTED),
        ]
        .spacing(8)
        .into()
    }

    fn setup_card(&self) -> Element<'_, Message> {
        let ytdlp = dependency_line(
            "yt-dlp",
            self.t(
                "Reads the video and downloads the selected section.",
                "Lê o vídeo e baixa só o trecho.",
            ),
            self.presence_label(self.toolchain.ytdlp.is_some()),
            if self.toolchain.ytdlp.is_some() {
                SUCCESS
            } else {
                DANGER
            },
        );
        let ffmpeg = dependency_line(
            "FFmpeg",
            self.t(
                "Cuts the section and writes the file.",
                "Corta o trecho e grava o arquivo.",
            ),
            self.presence_label(self.toolchain.ffmpeg.is_some()),
            if self.toolchain.ffmpeg.is_some() {
                SUCCESS
            } else {
                DANGER
            },
        );
        let deno_present = self.toolchain.deno.is_some();
        let deno = dependency_line(
            "Deno",
            self.t(
                "Recommended for current YouTube extraction.",
                "Recomendado para a extração atual do YouTube.",
            ),
            self.presence_label(deno_present),
            if deno_present { SUCCESS } else { ACCENT },
        );
        let preview_present = self.toolchain.has_preview_player();
        let preview = dependency_line(
            "VLC",
            self.t(
                "Recommended preview player with pause, seek, and volume.",
                "Reprodutor recomendado para prévia, com pause, seek e volume.",
            ),
            if preview_present {
                self.t("found", "encontrado")
            } else {
                self.t("missing", "ausente")
            },
            if preview_present { SUCCESS } else { ACCENT },
        );
        let mut actions = row![].spacing(10).align_y(Alignment::Center);
        if can_auto_install() && self.toolchain.has_installable_gap() {
            actions = actions.push(
                button(
                    text(if self.installing_tools {
                        self.t("Installing…", "Instalando…")
                    } else {
                        self.t("Install missing tools", "Instalar o que falta")
                    })
                    .size(14),
                )
                .on_press_maybe((!self.installing_tools).then_some(Message::InstallMissingTools))
                .padding([11, 16])
                .style(if self.installing_tools {
                    busy_button
                } else {
                    primary_button
                }),
            );
        }
        actions = actions.push(
            button(text(self.t("Copy commands", "Copiar comandos")).size(14))
                .on_press(Message::CopyInstallCommands)
                .padding([11, 16])
                .style(secondary_button),
        );
        actions = actions.push(
            button(text(self.t("Check again", "Verificar de novo")).size(14))
                .on_press_maybe((!self.installing_tools).then_some(Message::RecheckTools))
                .padding([11, 16])
                .style(secondary_button),
        );
        card(
            column![
                text(self.t("Install the media tools", "Instale as ferramentas de mídia"))
                    .size(16)
                    .color(TEXT),
                text(self.t(
                    "ClipForge does not ship yt-dlp or FFmpeg. Accept the install below to use the official builds, or copy the commands and run them yourself.",
                    "O ClipForge não inclui o yt-dlp nem o FFmpeg. Aceite a instalação abaixo para usar as versões oficiais, ou copie os comandos e rode você mesmo.",
                ))
                .size(13)
                .color(MUTED),
                ytdlp,
                ffmpeg,
                deno,
                preview,
                self.install_progress_panel(),
                container(text(install_instructions(self.language)).size(13).color(TEXT))
                    .width(Fill)
                    .padding(14)
                    .style(|_theme| inset_style()),
                actions,
                row![
                    button(text(self.t("yt-dlp site", "Site do yt-dlp")).size(13))
                        .on_press(Message::OpenToolPage(ToolPage::YtDlp))
                        .padding([8, 12])
                        .style(quiet_button),
                    button(text(self.t("FFmpeg site", "Site do FFmpeg")).size(13))
                        .on_press(Message::OpenToolPage(ToolPage::Ffmpeg))
                        .padding([8, 12])
                        .style(quiet_button),
                    button(text(self.t("Deno site", "Site do Deno")).size(13))
                        .on_press(Message::OpenToolPage(ToolPage::Deno))
                        .padding([8, 12])
                        .style(quiet_button),
                    button(text(self.t("VLC site", "Site do VLC")).size(13))
                        .on_press(Message::OpenToolPage(ToolPage::Vlc))
                        .padding([8, 12])
                        .style(quiet_button),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            ]
            .spacing(14),
        )
    }

    fn preview_player_note(&self) -> Element<'_, Message> {
        let mut actions = row![].spacing(10).align_y(Alignment::Center);
        if can_auto_install() {
            actions = actions.push(
                button(
                    text(if self.installing_tools {
                        self.t("Installing…", "Instalando…")
                    } else {
                        self.t("Install VLC", "Instalar VLC")
                    })
                    .size(13),
                )
                .on_press_maybe((!self.installing_tools).then_some(Message::InstallMissingTools))
                .padding([8, 12])
                .style(if self.installing_tools {
                    busy_button
                } else {
                    primary_button
                }),
            );
        }
        actions = actions.push(
            button(text(self.t("Copy commands", "Copiar comandos")).size(13))
                .on_press(Message::CopyInstallCommands)
                .padding([8, 12])
                .style(secondary_button),
        );
        actions = actions.push(
            button(text(self.t("Check again", "Verificar de novo")).size(13))
                .on_press_maybe((!self.installing_tools).then_some(Message::RecheckTools))
                .padding([8, 12])
                .style(secondary_button),
        );
        actions = actions.push(
            button(text(self.t("VLC site", "Site do VLC")).size(13))
                .on_press(Message::OpenToolPage(ToolPage::Vlc))
                .padding([8, 12])
                .style(secondary_button),
        );
        container(
            column![
                text(self.t(
                    "Preview is using FFplay, which has few controls. Install VLC for pause, seek, and volume like on Linux.",
                    "A prévia está no FFplay, com poucos controles. Instale o VLC para pause, seek e volume como no Linux.",
                ))
                .size(13)
                .color(TEXT),
                text(self.t(
                    "winget install --id VideoLAN.VLC -e --accept-package-agreements --accept-source-agreements",
                    "winget install --id VideoLAN.VLC -e --accept-package-agreements --accept-source-agreements",
                ))
                .size(12)
                .color(MUTED),
                self.install_progress_panel(),
                actions,
            ]
            .spacing(10),
        )
        .width(Fill)
        .padding([12, 16])
        .style(|_theme| inset_style())
        .into()
    }

    fn deno_note(&self) -> Element<'_, Message> {
        let mut actions = row![].spacing(10).align_y(Alignment::Center);
        if can_auto_install() {
            actions = actions.push(
                button(
                    text(if self.installing_tools {
                        self.t("Installing…", "Instalando…")
                    } else {
                        self.t("Install Deno", "Instalar Deno")
                    })
                    .size(13),
                )
                .on_press_maybe((!self.installing_tools).then_some(Message::InstallMissingTools))
                .padding([8, 12])
                .style(if self.installing_tools {
                    busy_button
                } else {
                    primary_button
                }),
            );
        }
        actions = actions.push(
            button(text(self.t("Copy command", "Copiar comando")).size(13))
                .on_press(Message::CopyInstallCommands)
                .padding([8, 12])
                .style(secondary_button),
        );
        actions = actions.push(
            button(text(self.t("Check again", "Verificar de novo")).size(13))
                .on_press_maybe((!self.installing_tools).then_some(Message::RecheckTools))
                .padding([8, 12])
                .style(secondary_button),
        );
        actions = actions.push(
            button(text(self.t("Deno site", "Site do Deno")).size(13))
                .on_press(Message::OpenToolPage(ToolPage::Deno))
                .padding([8, 12])
                .style(secondary_button),
        );
        container(
            column![
                text(self.t(
                    "Deno was not found. YouTube may fail or expose fewer formats.",
                    "O Deno não foi encontrado. O YouTube pode falhar ou mostrar menos formatos.",
                ))
                .size(13)
                .color(TEXT),
                text(deno_install_hint(self.language)).size(12).color(MUTED),
                self.install_progress_panel(),
                actions,
            ]
            .spacing(10),
        )
        .width(Fill)
        .padding([12, 16])
        .style(|_theme| inset_style())
        .into()
    }

    fn install_progress_panel(&self) -> Element<'_, Message> {
        let Some(progress) = self
            .install_progress
            .as_ref()
            .filter(|_| self.installing_tools)
        else {
            return iced::widget::Space::new().height(0).into();
        };
        let current = progress.current.load(Ordering::Relaxed);
        let total = progress.total.load(Ordering::Relaxed).max(1);
        let fraction = (current as f32 / total as f32).clamp(0.0, 1.0) * 100.0;
        container(
            column![
                text(progress.summary(self.language)).size(14).color(TEXT),
                text(self.t(
                    "Winget downloads can take a minute. Keep ClipForge open.",
                    "O download do winget pode levar um minuto. Mantenha o ClipForge aberto.",
                ))
                .size(12)
                .color(MUTED),
                progress_bar(0.0..=100.0, fraction)
                    .girth(6)
                    .style(export_progress_style),
            ]
            .spacing(8),
        )
        .width(Fill)
        .padding(14)
        .style(|_theme| inset_style())
        .into()
    }

    fn presence_label(&self, present: bool) -> &'static str {
        if present {
            self.t("found", "encontrado")
        } else {
            self.t("missing", "ausente")
        }
    }

    fn step_row(&self) -> Element<'_, Message> {
        let step = if self.video.is_none() {
            1
        } else if self.exporting {
            3
        } else {
            2
        };
        row![
            step_label(1, step, self.t("Video", "Vídeo")),
            text("·").size(13).color(FAINT),
            step_label(2, step, self.t("Selection", "Seleção")),
            text("·").size(13).color(FAINT),
            step_label(3, step, self.t("Export", "Exportar")),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .into()
    }

    fn url_card(&self) -> Element<'_, Message> {
        let title = if self.video.is_none() {
            self.t("Start with a video", "Comece com um vídeo")
        } else {
            self.t("Load another video", "Carregar outro vídeo")
        };
        let action = if self.loading {
            self.t("Loading…", "Carregando…")
        } else {
            self.t("Load video", "Carregar vídeo")
        };
        card(
            column![
                text(title).size(16).color(TEXT),
                row![
                    text_input(
                        self.t(
                            "Paste a YouTube video URL",
                            "Cole um link de vídeo do YouTube",
                        ),
                        &self.url,
                    )
                    .on_input(Message::UrlChanged)
                    .padding(12)
                    .size(15)
                    .width(Fill)
                    .style(field_style),
                    button(text(action).size(14))
                        .on_press_maybe((!self.loading).then_some(Message::Load))
                        .padding([12, 18])
                        .style(primary_button),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            ]
            .spacing(14),
        )
    }

    fn selection_card(&self, video: &VideoInfo) -> Element<'_, Message> {
        let duration = video.duration.max(1) as f32;
        let start_seconds = parse_time(&self.start).unwrap_or(0) as f32;
        let end_seconds = parse_time(&self.end).unwrap_or(video.duration) as f32;
        let formats = video.formats.clone();
        let qualities = if formats.is_empty() {
            self.t("Best available quality", "Melhor qualidade disponível")
                .to_owned()
        } else {
            formats
                .iter()
                .map(|format| format.label.as_str())
                .collect::<Vec<_>>()
                .join("   ·   ")
        };
        let meta = Self::video_meta(video);
        let range_summary: Element<'_, Message> =
            match parse_range(&self.start, &self.end, video.duration) {
                Ok((start, end)) => text(format!(
                    "{}    —    {}          {} {}",
                    format_time(start),
                    format_time(end),
                    end - start,
                    self.t("seconds", "segundos"),
                ))
                .size(13)
                .color(MUTED)
                .into(),
                Err(_) => text(self.t(
                    "Use HH:MM:SS, with the start before the end and inside the video.",
                    "Use HH:MM:SS, com o início antes do fim e dentro da duração.",
                ))
                .size(13)
                .color(DANGER)
                .into(),
            };
        let mut actions = row![
            button(
                text(if self.previewing {
                    self.t("Please wait…", "Aguarde…")
                } else {
                    self.t("Preview", "Pré-visualizar")
                })
                .size(14),
            )
            .on_press_maybe((!self.previewing).then_some(Message::PreviewSelection))
            .padding([11, 16])
            .style(if self.previewing {
                busy_button
            } else {
                secondary_button
            }),
            button(
                text(if self.exporting {
                    self.t("Exporting…", "Exportando…")
                } else {
                    self.t("Export clip", "Exportar clipe")
                })
                .size(14),
            )
            .on_press_maybe((!self.exporting).then_some(Message::Export))
            .padding([11, 18])
            .style(primary_button),
        ]
        .spacing(10)
        .align_y(Alignment::Center);
        if self.exporting {
            actions = actions.push(
                button(text(self.t("Cancel", "Cancelar")).size(14))
                    .on_press(Message::CancelExport)
                    .padding([11, 16])
                    .style(danger_button),
            );
        } else if self.retry_available {
            actions = actions.push(
                button(text(self.t("Try again", "Tentar de novo")).size(14))
                    .on_press(Message::Export)
                    .padding([11, 16])
                    .style(secondary_button),
            );
        }

        card(
            column![
                row![self.thumbnail_view(), meta]
                    .spacing(18)
                    .align_y(Alignment::Center),
                hairline(),
                text(self.t("Clip range", "Intervalo do clipe"))
                    .size(16)
                    .color(TEXT),
                text(self.t(
                    "Drag the handles, move the sliders, or type HH:MM:SS.",
                    "Arraste as alças, use os controles ou digite HH:MM:SS.",
                ))
                .size(13)
                .color(MUTED),
                canvas(RangeTrack {
                    start: start_seconds,
                    end: end_seconds,
                    duration,
                })
                .width(Fill)
                .height(52),
                range_summary,
                row![
                    column![
                        field_label(self.t("Start", "Início")),
                        text_input(self.t("Start", "Início"), &self.start)
                            .on_input(Message::StartChanged)
                            .padding(12)
                            .style(field_style),
                        slider(0.0..=duration, start_seconds.min(duration), |value| {
                            Message::TimelineStart(value as u64)
                        })
                        .step(1.0_f32)
                        .style(range_slider),
                    ]
                    .spacing(8)
                    .width(Fill),
                    column![
                        field_label(self.t("End", "Fim")),
                        text_input(self.t("End", "Fim"), &self.end)
                            .on_input(Message::EndChanged)
                            .padding(12)
                            .style(field_style),
                        slider(1.0..=duration, end_seconds.clamp(1.0, duration), |value| {
                            Message::TimelineEnd(value as u64)
                        },)
                        .step(1.0_f32)
                        .style(range_slider),
                    ]
                    .spacing(8)
                    .width(Fill),
                ]
                .spacing(16),
                button(text(self.t("Use full video", "Usar vídeo inteiro")).size(14))
                    .on_press(Message::SelectFull)
                    .padding([8, 12])
                    .style(quiet_button),
                hairline(),
                row![
                    column![
                        field_label(self.t("Quality", "Qualidade")),
                        pick_list(
                            formats,
                            self.selected_format.clone(),
                            Message::FormatChanged,
                        )
                        .placeholder(self.t(
                            "Automatic · best available",
                            "Automática · melhor qualidade",
                        ))
                        .width(Fill)
                        .style(menu_field),
                        text(qualities).size(13).color(MUTED),
                    ]
                    .spacing(8)
                    .width(Fill),
                    column![
                        field_label(self.t("Output", "Saída")),
                        row![
                            self.format_chip(ExportFormat::Mp4),
                            self.format_chip(ExportFormat::Mp3),
                            self.format_chip(ExportFormat::M4a),
                        ]
                        .spacing(8),
                    ]
                    .spacing(8)
                    .width(Fill),
                ]
                .spacing(18),
                hairline(),
                field_label(self.t("Output file", "Arquivo de saída")),
                row![
                    text_input(
                        self.t("/path/to/clip.mp4", "/caminho/para/clipe.mp4"),
                        &self.destination,
                    )
                    .on_input(Message::DestinationChanged)
                    .padding(12)
                    .width(Fill)
                    .style(field_style),
                    button(text(self.t("Choose…", "Escolher…")).size(14))
                        .on_press(Message::ChooseDestination)
                        .padding([12, 16])
                        .style(secondary_button),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
                self.export_status(),
                self.preview_actions(actions.into()),
            ]
            .spacing(14),
        )
    }

    fn video_meta(video: &VideoInfo) -> Element<'static, Message> {
        column![
            text(video.title.clone()).size(22).color(TEXT),
            text(format!(
                "{}     ·     {}",
                video.channel,
                format_time(video.duration)
            ))
            .size(14)
            .color(MUTED),
        ]
        .spacing(6)
        .width(Fill)
        .into()
    }

    fn thumbnail_view(&self) -> Element<'_, Message> {
        if let Some(handle) = &self.thumbnail {
            iced::widget::image(handle.clone())
                .width(228)
                .height(128)
                .content_fit(ContentFit::Cover)
                .border_radius(12.0)
                .into()
        } else {
            container(
                text(if self.thumbnail_pending {
                    self.t("Loading thumbnail…", "Carregando miniatura…")
                } else {
                    self.t("No thumbnail", "Sem miniatura")
                })
                .size(12)
                .color(MUTED),
            )
            .width(228)
            .height(128)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(|_theme| inset_style())
            .into()
        }
    }

    fn export_status(&self) -> Element<'_, Message> {
        let Some(progress) = &self.progress else {
            return text(self.t("Ready to export", "Pronto para exportar"))
                .size(13)
                .color(MUTED)
                .into();
        };
        let stage = ExportStage::from_u8(progress.stage.load(Ordering::Relaxed));
        let percent = progress.percent.load(Ordering::Relaxed);
        let details = progress.details.lock().ok();
        let speed = details
            .as_ref()
            .and_then(|details| details.speed.clone())
            .unwrap_or_else(|| self.t("calculating…", "calculando…").to_owned());
        let eta = details
            .as_ref()
            .and_then(|details| details.eta.clone())
            .unwrap_or_else(|| self.t("calculating…", "calculando…").to_owned());
        let mut body = column![
            text(format!("{}    {percent}%", stage.label(self.language)))
                .size(14)
                .color(TEXT),
        ]
        .spacing(8)
        .width(Fill);
        if matches!(
            stage,
            ExportStage::Preparing
                | ExportStage::Downloading
                | ExportStage::Processing
                | ExportStage::Finalizing
        ) {
            body = body.push(
                text(format!(
                    "{}  {speed}      {}  {eta}",
                    self.t("Speed", "Velocidade"),
                    self.t("Time left", "Tempo restante"),
                ))
                .size(13)
                .color(MUTED),
            );
        }
        container(
            body.push(
                progress_bar(0.0..=100.0, percent as f32)
                    .girth(6)
                    .style(export_progress_style),
            ),
        )
        .width(Fill)
        .padding(14)
        .style(|_theme| inset_style())
        .into()
    }

    fn preview_volume_hint(&self) -> &'static str {
        if self.toolchain.vlc.is_some() {
            self.t(
                "The volume slider is on the bottom bar of the player window.",
                "O volume fica na barra inferior da janela do reprodutor.",
            )
        } else if self.toolchain.mpv.is_some() {
            self.t(
                "Move the pointer over the video to show the volume control.",
                "Passe o mouse sobre o vídeo para mostrar o volume.",
            )
        } else if in_app_preview_volume_supported() {
            self.t(
                "Volume buttons stay in ClipForge while the preview plays.",
                "Os botões de volume ficam no ClipForge enquanto a pré-visualização toca.",
            )
        } else {
            self.t(
                "Use 0-9 or the arrow keys in the preview window to change volume.",
                "Use 0-9 ou as setas na janela da pré-visualização para mudar o volume.",
            )
        }
    }

    fn preview_actions<'a>(&self, actions: Element<'a, Message>) -> Element<'a, Message> {
        let hint = text(self.preview_volume_hint()).size(13).color(MUTED);
        if !self.previewing {
            return column![hint, actions].spacing(10).into();
        }
        let stage = self
            .preview_progress
            .as_ref()
            .map(|progress| progress.get().label(self.language))
            .unwrap_or_else(|| {
                self.t(
                    "Preparing the preview. Please wait…",
                    "Preparando a pré-visualização. Aguarde…",
                )
            });
        column![
            container(
                column![
                    text(self.t("Please wait", "Aguarde"))
                        .size(15)
                        .color(ACCENT),
                    text(stage).size(14).color(TEXT),
                    text(self.t(
                        "The preview opens in a separate window. Keep ClipForge open.",
                        "A pré-visualização abre em outra janela. Mantenha o ClipForge aberto.",
                    ))
                    .size(13)
                    .color(MUTED),
                    hint,
                ]
                .spacing(6),
            )
            .width(Fill)
            .padding(14)
            .style(|_theme| notice_style()),
            actions,
        ]
        .spacing(10)
        .into()
    }

    fn format_chip(&self, format: ExportFormat) -> Element<'_, Message> {
        let selected = self.output_format == format;
        let label = match format {
            ExportFormat::Mp4 => "MP4",
            ExportFormat::Mp3 => "MP3",
            ExportFormat::M4a => "M4A",
        };
        button(text(label).size(14))
            .width(Fill)
            .padding([10, 8])
            .on_press(Message::OutputFormatChanged(format))
            .style(if selected {
                primary_button
            } else {
                secondary_button
            })
            .into()
    }

    fn downloads_page(&self) -> Element<'_, Message> {
        let mut header = column![
            text(self.t("Recent exports", "Exportações recentes"))
                .size(28)
                .color(TEXT),
            text(self.t(
                "Clips saved on this computer. Open one to play it.",
                "Clipes salvos neste computador. Abra um para reproduzir.",
            ))
            .size(15)
            .color(MUTED),
        ]
        .spacing(8)
        .width(Fill);
        if !self.history.is_empty() {
            header = header.push(
                button(text(self.t("Clear list", "Limpar lista")).size(14))
                    .on_press(Message::RequestClearExports)
                    .padding([10, 14])
                    .style(danger_button),
            );
        }
        let mut page = column![header].spacing(16).width(Fill);
        if let Some(confirmation) = self.history_confirmation() {
            page = page.push(confirmation);
        }
        if self.history.is_empty() {
            page = page.push(card(
                column![
                    text(self.t("No exports yet", "Nenhuma exportação ainda"))
                        .size(18)
                        .color(TEXT),
                    text(self.t(
                        "Finished clips appear here, including ones from earlier sessions.",
                        "Os clipes concluídos aparecem aqui, inclusive os de sessões anteriores.",
                    ))
                    .size(14)
                    .color(MUTED),
                    button(text(self.t("Create a clip", "Criar um clipe")).size(14))
                        .on_press(Message::SelectPage(Page::Create))
                        .padding([11, 16])
                        .style(primary_button),
                ]
                .spacing(12),
            ));
        } else {
            for path in &self.history {
                page = page.push(self.history_row(path));
            }
        }
        page.padding(Padding {
            bottom: 28.0,
            ..Padding::default()
        })
        .into()
    }

    fn history_confirmation(&self) -> Option<Element<'_, Message>> {
        let prompt = self.history_prompt.as_ref()?;
        let (title, detail, confirm) = match prompt {
            HistoryPrompt::Remove(path) => (
                self.t(
                    "Remove this export from the list?",
                    "Remover esta exportação da lista?",
                ),
                format!(
                    "{}\n{}",
                    export_name(path),
                    self.t("The file stays on disk.", "O arquivo permanece no disco.",)
                ),
                self.t("Remove", "Remover"),
            ),
            HistoryPrompt::Clear => (
                self.t(
                    "Clear every export from the list?",
                    "Limpar todas as exportações da lista?",
                ),
                self.t(
                    "The files stay on disk.",
                    "Os arquivos permanecem no disco.",
                )
                .to_owned(),
                self.t("Clear list", "Limpar lista"),
            ),
        };
        Some(
            container(
                column![
                    text(title).size(16).color(TEXT),
                    text(detail).size(13).color(MUTED),
                    row![
                        button(text(self.t("Cancel", "Cancelar")).size(14))
                            .on_press(Message::DismissHistoryAction)
                            .padding([10, 14])
                            .style(secondary_button),
                        button(text(confirm).size(14))
                            .on_press(Message::ConfirmHistoryAction)
                            .padding([10, 14])
                            .style(danger_button),
                    ]
                    .spacing(10),
                ]
                .spacing(10),
            )
            .width(Fill)
            .padding(16)
            .style(|_theme| notice_style())
            .into(),
        )
    }

    fn history_row(&self, path: &str) -> Element<'_, Message> {
        container(
            row![
                column![
                    text(export_name(path).to_owned()).size(15).color(TEXT),
                    text(path.to_owned()).size(12).color(MUTED),
                ]
                .spacing(4)
                .width(Fill),
                button(text(self.t("Open", "Abrir")).size(14))
                    .on_press(Message::OpenExport(path.to_owned()))
                    .padding([10, 14])
                    .style(secondary_button),
                button(text(self.t("Remove", "Remover")).size(14))
                    .on_press(Message::RequestRemoveExport(path.to_owned()))
                    .padding([10, 14])
                    .style(danger_button),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        )
        .width(Fill)
        .padding(16)
        .style(|_theme| surface_style())
        .into()
    }

    fn language_switch(&self) -> Element<'_, Message> {
        row![
            self.language_chip(Language::Portuguese),
            self.language_chip(Language::English),
        ]
        .spacing(8)
        .into()
    }

    fn language_chip(&self, language: Language) -> Element<'_, Message> {
        let selected = self.language == language;
        button(text(language.to_string()).size(13))
            .width(Fill)
            .padding([8, 8])
            .on_press(Message::LanguageChanged(language))
            .style(if selected {
                primary_button
            } else {
                secondary_button
            })
            .into()
    }
}

fn export_name(path: &str) -> &str {
    std::path::Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
}

fn field_label(value: &str) -> Element<'_, Message> {
    text(value).size(13).color(MUTED).into()
}

fn nav_button(
    label: impl Into<String>,
    active: bool,
    message: Message,
) -> Element<'static, Message> {
    button(text(label.into()).size(15))
        .width(Fill)
        .padding([11, 12])
        .on_press(message)
        .style(move |_theme, status| nav_style(active, status))
        .into()
}

fn brand_mark() -> Element<'static, Message> {
    use std::sync::LazyLock;

    static MARK: LazyLock<Handle> = LazyLock::new(|| Handle::from_bytes(ICON_256));
    iced::widget::image(MARK.clone())
        .width(36)
        .height(36)
        .content_fit(ContentFit::Cover)
        .into()
}

fn status_dot(color: Color) -> Element<'static, Message> {
    container(iced::widget::Space::new().width(8).height(8))
        .width(8)
        .height(8)
        .style(move |_theme| iced::widget::container::Style {
            background: Some(Background::Color(color)),
            border: Border {
                radius: 8.0.into(),
                ..Border::default()
            },
            ..Default::default()
        })
        .into()
}

fn card<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content)
        .width(Fill)
        .padding(22)
        .style(|_theme| surface_style())
        .into()
}

fn hairline<'a>() -> Element<'a, Message> {
    rule::horizontal(1).style(|_theme| separator()).into()
}

fn step_label(index: u8, current: u8, label: &str) -> Element<'static, Message> {
    text(format!("{index}   {label}"))
        .size(13)
        .color(if index == current { ACCENT } else { FAINT })
        .into()
}

async fn choose_destination(filename: String) -> Result<Option<String>, String> {
    let filter = "Media files | *.mp4 *.mp3 *.m4a";
    let dialog = if command_exists("zenity") {
        tokio::process::Command::new("zenity")
            .args([
                "--file-selection",
                "--save",
                "--title=Choose export destination",
                &format!("--filename={filename}"),
                &format!("--file-filter={filter}"),
            ])
            .output()
            .await
            .map_err(|error| format!("Could not open the save dialog: {error}"))?
    } else if command_exists("kdialog") {
        tokio::process::Command::new("kdialog")
            .args([
                "--getsavefilename",
                &filename,
                filter,
                "--title",
                "Choose export destination",
            ])
            .output()
            .await
            .map_err(|error| format!("Could not open the save dialog: {error}"))?
    } else {
        let picked = rfd::AsyncFileDialog::new()
            .add_filter("Media files", &["mp4", "mp3", "m4a"])
            .set_file_name(&filename)
            .save_file()
            .await;
        return Ok(picked.map(|file| file.path().to_string_lossy().into_owned()));
    };
    if dialog.status.success() {
        let path = String::from_utf8_lossy(&dialog.stdout).trim().to_owned();
        return Ok((!path.is_empty()).then_some(path));
    }
    let diagnostic = String::from_utf8_lossy(&dialog.stderr).trim().to_owned();
    if dialog.status.code() == Some(1)
        && !diagnostic.to_ascii_lowercase().contains("error")
        && !diagnostic.to_ascii_lowercase().contains("failed")
    {
        return Ok(None);
    }
    Err(format!(
        "Save dialog failed: {}",
        if diagnostic.is_empty() {
            format!("exit status {}", dialog.status)
        } else {
            diagnostic
        }
    ))
}

#[derive(Clone, Debug)]
struct Toolchain {
    ytdlp: Option<PathBuf>,
    ffmpeg: Option<PathBuf>,
    deno: Option<PathBuf>,
    vlc: Option<PathBuf>,
    mpv: Option<PathBuf>,
}

impl Default for Toolchain {
    fn default() -> Self {
        Self::probe()
    }
}

impl Toolchain {
    fn probe() -> Self {
        refresh_process_path();
        Self {
            ytdlp: resolve_executable("yt-dlp"),
            ffmpeg: resolve_executable("ffmpeg"),
            deno: resolve_executable("deno"),
            vlc: resolve_executable("vlc"),
            mpv: resolve_executable("mpv"),
        }
    }

    fn ready(&self) -> bool {
        self.ytdlp.is_some() && self.ffmpeg.is_some()
    }

    fn missing_required(&self) -> Vec<&'static str> {
        let mut missing = Vec::new();
        if self.ytdlp.is_none() {
            missing.push("yt-dlp");
        }
        if self.ffmpeg.is_none() {
            missing.push("FFmpeg");
        }
        missing
    }

    fn has_preview_player(&self) -> bool {
        self.vlc.is_some() || self.mpv.is_some()
    }

    fn has_installable_gap(&self) -> bool {
        self.ytdlp.is_none()
            || self.ffmpeg.is_none()
            || self.deno.is_none()
            || !self.has_preview_player()
    }

    fn packages_to_install(&self) -> Vec<WingetPackage> {
        let mut packages = Vec::new();
        if self.ytdlp.is_none() {
            packages.push(WingetPackage::YtDlp);
        }
        if self.ffmpeg.is_none() {
            packages.push(WingetPackage::Ffmpeg);
        }
        if self.deno.is_none() {
            packages.push(WingetPackage::Deno);
        }
        if !self.has_preview_player() {
            packages.push(WingetPackage::Vlc);
        }
        packages
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WingetPackage {
    YtDlp,
    Ffmpeg,
    Deno,
    Vlc,
}

impl WingetPackage {
    fn label(self) -> &'static str {
        match self {
            Self::YtDlp => "yt-dlp",
            Self::Ffmpeg => "FFmpeg",
            Self::Deno => "Deno",
            Self::Vlc => "VLC",
        }
    }

    fn id(self) -> &'static str {
        match self {
            Self::YtDlp => "yt-dlp.yt-dlp",
            Self::Ffmpeg => "Gyan.FFmpeg",
            Self::Deno => "DenoLand.Deno",
            Self::Vlc => "VideoLAN.VLC",
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum ToolPage {
    YtDlp,
    Ffmpeg,
    Deno,
    Vlc,
}

impl ToolPage {
    fn url(self) -> &'static str {
        match self {
            Self::YtDlp => "https://github.com/yt-dlp/yt-dlp#installation",
            Self::Ffmpeg => "https://ffmpeg.org/download.html",
            Self::Deno => "https://docs.deno.com/runtime/getting_started/installation/",
            Self::Vlc => "https://www.videolan.org/vlc/",
        }
    }
}

fn command_exists(name: &str) -> bool {
    resolve_executable(name).is_some()
}

fn resolve_executable(name: &str) -> Option<PathBuf> {
    find_named_tool(&tool_search_dirs(), name)
}

fn find_named_tool(directories: &[PathBuf], name: &str) -> Option<PathBuf> {
    let names = executable_names(name);
    directories.iter().find_map(|directory| {
        names.iter().find_map(|file| {
            let candidate = directory.join(file);
            candidate.is_file().then_some(candidate)
        })
    })
}

fn tool_search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(dir) = app_directory() {
        push_unique_dir(&mut dirs, dir);
    }
    #[cfg(windows)]
    {
        for directory in windows_known_tool_dirs() {
            push_unique_dir(&mut dirs, directory);
        }
    }
    #[cfg(windows)]
    if let Some(home) = user_home() {
        push_unique_dir(&mut dirs, home.join(".deno").join("bin"));
    }
    if let Some(path) = std::env::var_os("PATH") {
        for directory in std::env::split_paths(&path) {
            push_unique_dir(&mut dirs, directory);
        }
    }
    dirs
}

#[cfg(windows)]
fn windows_known_tool_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(local) = std::env::var_os("LOCALAPPDATA")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
    {
        push_unique_dir(
            &mut dirs,
            local.join("Microsoft").join("WinGet").join("Links"),
        );
        push_unique_dir(
            &mut dirs,
            local.join("Programs").join("VideoLAN").join("VLC"),
        );
        push_unique_dir(&mut dirs, local.join("Programs").join("VLC"));
        push_unique_dir(&mut dirs, local.join("Programs").join("mpv"));
        let packages = local.join("Microsoft").join("WinGet").join("Packages");
        if let Ok(entries) = std::fs::read_dir(&packages) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                // yt-dlp.exe and deno.exe usually sit in the package root.
                push_unique_dir(&mut dirs, path.clone());
                collect_winget_bin_dirs(&path, &mut dirs, 0);
            }
        }
    }
    for key in [
        "ProgramFiles",
        "ProgramFiles(x86)",
        "PROGRAMFILES",
        "PROGRAMFILES(X86)",
    ] {
        if let Some(root) = std::env::var_os(key)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
        {
            push_unique_dir(&mut dirs, root.join("VideoLAN").join("VLC"));
            push_unique_dir(&mut dirs, root.join("mpv"));
            push_unique_dir(&mut dirs, root.join("mpv.net"));
        }
    }
    dirs
}

#[cfg(windows)]
fn collect_winget_bin_dirs(dir: &std::path::Path, dirs: &mut Vec<PathBuf>, depth: u8) {
    if depth > 3 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("bin"))
        {
            push_unique_dir(dirs, path.clone());
        }
        collect_winget_bin_dirs(&path, dirs, depth + 1);
    }
}

fn push_unique_dir(dirs: &mut Vec<PathBuf>, dir: PathBuf) {
    if !dir.as_os_str().is_empty() && !dirs.contains(&dir) {
        dirs.push(dir);
    }
}

fn app_directory() -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    if let Some(appimage) = std::env::var_os("APPIMAGE").filter(|value| !value.is_empty()) {
        let path = PathBuf::from(appimage);
        if path.is_file()
            && let Some(parent) = path.parent()
        {
            return Some(parent.to_path_buf());
        }
    }
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(PathBuf::from))
}

fn executable_names(name: &str) -> Vec<String> {
    #[cfg(windows)]
    {
        let mut names = Vec::new();
        if !name.to_ascii_lowercase().ends_with(".exe") {
            names.push(format!("{name}.exe"));
        }
        names.push(name.to_owned());
        names
    }
    #[cfg(not(windows))]
    {
        vec![name.to_owned()]
    }
}

fn require_executable(name: &str) -> Result<PathBuf, String> {
    resolve_executable(name).ok_or_else(|| missing_tool_error(name))
}

fn missing_tool_error(name: &str) -> String {
    match name {
        "yt-dlp" => AppError::MissingYtdlp.to_string(),
        "ffmpeg" => "FFmpeg is not installed. Install FFmpeg and retry.".into(),
        "ffplay" | "mpv" | "vlc" => {
            "Install VLC, MPV, or FFplay to preview the selected interval.".into()
        }
        other => format!("{other} is not installed."),
    }
}

fn prepare_media_command(program: PathBuf) -> tokio::process::Command {
    let mut command = tokio::process::Command::new(program);
    if let Ok(path) = std::env::join_paths(tool_search_dirs()) {
        command.env("PATH", path);
    }
    // yt-dlp and FFmpeg are console apps on Windows; without this they open an empty terminal.
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

fn media_tool(name: &str) -> Result<tokio::process::Command, String> {
    Ok(prepare_media_command(require_executable(name)?))
}

fn export_blocker(toolchain: &Toolchain, language: Language) -> Option<String> {
    let missing = toolchain.missing_required();
    if missing.is_empty() {
        return None;
    }
    let names = missing.join(", ");
    Some(match language {
        Language::Portuguese if can_auto_install() => {
            format!("Falta instalar: {names}. Aceite Instalar o que falta, ou copie os comandos.")
        }
        Language::Portuguese => {
            format!("Falta instalar: {names}. Copie os comandos acima e tente de novo.")
        }
        Language::English if can_auto_install() => {
            format!("Still missing: {names}. Choose Install missing tools, or copy the commands.")
        }
        Language::English => {
            format!("Still missing: {names}. Copy the commands above and try again.")
        }
    })
}

fn can_auto_install() -> bool {
    #[cfg(windows)]
    {
        resolve_executable("winget").is_some()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn toolchain_summary(toolchain: &Toolchain, language: Language) -> String {
    let missing = toolchain.missing_required();
    if missing.is_empty() {
        return match (language, toolchain.deno.is_some()) {
            (Language::Portuguese, true) => "yt-dlp, FFmpeg e Deno encontrados.".into(),
            (Language::Portuguese, false) => {
                "yt-dlp e FFmpeg encontrados. O Deno continua recomendado para o YouTube.".into()
            }
            (Language::English, true) => "yt-dlp, FFmpeg, and Deno were found.".into(),
            (Language::English, false) => {
                "yt-dlp and FFmpeg were found. Deno is still recommended for YouTube.".into()
            }
        };
    }
    let names = missing.join(", ");
    match language {
        Language::Portuguese => format!("Ainda falta: {names}."),
        Language::English => format!("Still missing: {names}."),
    }
}

fn install_commands_text(language: Language) -> String {
    let _ = language;
    #[cfg(windows)]
    {
        "winget install --id yt-dlp.yt-dlp -e --accept-package-agreements --accept-source-agreements\n\
winget install --id Gyan.FFmpeg -e --accept-package-agreements --accept-source-agreements\n\
winget install --id DenoLand.Deno -e --accept-package-agreements --accept-source-agreements\n\
winget install --id VideoLAN.VLC -e --accept-package-agreements --accept-source-agreements"
            .into()
    }
    #[cfg(not(windows))]
    {
        linux_install_command().into()
    }
}

fn install_instructions(language: Language) -> String {
    let commands = install_commands_text(language);
    #[cfg(windows)]
    {
        match language {
            Language::Portuguese => format!(
                "Comandos no Terminal do Windows:\n\n{commands}\n\nSe a instalação automática não atualizar o PATH, feche e abra o ClipForge."
            ),
            Language::English => format!(
                "Windows Terminal commands:\n\n{commands}\n\nIf automatic install does not refresh PATH, quit and reopen ClipForge."
            ),
        }
    }
    #[cfg(not(windows))]
    {
        match language {
            Language::Portuguese => format!(
                "No terminal:\n\n{commands}\n\nO Deno é opcional e recomendado para o YouTube. Use a página oficial se a distribuição não tiver o pacote."
            ),
            Language::English => format!(
                "In a terminal:\n\n{commands}\n\nDeno is optional and recommended for YouTube. Use the official page if your distribution has no package."
            ),
        }
    }
}

#[cfg(not(windows))]
fn linux_install_command() -> &'static str {
    if resolve_executable("dnf").is_some() {
        "sudo dnf install yt-dlp ffmpeg"
    } else if resolve_executable("apt").is_some() {
        "sudo apt install yt-dlp ffmpeg"
    } else if resolve_executable("pacman").is_some() {
        "sudo pacman -S yt-dlp ffmpeg"
    } else if resolve_executable("zypper").is_some() {
        "sudo zypper install yt-dlp ffmpeg"
    } else {
        "sudo <package manager> install yt-dlp ffmpeg"
    }
}

fn deno_install_hint(language: Language) -> String {
    #[cfg(windows)]
    {
        let _language = language;
        "winget install --id DenoLand.Deno -e --accept-package-agreements --accept-source-agreements"
            .into()
    }
    #[cfg(not(windows))]
    {
        match language {
            Language::Portuguese => {
                "Instale o pacote deno da distribuição, ou use a página oficial.".into()
            }
            Language::English => {
                "Install the distribution deno package, or use the official page.".into()
            }
        }
    }
}

fn refresh_process_path() {
    #[cfg(windows)]
    {
        let Some(path) = windows_registry_path() else {
            return;
        };
        // SAFETY: ClipForge only refreshes PATH around tool probing on the UI thread.
        unsafe { std::env::set_var("PATH", path) };
    }
}

#[cfg(windows)]
fn windows_registry_path() -> Option<String> {
    let output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "[Environment]::GetEnvironmentVariable('Path','Machine') + ';' + [Environment]::GetEnvironmentVariable('Path','User')",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (!path.is_empty()).then_some(path)
}

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

fn winget_code(status: &std::process::ExitStatus) -> u32 {
    status.code().map(|code| code as u32).unwrap_or(u32::MAX)
}

fn winget_result_is_ok(code: u32) -> bool {
    matches!(
        code,
        0
            // Already up to date / no applicable update.
            | 0x8A15_002B
            // Package already installed.
            | 0x8A15_0061
            | 0x8A15_010D
    )
}

fn winget_code_message(code: u32, package: &str, language: Language) -> String {
    match (code, language) {
        (0x8A15_002B, Language::Portuguese) => format!(
            "O winget diz que {package} já está atualizado. Se o ClipForge ainda não o encontra, reinicie o app."
        ),
        (0x8A15_002B, Language::English) => format!(
            "winget says {package} is already up to date. If ClipForge still cannot find it, reopen the app."
        ),
        (0x8A15_0061 | 0x8A15_010D, Language::Portuguese) => {
            format!("{package} já estava instalado.")
        }
        (0x8A15_0061 | 0x8A15_010D, Language::English) => {
            format!("{package} was already installed.")
        }
        (_, Language::Portuguese) => format!(
            "O winget não conseguiu instalar {package} (0x{code:08x}). Copie os comandos e instale manualmente."
        ),
        (_, Language::English) => format!(
            "winget could not install {package} (0x{code:08x}). Copy the commands and install it manually."
        ),
    }
}

async fn install_missing_tools(
    toolchain: Toolchain,
    language: Language,
    progress: Arc<InstallProgress>,
) -> Result<String, String> {
    #[cfg(windows)]
    {
        if resolve_executable("winget").is_none() {
            return Err(match language {
                Language::Portuguese => {
                    "O winget não foi encontrado. Instale o Instalador de Aplicativos da Microsoft Store, ou copie os comandos."
                        .into()
                }
                Language::English => {
                    "winget was not found. Install App Installer from Microsoft Store, or copy the commands."
                        .into()
                }
            });
        }
        let packages = toolchain.packages_to_install();
        progress
            .total
            .store(packages.len().max(1) as u8, Ordering::Relaxed);
        if packages.is_empty() {
            return Ok(match language {
                Language::Portuguese => "Tudo o que é necessário já está instalado.".into(),
                Language::English => "Everything needed is already installed.".into(),
            });
        }
        let mut installed = Vec::new();
        let mut notes = Vec::new();
        for (index, package) in packages.iter().enumerate() {
            progress.set_package((index + 1) as u8, package.label(), language);
            let status = tokio::process::Command::new("winget")
                .args([
                    "install",
                    "--id",
                    package.id(),
                    "-e",
                    "--accept-package-agreements",
                    "--accept-source-agreements",
                    "--disable-interactivity",
                ])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .creation_flags(CREATE_NO_WINDOW)
                .status()
                .await
                .map_err(|error| match language {
                    Language::Portuguese => format!("Não foi possível iniciar o winget: {error}"),
                    Language::English => format!("Could not start winget: {error}"),
                })?;
            let code = winget_code(&status);
            if winget_result_is_ok(code) {
                installed.push(package.label());
                if code != 0 {
                    notes.push(winget_code_message(code, package.label(), language));
                }
                continue;
            }
            let message = winget_code_message(code, package.label(), language);
            if matches!(package, WingetPackage::Deno | WingetPackage::Vlc) {
                notes.push(message);
                continue;
            }
            return Err(message);
        }
        refresh_process_path();
        let mut summary = match language {
            Language::Portuguese => format!(
                "Instalação concluída: {}. Clique em Verificar de novo se ainda faltar alguma ferramenta.",
                installed.join(", ")
            ),
            Language::English => format!(
                "Install finished: {}. Choose Check again if a tool is still missing.",
                installed.join(", ")
            ),
        };
        if !notes.is_empty() {
            summary.push(' ');
            summary.push_str(&notes.join(" "));
        }
        Ok(summary)
    }
    #[cfg(not(windows))]
    {
        let _ = (toolchain, progress);
        Err(match language {
            Language::Portuguese => {
                "A instalação automática está disponível no Windows com o winget. Copie os comandos da sua distribuição."
                    .into()
            }
            Language::English => {
                "Automatic install is available on Windows with winget. Copy the commands for your distribution."
                    .into()
            }
        })
    }
}

fn dependency_line<'a>(
    name: &'a str,
    role: &'a str,
    state: &'a str,
    state_color: Color,
) -> Element<'a, Message> {
    column![
        row![
            text(name).size(14).color(TEXT),
            text(state).size(13).color(state_color),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
        text(role).size(12).color(MUTED),
    ]
    .spacing(2)
    .into()
}

fn metadata_notice(error: &str, language: Language) -> String {
    if error == AppError::MissingYtdlp.to_string()
        || error == "FFmpeg is not installed. Install FFmpeg and retry."
    {
        return localized_error(error, language);
    }
    let detail = summarize_external_error(error);
    match language {
        Language::Portuguese => format!("Falha ao carregar metadados. {detail}"),
        Language::English => format!("Metadata lookup failed. {detail}"),
    }
}

fn summarize_external_error(error: &str) -> String {
    let line = error
        .lines()
        .map(str::trim)
        .rfind(|line| !line.is_empty() && !line.starts_with("WARNING:"))
        .unwrap_or("Check the URL and your connection.");
    let mut summary: String = line.chars().take(220).collect();
    if line.chars().count() > 220 {
        summary.push('…');
    }
    summary
}

async fn open_web_page(url: &'static str) -> Result<(), String> {
    open_command(url)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Could not open the installation page: {error}"))
}

fn history_file() -> Option<PathBuf> {
    Some(state_dir()?.join("history.json"))
}

fn state_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| user_home().map(|home| home.join("AppData").join("Local")))
            .map(|base| base.join("clipforge"))
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("XDG_STATE_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| user_home().map(|home| home.join(".local/state")))
            .map(|base| base.join("clipforge"))
    }
}

fn user_home() -> Option<PathBuf> {
    #[cfg(windows)]
    let key = "USERPROFILE";
    #[cfg(not(windows))]
    let key = "HOME";
    std::env::var_os(key)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn load_history() -> Vec<String> {
    history_file()
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice::<Vec<String>>(&bytes).ok())
        .unwrap_or_default()
}

fn save_history(entries: &[String]) {
    let Some(path) = history_file() else { return };
    if let Some(parent) = path.parent()
        && std::fs::create_dir_all(parent).is_err()
    {
        return;
    }
    if let Ok(data) = serde_json::to_vec(entries) {
        let _ = std::fs::write(path, data);
    }
}

fn drop_export(history: &mut Vec<String>, path: &str) {
    history.retain(|entry| entry != path);
}

fn language_file() -> Option<PathBuf> {
    history_file().map(|path| path.with_file_name("language"))
}
fn parse_language(value: Option<&str>) -> Language {
    match value.map(str::trim) {
        Some("pt") => Language::Portuguese,
        _ => Language::English,
    }
}
fn language_code(language: Language) -> &'static str {
    match language {
        Language::Portuguese => "pt",
        Language::English => "en",
    }
}
fn load_language() -> Language {
    parse_language(
        language_file()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .as_deref(),
    )
}
fn save_language(language: Language) {
    if let Some(path) = language_file() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, language_code(language));
    }
}

async fn open_export(path: String) -> Result<(), String> {
    open_command(&path)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Could not open export: {error}"))
}

fn open_command(path: &str) -> tokio::process::Command {
    #[cfg(windows)]
    {
        let mut command = tokio::process::Command::new("cmd");
        command.args(["/C", "start", "", path]);
        command.creation_flags(CREATE_NO_WINDOW);
        command
    }
    #[cfg(not(windows))]
    {
        let mut command = tokio::process::Command::new("xdg-open");
        command.arg(path);
        command
    }
}
#[derive(Debug, Clone)]
struct VideoInfo {
    url: String,
    title: String,
    channel: String,
    thumbnail: Option<String>,
    duration: u64,
    formats: Vec<FormatChoice>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct FormatChoice {
    id: String,
    label: String,
}

impl std::fmt::Display for FormatChoice {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.label)
    }
}
#[derive(Deserialize)]
struct YtInfo {
    title: String,
    #[serde(default)]
    uploader: Option<String>,
    #[serde(default)]
    channel: Option<String>,
    duration: Option<f64>,
    #[serde(default)]
    thumbnail: Option<String>,
    #[serde(default)]
    formats: Vec<YtFormat>,
}
#[derive(Deserialize)]
struct YtFormat {
    format_id: String,
    #[serde(default)]
    height: Option<u32>,
    #[serde(default)]
    ext: String,
    #[serde(default)]
    acodec: Option<String>,
    #[serde(default)]
    tbr: Option<f64>,
}
#[derive(Debug, Error)]
enum AppError {
    #[error("Enter a valid YouTube video URL (youtube.com or youtu.be).")]
    InvalidUrl,
    #[error("yt-dlp is not installed. Install yt-dlp and retry.")]
    MissingYtdlp,
    #[error("Invalid clip range: use HH:MM:SS and ensure start < end ≤ video duration.")]
    InvalidRange,
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

fn localized_error(error: &str, language: Language) -> String {
    if language != Language::Portuguese {
        return error.to_owned();
    }
    match error {
        "Enter a valid YouTube video URL (youtube.com or youtu.be)." => {
            "Informe uma URL válida do YouTube (youtube.com ou youtu.be).".into()
        }
        "yt-dlp is not installed. Install yt-dlp and retry." => {
            "O yt-dlp não está instalado. Instale-o e tente novamente.".into()
        }
        "FFmpeg is not installed. Install FFmpeg and retry." => {
            "O FFmpeg não está instalado. Instale-o e tente novamente.".into()
        }
        "Invalid clip range: use HH:MM:SS and ensure start < end ≤ video duration." => {
            "Intervalo inválido: use HH:MM:SS e verifique início < fim ≤ duração do vídeo.".into()
        }
        "Cancelling export…" => "Cancelando exportação…".into(),
        "Opened selected interval in MPV." => "Trecho selecionado aberto no MPV.".into(),
        "Install VLC, MPV, or FFplay to preview the selected interval." => {
            "Instale o VLC, o MPV ou o FFplay para pré-visualizar o trecho.".into()
        }
        "VLC could not open the preview." => {
            "O VLC não conseguiu abrir a pré-visualização.".into()
        }
        "FFplay could not open the preview." => {
            "O FFplay não conseguiu abrir a pré-visualização.".into()
        }
        "Install WirePlumber (wpctl) or PulseAudio (pactl) to change the preview volume." => {
            "Instale o WirePlumber (wpctl) ou o PulseAudio (pactl) para mudar o volume da pré-visualização.".into()
        }
        "The preview audio stream is not available yet. Try the volume buttons again in a moment." => {
            "O áudio da pré-visualização ainda não está disponível. Tente o volume de novo em instantes.".into()
        }
        other => other.to_owned(),
    }
}

async fn set_preview_volume(percent: u8) -> Result<(), String> {
    let percent = percent.min(100);
    if !command_exists("wpctl") && !command_exists("pactl") {
        return Err(
            "Install WirePlumber (wpctl) or PulseAudio (pactl) to change the preview volume."
                .into(),
        );
    }
    for _ in 0..10 {
        if try_set_preview_volume(percent).await? {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    Err(
        "The preview audio stream is not available yet. Try the volume buttons again in a moment."
            .into(),
    )
}

async fn try_set_preview_volume(percent: u8) -> Result<bool, String> {
    let volume = format!("{percent}%");
    if command_exists("wpctl") {
        let output = tokio::process::Command::new("wpctl")
            .arg("status")
            .output()
            .await
            .map_err(|error| format!("Could not inspect PipeWire streams: {error}"))?;
        if output.status.success() {
            let ids = wpctl_preview_ids(&String::from_utf8_lossy(&output.stdout));
            if !ids.is_empty() {
                for id in ids {
                    let status = tokio::process::Command::new("wpctl")
                        .args(["set-volume", &id, &volume])
                        .status()
                        .await
                        .map_err(|error| format!("Could not change the preview volume: {error}"))?;
                    if !status.success() {
                        return Err(format!(
                            "wpctl could not set the preview volume ({status})."
                        ));
                    }
                }
                return Ok(true);
            }
        }
    }
    if command_exists("pactl") {
        let output = tokio::process::Command::new("pactl")
            .args(["list", "sink-inputs"])
            .output()
            .await
            .map_err(|error| format!("Could not inspect PulseAudio streams: {error}"))?;
        if output.status.success() {
            let ids = pactl_preview_ids(&String::from_utf8_lossy(&output.stdout));
            if !ids.is_empty() {
                for id in ids {
                    let status = tokio::process::Command::new("pactl")
                        .args(["set-sink-input-volume", &id, &volume])
                        .status()
                        .await
                        .map_err(|error| format!("Could not change the preview volume: {error}"))?;
                    if !status.success() {
                        return Err(format!(
                            "pactl could not set the preview volume ({status})."
                        ));
                    }
                }
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn wpctl_preview_ids(status: &str) -> Vec<String> {
    let mut in_streams = false;
    let mut ids = Vec::new();
    for line in status.lines() {
        let compact = line.replace(['│', '├', '└', '─', '*'], " ");
        let trimmed = compact.trim();
        if trimmed.starts_with("Streams:") {
            in_streams = true;
            continue;
        }
        if in_streams && trimmed.ends_with(':') && !trimmed.contains('.') {
            in_streams = false;
            continue;
        }
        if !in_streams || !preview_player_line(trimmed) {
            continue;
        }
        if let Some(id) = trimmed.split_whitespace().find_map(|token| {
            token
                .strip_suffix('.')
                .filter(|value| !value.is_empty() && value.chars().all(|c| c.is_ascii_digit()))
                .map(str::to_owned)
        }) {
            ids.push(id);
        }
    }
    ids
}

fn pactl_preview_ids(list: &str) -> Vec<String> {
    let mut current = None;
    let mut matched = false;
    let mut ids = Vec::new();
    for line in list.lines() {
        if let Some(rest) = line.trim().strip_prefix("Sink Input #") {
            if let Some(id) = matched.then_some(current).flatten() {
                ids.push(id);
            }
            current = Some(rest.trim().to_owned());
            matched = false;
            continue;
        }
        if preview_player_line(line) {
            matched = true;
        }
    }
    if let Some(id) = matched.then_some(current).flatten() {
        ids.push(id);
    }
    ids
}

fn preview_player_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.contains("ffplay") || lower.contains("mpv") || lower.contains("clipforge")
}

fn validate_video_url(input: &str) -> Result<String, AppError> {
    let url = Url::parse(input.trim()).map_err(|_| AppError::InvalidUrl)?;
    let host = url.host_str().unwrap_or("").to_ascii_lowercase();
    let supported = matches!(
        host.as_str(),
        "youtube.com"
            | "www.youtube.com"
            | "m.youtube.com"
            | "youtu.be"
            | "www.youtube-nocookie.com"
            | "youtube-nocookie.com"
    );
    if url.scheme() != "https" || !supported || (host.contains("youtu") && url.path() == "/") {
        return Err(AppError::InvalidUrl);
    }
    Ok(url.to_string())
}
fn parse_time(input: &str) -> Option<u64> {
    let mut parts = input.trim().split(':');
    let hours: u64 = parts.next()?.parse().ok()?;
    let minutes: u64 = parts.next()?.parse().ok()?;
    let seconds: u64 = parts.next()?.parse().ok()?;
    if parts.next().is_some() || minutes >= 60 || seconds >= 60 {
        return None;
    }
    hours
        .checked_mul(3600)?
        .checked_add(minutes.checked_mul(60)?)?
        .checked_add(seconds)
}
fn format_time(seconds: u64) -> String {
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        (seconds / 60) % 60,
        seconds % 60
    )
}
fn safe_file_stem(title: &str) -> String {
    let stem = title
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | ' ') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let stem = stem.trim_matches([' ', '.', '-']);
    let stem = stem.chars().take(80).collect::<String>();
    if stem.is_empty() { "clip".into() } else { stem }
}
fn default_destination(title: &str) -> PathBuf {
    let filename = format!("{}.mp4", safe_file_stem(title));
    if let Some(home) = user_home()
        && home.is_absolute()
    {
        return default_destination_in(&home, &filename);
    }
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(filename)
}

fn default_destination_in(home: &std::path::Path, filename: &str) -> PathBuf {
    home.join("Downloads").join(filename)
}
fn progress_details(line: &str) -> Option<(u8, Option<String>, Option<String>)> {
    let value = line.split_once("CLIP_PROGRESS:")?.1;
    let mut fields = value.split('|');
    let percent = fields.next()?.parse::<f32>().ok()?;
    if !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
        return None;
    }
    let clean = |value: Option<&str>| {
        value
            .filter(|s| !s.is_empty() && !matches!(*s, "NA" | "N/A"))
            .map(str::to_owned)
    };
    Some((
        percent.round() as u8,
        clean(fields.next()),
        clean(fields.next()),
    ))
}
fn parse_progress_percent(line: &str) -> Option<u8> {
    if let Some((percent, _, _)) = progress_details(line) {
        return Some(percent);
    }
    let marker = line.find('%')?;
    let token = line[..marker].split_whitespace().last()?;
    let percent: f32 = token.trim_end_matches('%').parse().ok()?;
    (percent.is_finite() && (0.0..=100.0).contains(&percent)).then_some(percent.round() as u8)
}
fn parse_ffmpeg_progress(
    line: &str,
    clip_duration: u64,
) -> Option<(u8, Option<String>, Option<String>)> {
    let timestamp = line
        .split_whitespace()
        .find_map(|field| field.strip_prefix("time="))?;
    let current = parse_ffmpeg_timestamp(timestamp)?;
    if clip_duration == 0 {
        return None;
    }
    let percent = (current / clip_duration as f64 * 100.0)
        .round()
        .clamp(0.0, 100.0) as u8;
    let speed_field = line
        .split_whitespace()
        .find_map(|field| field.strip_prefix("speed="));
    let speed = speed_field
        .filter(|value| *value != "N/A")
        .map(|value| format!("FFmpeg {value}"));
    let eta = speed_field
        .and_then(|value| value.trim_end_matches('x').parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value > 0.0)
        .map(
            |value| format_time(((clip_duration as f64 - current).max(0.0) / value).ceil() as u64),
        );
    Some((percent, speed, eta))
}

fn parse_ffmpeg_timestamp(input: &str) -> Option<f64> {
    let mut parts = input.split(':');
    let hours = parts.next()?.parse::<f64>().ok()?;
    let minutes = parts.next()?.parse::<f64>().ok()?;
    let seconds = parts.next()?.parse::<f64>().ok()?;
    if parts.next().is_some()
        || !hours.is_finite()
        || !minutes.is_finite()
        || !seconds.is_finite()
        || hours < 0.0
        || !(0.0..60.0).contains(&minutes)
        || !(0.0..60.0).contains(&seconds)
    {
        return None;
    }
    Some(hours * 3600.0 + minutes * 60.0 + seconds)
}
fn record_export_output(
    line: &str,
    clip_duration: u64,
    progress: &DownloadProgress,
    error_tail: &mut String,
) {
    if line.trim().is_empty() {
        return;
    }
    if let Some((percent, speed, eta)) = parse_ffmpeg_progress(line, clip_duration) {
        progress
            .stage
            .store(ExportStage::Processing as u8, Ordering::Relaxed);
        progress.percent.store(percent, Ordering::Relaxed);
        if let Ok(mut details) = progress.details.lock() {
            details.speed = speed;
            details.eta = eta;
        }
    } else if let Some((percent, speed, eta)) = progress_details(line) {
        progress
            .stage
            .store(ExportStage::Downloading as u8, Ordering::Relaxed);
        progress.percent.store(percent, Ordering::Relaxed);
        if let Ok(mut details) = progress.details.lock() {
            details.speed = speed;
            details.eta = eta;
        }
    } else if let Some(percent) = parse_progress_percent(line) {
        progress
            .stage
            .store(ExportStage::Downloading as u8, Ordering::Relaxed);
        progress.percent.store(percent, Ordering::Relaxed);
    }
    if !error_tail.is_empty() {
        error_tail.push('\n');
    }
    error_tail.push_str(line);
    if error_tail.len() > 8 * 1024 {
        let excess = error_tail.len() - 8 * 1024;
        error_tail.drain(..excess);
    }
}

fn parse_range(start: &str, end: &str, duration: u64) -> Result<(u64, u64), AppError> {
    let a = parse_time(start).ok_or(AppError::InvalidRange)?;
    let b = parse_time(end).ok_or(AppError::InvalidRange)?;
    if a >= b || b > duration {
        Err(AppError::InvalidRange)
    } else {
        Ok((a, b))
    }
}
async fn fetch_metadata(url: String) -> Result<VideoInfo, String> {
    let output = media_tool("yt-dlp")?
        .args([
            "--no-warnings",
            "--dump-single-json",
            "--skip-download",
            "--no-playlist",
            &url,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                AppError::MissingYtdlp.to_string()
            } else {
                e.to_string()
            }
        })?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    let data: YtInfo = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Could not parse yt-dlp metadata: {e}"))?;
    let duration = data
        .duration
        .filter(|n| n.is_finite() && *n > 0.0)
        .ok_or("Video duration is unavailable".to_string())? as u64;
    let formats = format_choices(data.formats);
    Ok(VideoInfo {
        url,
        title: data.title,
        channel: data
            .channel
            .or(data.uploader)
            .unwrap_or_else(|| "Unknown channel".into()),
        thumbnail: data.thumbnail,
        duration,
        formats,
    })
}

fn quality_label(height: u32) -> String {
    let quality = match height {
        2160.. => " · 4K",
        1440..=2159 => " · QHD",
        1080..=1439 => " · Full HD",
        720..=1079 => " · HD",
        480..=719 => " · SD",
        _ => "",
    };
    format!("{height}p{quality}")
}

fn format_choices(formats: Vec<YtFormat>) -> Vec<FormatChoice> {
    let has_video_only = formats
        .iter()
        .any(|format| format.height.is_some() && format.acodec.as_deref() == Some("none"));
    let mut best_by_height: BTreeMap<u32, (bool, f64, String)> = BTreeMap::new();
    for format in formats.into_iter().filter(|format| {
        format.height.is_some() && (!has_video_only || format.acodec.as_deref() == Some("none"))
    }) {
        let height = format.height.unwrap_or_default();
        let candidate = (
            format.ext.eq_ignore_ascii_case("mp4"),
            format.tbr.unwrap_or_default(),
            format.format_id,
        );
        let replace = best_by_height
            .get(&height)
            .is_none_or(|(old_mp4, old_bitrate, _)| {
                (candidate.0, candidate.1) > (*old_mp4, *old_bitrate)
            });
        if replace {
            best_by_height.insert(height, candidate);
        }
    }
    best_by_height
        .into_iter()
        .rev()
        .take(8)
        .map(|(height, (_, _, id))| FormatChoice {
            id,
            label: quality_label(height),
        })
        .collect()
}

fn preview_opened_notice(player: PreviewPlayer, language: Language) -> String {
    let portuguese = language == Language::Portuguese;
    match player {
        PreviewPlayer::Vlc => {
            if portuguese {
                "Pré-visualização aberta. O volume fica na barra inferior da janela do reprodutor."
            } else {
                "Preview opened. The volume slider is on the bottom bar of the player window."
            }
        }
        PreviewPlayer::Mpv => {
            if portuguese {
                "Pré-visualização aberta no MPV. Passe o mouse sobre o vídeo para mostrar o volume."
            } else {
                "Preview opened in MPV. Move the pointer over the video to show the volume control."
            }
        }
        PreviewPlayer::Ffplay => {
            if in_app_preview_volume_supported() {
                if portuguese {
                    "Pré-visualização aberta. Ajuste o volume aqui."
                } else {
                    "Preview opened. Adjust the volume here."
                }
            } else if portuguese {
                "Pré-visualização aberta no FFplay (controles limitados). Instale o VLC para pause, seek e volume."
            } else {
                "Preview opened in FFplay (limited controls). Install VLC for pause, seek, and volume."
            }
        }
    }
    .to_owned()
}

fn in_app_preview_volume_supported() -> bool {
    cfg!(target_os = "linux")
}

fn schedule_preview_window_style() {
    #[cfg(windows)]
    {
        tokio::spawn(async {
            style_preview_window().await;
        });
    }
}

#[cfg(windows)]
async fn style_preview_window() {
    for _ in 0..50 {
        if apply_dark_title_bar(PREVIEW_WINDOW_TITLE) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[cfg(windows)]
struct PreviewWindowSearch {
    title: String,
    found: std::sync::atomic::AtomicBool,
}

#[cfg(windows)]
fn apply_dark_title_bar(title: &str) -> bool {
    let search = PreviewWindowSearch {
        title: title.to_owned(),
        found: std::sync::atomic::AtomicBool::new(false),
    };
    unsafe {
        EnumWindows(Some(enum_preview_windows), &raw const search as isize);
    }
    search.found.load(std::sync::atomic::Ordering::Relaxed)
}

#[cfg(windows)]
unsafe extern "system" fn enum_preview_windows(hwnd: Hwnd, lparam: isize) -> i32 {
    let search = unsafe { &*(lparam as *const PreviewWindowSearch) };
    let mut buffer = [0u16; 512];
    let length = unsafe { GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32) };
    if length <= 0 {
        return 1;
    }
    let window_title = String::from_utf16_lossy(&buffer[..length as usize]);
    if window_title != search.title {
        return 1;
    }
    let enabled: i32 = 1;
    let result = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            (&raw const enabled).cast(),
            std::mem::size_of_val(&enabled) as u32,
        )
    };
    if result >= 0 {
        search
            .found
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
    1
}

#[cfg(windows)]
type Hwnd = *mut std::ffi::c_void;

#[cfg(windows)]
const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;

#[cfg(windows)]
#[link(name = "user32")]
unsafe extern "system" {
    fn EnumWindows(
        callback: Option<unsafe extern "system" fn(Hwnd, isize) -> i32>,
        lparam: isize,
    ) -> i32;
    fn GetWindowTextW(hwnd: Hwnd, text: *mut u16, max_count: i32) -> i32;
}

#[cfg(windows)]
#[link(name = "dwmapi")]
unsafe extern "system" {
    fn DwmSetWindowAttribute(
        hwnd: Hwnd,
        attribute: u32,
        value: *const std::ffi::c_void,
        size: u32,
    ) -> i32;
}

async fn launch_preview(
    url: String,
    start: u64,
    end: u64,
    progress: Arc<PreviewProgress>,
) -> Result<PreviewPlayer, String> {
    if !command_exists("vlc") && !command_exists("mpv") && !command_exists("ffplay") {
        return Err("Install VLC, MPV, or FFplay to preview the selected interval.".into());
    }
    // Download only the selected interval so the player never opens the full source.
    let (mut temp_dir, clip) = download_preview_section(&url, start, end, progress.clone()).await?;
    let path = clip.to_string_lossy().into_owned();
    let mut last_error = None;

    if command_exists("vlc") {
        match spawn_local_preview_player(
            "vlc",
            &vlc_local_preview_args(&path),
            progress.clone(),
            PreviewPlayer::Vlc,
            "VLC",
            "VLC could not open the preview.",
            temp_dir,
        )
        .await
        {
            Ok(player) => return Ok(player),
            Err((error, returned_dir)) => {
                log::warn!("VLC preview failed: {error}");
                last_error = Some(error);
                temp_dir = returned_dir;
            }
        }
    }

    if command_exists("mpv") {
        match spawn_local_preview_player(
            "mpv",
            &mpv_local_preview_args(&path),
            progress.clone(),
            PreviewPlayer::Mpv,
            "MPV",
            "MPV could not open the preview.",
            temp_dir,
        )
        .await
        {
            Ok(player) => return Ok(player),
            Err((error, returned_dir)) => {
                log::warn!("MPV preview failed: {error}");
                last_error = Some(error);
                temp_dir = returned_dir;
            }
        }
    }

    if command_exists("ffplay") {
        return spawn_local_preview_player(
            "ffplay",
            &ffplay_local_preview_args(&path),
            progress,
            PreviewPlayer::Ffplay,
            "FFplay",
            "FFplay could not open the preview.",
            temp_dir,
        )
        .await
        .map_err(|(error, _)| error);
    }

    progress.set(PreviewStage::Failed);
    Err(last_error
        .unwrap_or_else(|| "Install VLC, MPV, or FFplay to preview the selected interval.".into()))
}

fn build_preview_download_args(url: &str, start: u64, end: u64, template: &str) -> Vec<String> {
    vec![
        "--newline".into(),
        "--ignore-config".into(),
        "--no-warnings".into(),
        "--no-playlist".into(),
        "--download-sections".into(),
        format!("*{}-{}", format_time(start), format_time(end)),
        "-f".into(),
        "bv*[height<=1080]+ba/b".into(),
        "--merge-output-format".into(),
        "mp4".into(),
        "-o".into(),
        template.into(),
        url.into(),
    ]
}

async fn download_preview_section(
    url: &str,
    start: u64,
    end: u64,
    progress: Arc<PreviewProgress>,
) -> Result<(tempfile::TempDir, PathBuf), String> {
    progress.set(PreviewStage::Resolving);
    let temp_dir = tempfile::Builder::new()
        .prefix("clipforge-preview-")
        .tempdir()
        .map_err(|error| format!("Could not create a preview temp folder: {error}"))?;
    let template = temp_dir.path().join("preview.%(ext)s");
    let template = template.to_string_lossy().into_owned();
    let args = build_preview_download_args(url, start, end, &template);
    progress.set(PreviewStage::Buffering);
    let output = media_tool("yt-dlp")?
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|error| format!("Could not download the preview section: {error}"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(if detail.is_empty() {
            format!(
                "yt-dlp failed to download the preview section ({}).",
                output.status
            )
        } else {
            format!("yt-dlp could not download the preview section: {detail}")
        });
    }
    let mut entries = tokio::fs::read_dir(temp_dir.path())
        .await
        .map_err(|error| error.to_string())?;
    let mut clip = None;
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|error| error.to_string())?
    {
        let path = entry.path();
        if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| matches!(ext.to_ascii_lowercase().as_str(), "mp4" | "mkv" | "webm"))
        {
            clip = Some(path);
            break;
        }
    }
    let clip =
        clip.ok_or_else(|| "yt-dlp completed without producing a preview media file.".to_owned())?;
    Ok((temp_dir, clip))
}

fn vlc_local_preview_args(path: &str) -> Vec<String> {
    vec![
        "--ignore-config".into(),
        "--intf=qt".into(),
        "--no-one-instance".into(),
        "--no-one-instance-when-started-from-file".into(),
        "--play-and-exit".into(),
        "--no-video-title-show".into(),
        "--no-qt-privacy-ask".into(),
        "--no-qt-system-tray".into(),
        "--qt-notification=0".into(),
        "--qt-continue=0".into(),
        format!("--meta-title={PREVIEW_WINDOW_TITLE}"),
        "--".into(),
        path.into(),
    ]
}

fn mpv_local_preview_args(path: &str) -> Vec<String> {
    vec![
        "--force-window=yes".into(),
        "--osc=yes".into(),
        "--script-opts=osc-visibility=always".into(),
        "--speed=1".into(),
        format!("--title={PREVIEW_WINDOW_TITLE}"),
        "--".into(),
        path.into(),
    ]
}

fn ffplay_local_preview_args(path: &str) -> Vec<String> {
    vec![
        "-hide_banner".into(),
        "-autoexit".into(),
        "-loglevel".into(),
        "error".into(),
        "-window_title".into(),
        PREVIEW_WINDOW_TITLE.into(),
        "-i".into(),
        path.into(),
    ]
}

async fn spawn_local_preview_player(
    program: &str,
    args: &[String],
    progress: Arc<PreviewProgress>,
    player: PreviewPlayer,
    label: &str,
    failure: &'static str,
    temp_dir: tempfile::TempDir,
) -> Result<PreviewPlayer, (String, tempfile::TempDir)> {
    let mut child = match media_tool(program) {
        Ok(mut command) => match command
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(false)
            .spawn()
        {
            Ok(child) => child,
            Err(error) => {
                return Err((format!("Could not launch {label}: {error}"), temp_dir));
            }
        },
        Err(error) => return Err((error, temp_dir)),
    };
    tokio::time::sleep(Duration::from_millis(400)).await;
    match child.try_wait() {
        Ok(Some(status)) => {
            progress.set(if status.success() {
                PreviewStage::Finished
            } else {
                PreviewStage::Failed
            });
            if !status.success() {
                return Err((failure.to_owned(), temp_dir));
            }
            schedule_preview_window_style();
            // Keep the file until the short-lived player process is fully gone.
            drop(temp_dir);
            Ok(player)
        }
        Ok(None) => {
            progress.set(PreviewStage::Playing);
            schedule_preview_window_style();
            tokio::spawn(async move {
                let status = child.wait().await;
                progress.set(if status.is_ok_and(|status| status.success()) {
                    PreviewStage::Finished
                } else {
                    PreviewStage::Failed
                });
                drop(temp_dir);
            });
            Ok(player)
        }
        Err(error) => Err((
            format!("Could not check {label} startup: {error}"),
            temp_dir,
        )),
    }
}
async fn fetch_thumbnail(url: String) -> Result<Vec<u8>, String> {
    let parsed = Url::parse(&url).map_err(|error| error.to_string())?;
    let host = parsed.host_str().unwrap_or_default();
    let trusted_host = host == "ytimg.com"
        || host.ends_with(".ytimg.com")
        || host == "googleusercontent.com"
        || host.ends_with(".googleusercontent.com");
    if parsed.scheme() != "https" || !trusted_host {
        return Err("Thumbnail URL host is not allowed.".into());
    }
    let response = reqwest::get(parsed)
        .await
        .map_err(|error| error.to_string())?;
    let final_host = response.url().host_str().unwrap_or_default();
    if !(final_host == "ytimg.com"
        || final_host.ends_with(".ytimg.com")
        || final_host == "googleusercontent.com"
        || final_host.ends_with(".googleusercontent.com"))
    {
        return Err("Thumbnail redirected to a disallowed host.".into());
    }
    if !response.status().is_success() {
        return Err(format!(
            "Thumbnail request returned HTTP {}",
            response.status()
        ));
    }
    if response
        .content_length()
        .is_some_and(|size| size > 5 * 1024 * 1024)
    {
        return Err("Thumbnail exceeds the 5 MiB limit.".into());
    }
    let bytes = response.bytes().await.map_err(|error| error.to_string())?;
    if bytes.len() > 5 * 1024 * 1024 {
        return Err("Thumbnail exceeds the 5 MiB limit.".into());
    }
    Ok(bytes.to_vec())
}
fn build_download_args(
    url: &str,
    start: u64,
    end: u64,
    format: ExportFormat,
    selected_format: Option<&str>,
    template: &str,
) -> Vec<String> {
    let video_format = selected_format
        .map(|format_id| format!("{format_id}+bestaudio/best"))
        .unwrap_or_else(|| "bv*+ba/b".to_owned());
    let format_selector = if format == ExportFormat::Mp4 {
        video_format
    } else {
        "bestaudio/best".to_owned()
    };
    let mut args = vec![
        "--newline".into(),
        "--ignore-config".into(),
        "--progress".into(),
        "--progress-template".into(),
        "download:CLIP_PROGRESS:%(progress._percent)s|%(progress._speed_str)s|%(progress._eta_str)s".into(),
        "--download-sections".into(),
        format!("*{}-{}", format_time(start), format_time(end)),
        "--force-keyframes-at-cuts".into(),
        "-f".into(),
        format_selector,
        "-o".into(),
        template.into(),
    ];
    match format {
        ExportFormat::Mp4 => args.extend(["--merge-output-format", "mp4"].map(str::to_owned)),
        ExportFormat::Mp3 => args.extend(["-x", "--audio-format", "mp3"].map(str::to_owned)),
        ExportFormat::M4a => args.extend(["-x", "--audio-format", "m4a"].map(str::to_owned)),
    }
    args.push(url.into());
    args
}
#[allow(clippy::too_many_arguments)]
async fn export(
    url: String,
    start: u64,
    end: u64,
    mut destination: PathBuf,
    selected_format: Option<String>,
    output_format: ExportFormat,
    progress: Arc<DownloadProgress>,
    mut cancel: watch::Receiver<bool>,
) -> Result<String, String> {
    destination.set_extension(output_format.extension());
    if tokio::fs::try_exists(&destination)
        .await
        .map_err(|error| error.to_string())?
    {
        return Err(format!("Output already exists: {}", destination.display()));
    }
    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|error| error.to_string())?;

    let temp_dir = tempfile::Builder::new()
        .prefix("clipforge-")
        .tempdir_in(parent)
        .map_err(|error| error.to_string())?;
    let template = temp_dir.path().join("clip.%(ext)s");
    let template = template.to_string_lossy().into_owned();
    let args = build_download_args(
        &url,
        start,
        end,
        output_format,
        selected_format.as_deref(),
        &template,
    );
    let program = require_executable("yt-dlp")?;
    require_executable("ffmpeg")?;
    let mut command = prepare_media_command(program);
    command
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command.spawn().map_err(|error| error.to_string())?;
    progress
        .stage
        .store(ExportStage::Downloading as u8, Ordering::Relaxed);
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "yt-dlp stderr was not available".to_owned())?;
    let mut reader = BufReader::new(stderr);
    let mut chunk = [0; 8192];
    let mut pending = Vec::with_capacity(1024);
    let mut error_tail = String::new();
    let clip_duration = end - start;
    loop {
        let count = tokio::select! {
            result = reader.read(&mut chunk) => result.map_err(|error| error.to_string())?,
            _ = cancel.changed() => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                return Err("Export cancelled.".into());
            }
        };
        if count == 0 {
            break;
        }
        for byte in chunk[..count].iter().copied() {
            if byte == b'\n' || byte == b'\r' {
                if !pending.is_empty() {
                    let line = String::from_utf8_lossy(&pending);
                    record_export_output(&line, clip_duration, &progress, &mut error_tail);
                    pending.clear();
                }
            } else if pending.len() < 16 * 1024 {
                pending.push(byte);
            }
        }
    }
    if !pending.is_empty() {
        let line = String::from_utf8_lossy(&pending);
        record_export_output(&line, clip_duration, &progress, &mut error_tail);
    }
    let status = child.wait().await.map_err(|error| error.to_string())?;
    if !status.success() {
        return Err(error_tail.trim().to_owned());
    }
    progress
        .stage
        .store(ExportStage::Finalizing as u8, Ordering::Relaxed);
    progress.percent.store(100, Ordering::Relaxed);

    let mut entries = tokio::fs::read_dir(temp_dir.path())
        .await
        .map_err(|error| error.to_string())?;
    let mut clip = None;
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|error| error.to_string())?
    {
        if entry.path().extension().and_then(|ext| ext.to_str()) == Some(output_format.extension())
        {
            clip = Some(entry.path());
            break;
        }
    }
    let clip = clip.ok_or_else(|| {
        "yt-dlp completed without producing the requested output format.".to_owned()
    })?;
    tokio::fs::hard_link(&clip, &destination)
        .await
        .map_err(|error| {
            format!("Could not create output without overwriting an existing file: {error}")
        })?;
    tokio::fs::remove_file(clip)
        .await
        .map_err(|error| error.to_string())?;
    Ok(destination.display().to_string())
}
struct RangeTrack {
    start: f32,
    end: f32,
    duration: f32,
}

const RANGE_HANDLE_RADIUS: f32 = 8.0;
const RANGE_TRACK_INSET: f32 = RANGE_HANDLE_RADIUS + 6.0;

fn range_track_span(width: f32) -> (f32, f32) {
    let inset = RANGE_TRACK_INSET.min(width / 3.0);
    (inset, (width - inset).max(inset + 1.0))
}

fn range_track_x(fraction: f32, width: f32) -> f32 {
    let (start, end) = range_track_span(width);
    start + fraction.clamp(0.0, 1.0) * (end - start)
}

fn range_track_fraction(x: f32, width: f32) -> f32 {
    let (start, end) = range_track_span(width);
    ((x - start) / (end - start)).clamp(0.0, 1.0)
}

fn release_range_drag(state: &mut Option<bool>) -> Option<canvas::Action<Message>> {
    state.take().map(|_| canvas::Action::capture())
}

impl canvas::Program<Message> for RangeTrack {
    type State = Option<bool>;

    fn update(
        &self,
        state: &mut Self::State,
        event: &iced::Event,
        bounds: iced::Rectangle,
        cursor: iced::mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        if matches!(
            event,
            iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                iced::mouse::Button::Left
            ))
        ) {
            return release_range_drag(state);
        }
        let position = cursor.position_in(bounds)?;
        let width = bounds.width.max(1.0);
        let start_x = range_track_x(self.start / self.duration, width);
        let end_x = range_track_x(self.end / self.duration, width);
        match event {
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) => {
                *state = Some((position.x - start_x).abs() <= (position.x - end_x).abs());
            }
            iced::Event::Mouse(iced::mouse::Event::CursorMoved { .. }) => {}
            _ => return None,
        }
        let selected_start = (*state)?;
        let seconds = (range_track_fraction(position.x, width) * self.duration).round() as u64;
        let message = if selected_start {
            Message::TimelineStart(seconds.min((self.end as u64).saturating_sub(1)))
        } else {
            Message::TimelineEnd(seconds.max(self.start as u64 + 1).min(self.duration as u64))
        };
        Some(canvas::Action::publish(message).and_capture())
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let width = bounds.width;
        let height = bounds.height;
        let origin = range_track_x(0.0, width);
        let finish = range_track_x(1.0, width);
        let track = canvas::Path::rounded_rectangle(
            iced::Point::new(origin, height / 2.0 - 3.0),
            iced::Size::new((finish - origin).max(1.0), 6.0),
            3.0.into(),
        );
        frame.fill(&track, TRACK);
        let left = range_track_x(self.start / self.duration.max(1.0), width);
        let right = range_track_x(self.end / self.duration.max(1.0), width).max(left);
        let selection = canvas::Path::rounded_rectangle(
            iced::Point::new(left, height / 2.0 - 3.0),
            iced::Size::new((right - left).max(2.0), 6.0),
            3.0.into(),
        );
        frame.fill(&selection, ACCENT);
        for x in [left, right] {
            let center = iced::Point::new(x, height / 2.0);
            frame.fill(&canvas::Path::circle(center, RANGE_HANDLE_RADIUS), ACCENT);
            frame.fill(
                &canvas::Path::circle(center, RANGE_HANDLE_RADIUS - 4.5),
                INPUT_BG,
            );
        }
        vec![frame.into_geometry()]
    }
}

fn surface_style() -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(SURFACE)),
        text_color: Some(TEXT),
        border: Border {
            color: LINE,
            width: 1.0,
            radius: 16.0.into(),
        },
        ..Default::default()
    }
}

fn inset_style() -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(INPUT_BG)),
        text_color: Some(TEXT),
        border: Border {
            color: LINE,
            width: 1.0,
            radius: 12.0.into(),
        },
        ..Default::default()
    }
}

fn notice_style() -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(SURFACE)),
        text_color: Some(TEXT),
        border: Border {
            color: ACCENT,
            width: 1.0,
            radius: 12.0.into(),
        },
        ..Default::default()
    }
}

fn sidebar_style() -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(SIDEBAR_BG)),
        text_color: Some(TEXT),
        ..Default::default()
    }
}

fn app_background() -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(APP_BG)),
        text_color: Some(TEXT),
        ..Default::default()
    }
}

fn separator() -> iced::widget::rule::Style {
    iced::widget::rule::Style {
        color: LINE,
        radius: 0.0.into(),
        fill_mode: iced::widget::rule::FillMode::Full,
        snap: true,
    }
}

fn primary_button(_theme: &Theme, status: button::Status) -> button::Style {
    let (background, text_color) = match status {
        button::Status::Hovered => (ACCENT_HOVER, INK),
        button::Status::Pressed => (ACCENT_PRESS, INK),
        button::Status::Disabled => (Color { a: 0.38, ..ACCENT }, Color { a: 0.5, ..INK }),
        button::Status::Active => (ACCENT, INK),
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color,
        border: Border {
            radius: 10.0.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        ..button::Style::default()
    }
}

fn secondary_button(_theme: &Theme, status: button::Status) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
        background: Some(Background::Color(if hovered {
            SURFACE_RAISED
        } else {
            SURFACE
        })),
        text_color: if matches!(status, button::Status::Disabled) {
            FAINT
        } else {
            TEXT
        },
        border: Border {
            radius: 10.0.into(),
            width: 1.0,
            color: if hovered { LINE_HOVER } else { LINE },
        },
        ..button::Style::default()
    }
}

fn danger_button(_theme: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered | button::Status::Pressed => {
                Color::from_rgba8(196, 84, 74, 0.16)
            }
            _ => Color::TRANSPARENT,
        })),
        text_color: DANGER,
        border: Border {
            radius: 10.0.into(),
            width: 1.0,
            color: Color::from_rgb8(0xC4, 0x54, 0x4A),
        },
        ..button::Style::default()
    }
}

fn quiet_button(_theme: &Theme, status: button::Status) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
        background: Some(Background::Color(if hovered {
            ACCENT_WASH
        } else {
            Color::TRANSPARENT
        })),
        text_color: if hovered { ACCENT_HOVER } else { ACCENT },
        border: Border {
            radius: 10.0.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

fn busy_button(_theme: &Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(ACCENT_WASH)),
        text_color: ACCENT,
        border: Border {
            radius: 10.0.into(),
            width: 1.0,
            color: ACCENT,
        },
        ..button::Style::default()
    }
}

fn nav_style(active: bool, status: button::Status) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    let background = if active {
        ACCENT_WASH
    } else if hovered {
        SURFACE_RAISED
    } else {
        Color::TRANSPARENT
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color: if active || hovered { TEXT } else { MUTED },
        border: Border {
            radius: 10.0.into(),
            width: if active { 1.0 } else { 0.0 },
            color: if active {
                Color::from_rgba8(212, 166, 86, 0.45)
            } else {
                Color::TRANSPARENT
            },
        },
        ..button::Style::default()
    }
}

fn field_style(_theme: &Theme, status: text_input::Status) -> text_input::Style {
    let border_color = match status {
        text_input::Status::Focused { .. } => ACCENT,
        text_input::Status::Hovered => LINE_HOVER,
        text_input::Status::Active | text_input::Status::Disabled => LINE,
    };
    text_input::Style {
        background: Background::Color(INPUT_BG),
        border: Border {
            radius: 10.0.into(),
            width: 1.0,
            color: border_color,
        },
        icon: MUTED,
        placeholder: FAINT,
        value: TEXT,
        selection: ACCENT_WASH,
    }
}

fn menu_field(_theme: &Theme, status: pick_list::Status) -> pick_list::Style {
    let open = matches!(
        status,
        pick_list::Status::Hovered | pick_list::Status::Opened { .. }
    );
    pick_list::Style {
        text_color: TEXT,
        placeholder_color: FAINT,
        handle_color: MUTED,
        background: Background::Color(INPUT_BG),
        border: Border {
            radius: 10.0.into(),
            width: 1.0,
            color: if open { ACCENT } else { LINE },
        },
    }
}

fn range_slider(_theme: &Theme, status: slider::Status) -> slider::Style {
    let accent = match status {
        slider::Status::Hovered => ACCENT_HOVER,
        slider::Status::Dragged => ACCENT_PRESS,
        slider::Status::Active => ACCENT,
    };
    slider::Style {
        rail: slider::Rail {
            backgrounds: (Background::Color(accent), Background::Color(TRACK)),
            width: 4.0,
            border: Border {
                radius: 2.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
        },
        handle: slider::Handle {
            shape: slider::HandleShape::Circle { radius: 7.0 },
            background: Background::Color(TEXT),
            border_color: accent,
            border_width: 2.0,
        },
    }
}

fn export_progress_style(_theme: &Theme) -> progress_bar::Style {
    progress_bar::Style {
        background: Background::Color(TRACK),
        bar: Background::Color(ACCENT),
        border: Border {
            radius: 4.0.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_youtube_video_and_short_urls() {
        assert!(validate_video_url("https://www.youtube.com/watch?v=abc").is_ok());
        assert!(validate_video_url("https://youtu.be/abc").is_ok());
        assert!(validate_video_url("https://example.com/watch?v=abc").is_err());
        assert!(validate_video_url("http://youtube.com/watch?v=abc").is_err());
    }
    #[test]
    fn language_preference_codes_round_trip_and_default_safely() {
        assert_eq!(language_code(Language::Portuguese), "pt");
        assert_eq!(
            parse_language(Some(language_code(Language::Portuguese))),
            Language::Portuguese
        );
        assert_eq!(language_code(Language::English), "en");
        assert_eq!(
            parse_language(Some(language_code(Language::English))),
            Language::English
        );
        assert_eq!(parse_language(Some("invalid")), Language::English);
        assert_eq!(parse_language(None), Language::English);
    }
    #[test]
    fn output_formats_map_to_expected_extensions() {
        assert_eq!(ExportFormat::Mp4.extension(), "mp4");
        assert_eq!(ExportFormat::Mp3.extension(), "mp3");
        assert_eq!(ExportFormat::M4a.extension(), "m4a");
    }
    #[test]
    fn builds_safe_format_specific_downloader_arguments() {
        let mp3 = build_download_args(
            "https://youtu.be/a?x=1&y=2",
            10,
            25,
            ExportFormat::Mp3,
            None,
            "clip",
        );
        assert!(mp3.windows(3).any(|w| w == ["-x", "--audio-format", "mp3"]));
        assert!(
            mp3.windows(2)
                .any(|w| w == ["--download-sections", "*00:00:10-00:00:25"])
        );
        assert_eq!(
            mp3.last().map(String::as_str),
            Some("https://youtu.be/a?x=1&y=2")
        );
        let m4a = build_download_args("url", 0, 5, ExportFormat::M4a, None, "clip");
        assert!(m4a.windows(3).any(|w| w == ["-x", "--audio-format", "m4a"]));
        let mp4 = build_download_args("url", 0, 5, ExportFormat::Mp4, Some("137"), "clip");
        assert!(mp4.windows(2).any(|w| w == ["-f", "137+bestaudio/best"]));
    }
    #[test]
    fn parses_download_progress_without_accepting_garbage() {
        assert_eq!(
            parse_progress_percent("[download]  42.6% of 10MiB"),
            Some(43)
        );
        assert_eq!(
            parse_progress_percent("[download] 100% of 10MiB"),
            Some(100)
        );
        assert_eq!(parse_progress_percent("CLIP_PROGRESS:42.6"), Some(43));
        assert_eq!(parse_progress_percent("CLIP_PROGRESS:100.0"), Some(100));
        assert_eq!(parse_progress_percent("CLIP_PROGRESS:nan"), None);
        assert_eq!(
            progress_details("CLIP_PROGRESS:42.6|1.5MiB/s|00:12"),
            Some((43, Some("1.5MiB/s".into()), Some("00:12".into())))
        );
        assert_eq!(
            progress_details("CLIP_PROGRESS:5|NA|NA"),
            Some((5, None, None))
        );
    }
    #[test]
    fn idle_timeline_does_not_capture_unrelated_button_releases() {
        let mut idle = None;
        assert!(release_range_drag(&mut idle).is_none());
        assert_eq!(idle, None);

        let mut dragging = Some(true);
        assert!(release_range_drag(&mut dragging).is_some());
        assert_eq!(dragging, None);
    }
    #[test]
    fn parses_ffmpeg_section_progress_and_processing_eta() {
        assert_eq!(
            parse_ffmpeg_progress(
                "frame=50 fps=25 time=00:01:30.00 bitrate=800kbits/s speed=2.00x",
                180
            ),
            Some((50, Some("FFmpeg 2.00x".into()), Some("00:00:45".into())))
        );
        assert_eq!(
            parse_ffmpeg_progress("frame=1 time=00:00:03.00 speed=1.00x", 2),
            Some((100, Some("FFmpeg 1.00x".into()), Some("00:00:00".into())))
        );
        assert_eq!(parse_ffmpeg_progress("time=N/A speed=N/A", 180), None);
        assert_eq!(parse_ffmpeg_progress("time=00:00:10.0 speed=1x", 0), None);
    }
    #[test]
    fn records_ffmpeg_progress_for_live_export_state() {
        let progress = DownloadProgress::default();
        let mut error_tail = String::new();
        record_export_output(
            "frame=150 time=00:00:05.00 speed=2.00x",
            10,
            &progress,
            &mut error_tail,
        );
        assert_eq!(progress.percent.load(Ordering::Relaxed), 50);
        let details = progress.details.lock().unwrap();
        assert_eq!(details.speed.as_deref(), Some("FFmpeg 2.00x"));
        assert_eq!(details.eta.as_deref(), Some("00:00:03"));
        assert!(error_tail.contains("time=00:00:05.00"));
    }
    #[test]
    fn time_conversion_and_range_boundaries() {
        assert_eq!(parse_time("01:02:03"), Some(3723));
        assert_eq!(format_time(3723), "01:02:03");
        assert_eq!(parse_time("00:60:00"), None);
        assert_eq!(parse_time("00:00:60"), None);
        assert_eq!(parse_time("1:2:3:4"), None);
        assert_eq!(parse_range("00:00:00", "00:00:10", 10).unwrap(), (0, 10));
        assert!(parse_range("00:00:10", "00:00:10", 10).is_err());
        assert!(parse_range("00:00:00", "00:00:11", 10).is_err());
    }
    #[test]
    fn filename_stem_removes_path_and_control_characters() {
        assert_eq!(safe_file_stem("../../My: clip?"), "My- clip");
        assert_eq!(safe_file_stem(""), "clip");
        assert!(safe_file_stem(&"x".repeat(100)).len() <= 80);
    }
    #[test]
    fn default_destination_uses_an_absolute_downloads_path() {
        assert_eq!(
            default_destination_in(std::path::Path::new("/home/testuser"), "My- clip.mp4"),
            PathBuf::from("/home/testuser/Downloads/My- clip.mp4")
        );
    }
    #[test]
    fn metadata_parser_reads_fixture() {
        let parsed: YtInfo =
            serde_json::from_str(include_str!("../tests/fixtures/video_metadata.json")).unwrap();
        assert_eq!(parsed.title, "Sample authorized clip");
        assert_eq!(parsed.channel.as_deref(), Some("Example Creator"));
        assert_eq!(parsed.duration, Some(3723.5));
        assert_eq!(
            parsed.thumbnail.as_deref(),
            Some("https://i.ytimg.com/vi/example/hqdefault.jpg")
        );
        assert_eq!(parsed.formats.len(), 2);
        assert_eq!(parsed.formats[1].height, Some(1080));
    }
    #[test]
    fn quality_choices_are_unique_friendly_and_highest_first() {
        let parsed: YtInfo =
            serde_json::from_str(include_str!("../tests/fixtures/video_metadata.json")).unwrap();
        let choices = format_choices(parsed.formats);
        assert_eq!(quality_label(1080), "1080p · Full HD");
        assert_eq!(choices[0].label, "1080p · Full HD");
        assert_eq!(choices[1].label, "360p");
        assert_eq!(choices.len(), 2);
    }

    #[test]
    fn range_handles_stay_inside_the_track() {
        let width = 480.0;
        let start = range_track_x(0.0, width);
        let end = range_track_x(1.0, width);
        assert!(start >= RANGE_HANDLE_RADIUS);
        assert!(end <= width - RANGE_HANDLE_RADIUS);
        assert!((range_track_fraction(start, width)).abs() < 0.001);
        assert!((range_track_fraction(end, width) - 1.0).abs() < 0.001);
    }

    #[test]
    fn preview_downloads_only_the_selected_section() {
        let args = build_preview_download_args("https://youtu.be/a", 10, 25, "preview.%(ext)s");
        assert!(
            args.windows(2)
                .any(|w| w == ["--download-sections", "*00:00:10-00:00:25"])
        );
        assert!(!args.iter().any(|arg| arg == "--force-keyframes-at-cuts"));
        assert_eq!(args.last().map(String::as_str), Some("https://youtu.be/a"));

        let vlc = vlc_local_preview_args("/tmp/preview.mp4");
        assert!(vlc.iter().any(|arg| arg == "--play-and-exit"));
        assert!(!vlc.iter().any(|arg| arg.starts_with("--start-time=")));
        assert!(!vlc.iter().any(|arg| arg.starts_with("--stop-time=")));
        assert_eq!(vlc.last().map(String::as_str), Some("/tmp/preview.mp4"));

        let ffplay = ffplay_local_preview_args("/tmp/preview.mp4");
        assert!(!ffplay.iter().any(|arg| arg == "-ss"));
        assert!(
            ffplay
                .windows(2)
                .any(|pair| pair == ["-i", "/tmp/preview.mp4"])
        );
    }

    #[test]
    fn export_list_removes_one_entry_and_keeps_the_rest() {
        let mut history = vec![
            "/tmp/a.mp4".into(),
            "/tmp/b.mp4".into(),
            "/tmp/a.mp4".into(),
        ];
        drop_export(&mut history, "/tmp/a.mp4");
        assert_eq!(history, vec!["/tmp/b.mp4".to_owned()]);
    }

    #[test]
    fn preview_volume_ids_match_the_player_only() {
        let wpctl = "\
 ├─ Streams:
 │      63. ffplay
 │      70. Firefox
 ├─ Sources:
";
        assert_eq!(wpctl_preview_ids(wpctl), vec!["63".to_owned()]);
        let pactl = "\
Sink Input #12
    application.process.binary = \"firefox\"
Sink Input #18
    application.name = \"ffplay\"
";
        assert_eq!(pactl_preview_ids(pactl), vec!["18".to_owned()]);
    }

    #[test]
    fn application_icon_is_the_clipforge_mark() {
        let (width, height, pixels) = decode_icon(ICON_64).expect("icon");
        assert_eq!((width, height), (64, 64));
        assert_eq!(pixels.len(), 64 * 64 * 4);
        assert!(iced::window::icon::from_rgba(pixels.clone(), width, height).is_ok());
        let gold = pixels
            .chunks(4)
            .any(|pixel| pixel[3] > 200 && pixel[0] > pixel[2] + 30);
        assert!(gold);
    }

    #[test]
    fn vlc_preview_plays_a_local_clip_with_qt_controls() {
        let args = vlc_local_preview_args("C:/temp/preview.mp4");
        assert!(args.iter().any(|arg| arg == "--intf=qt"));
        assert!(args.iter().any(|arg| arg == "--play-and-exit"));
        assert!(args.iter().any(|arg| arg.starts_with("--meta-title=")));
        assert_eq!(args.last().map(String::as_str), Some("C:/temp/preview.mp4"));
        let mpv = mpv_local_preview_args("C:/temp/preview.mp4");
        assert!(mpv.iter().any(|arg| arg == "--force-window=yes"));
        assert_eq!(mpv.last().map(String::as_str), Some("C:/temp/preview.mp4"));
    }

    #[test]
    fn reports_missing_required_tools_and_keeps_external_errors_short() {
        let missing = Toolchain {
            ytdlp: None,
            ffmpeg: None,
            deno: None,
            vlc: None,
            mpv: None,
        };
        assert!(!missing.ready());
        assert_eq!(missing.missing_required(), ["yt-dlp", "FFmpeg"]);
        assert!(
            export_blocker(&missing, Language::Portuguese)
                .unwrap()
                .contains("yt-dlp")
        );
        let ready = Toolchain {
            ytdlp: Some(PathBuf::from("yt-dlp")),
            ffmpeg: Some(PathBuf::from("ffmpeg")),
            deno: None,
            ..missing
        };
        assert!(ready.ready());
        assert!(export_blocker(&ready, Language::English).is_none());
        assert_eq!(
            summarize_external_error("WARNING: old\nERROR: Sign in to confirm you're not a bot"),
            "ERROR: Sign in to confirm you're not a bot"
        );
        assert!(summarize_external_error(&"x".repeat(300)).chars().count() <= 221);
        assert!(
            metadata_notice("ERROR: Video unavailable", Language::Portuguese)
                .starts_with("Falha ao carregar metadados.")
        );
    }

    #[test]
    fn finds_an_executable_next_to_the_app() {
        let dir = tempfile::tempdir().unwrap();
        let file_name = executable_names("yt-dlp").remove(0);
        std::fs::write(dir.path().join(&file_name), b"tool").unwrap();
        let found = find_named_tool(&[dir.path().to_path_buf()], "yt-dlp").unwrap();
        assert_eq!(found, dir.path().join(file_name));
        assert!(find_named_tool(&[dir.path().to_path_buf()], "ffmpeg").is_none());
    }

    #[test]
    fn install_commands_are_ready_to_paste() {
        let commands = install_commands_text(Language::English);
        assert!(commands.contains("yt-dlp.yt-dlp") || commands.contains("yt-dlp"));
        assert!(
            commands.contains("ffmpeg")
                || commands.contains("FFmpeg")
                || commands.contains("Gyan.FFmpeg")
        );
        let missing = Toolchain {
            ytdlp: None,
            ffmpeg: None,
            deno: None,
            vlc: None,
            mpv: None,
        };
        assert_eq!(
            missing.packages_to_install(),
            [
                WingetPackage::YtDlp,
                WingetPackage::Ffmpeg,
                WingetPackage::Deno,
                WingetPackage::Vlc
            ]
        );
        assert!(winget_result_is_ok(0));
        assert!(winget_result_is_ok(0x8A15_002B));
        assert!(winget_result_is_ok(0x8A15_0061));
        assert!(!winget_result_is_ok(0x8A15_0010));
        let progress = InstallProgress::new(3);
        progress.set_package(2, "Deno", Language::Portuguese);
        let summary = progress.summary(Language::Portuguese);
        assert!(summary.contains("Deno"));
        assert!(summary.contains("2/3"));
    }

    #[cfg(windows)]
    #[test]
    fn finds_tools_nested_under_winget_style_package_dirs() {
        let root = tempfile::tempdir().unwrap();
        let package = root.path().join("Gyan.FFmpeg_Microsoft.Winget.Source_test");
        let bin = package.join("ffmpeg-9.0.2-full_build").join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("ffmpeg.exe"), b"ffmpeg").unwrap();
        let ytdlp_pkg = root
            .path()
            .join("yt-dlp.yt-dlp_Microsoft.Winget.Source_test");
        std::fs::create_dir_all(&ytdlp_pkg).unwrap();
        std::fs::write(ytdlp_pkg.join("yt-dlp.exe"), b"ytdlp").unwrap();

        let mut dirs = Vec::new();
        push_unique_dir(&mut dirs, ytdlp_pkg.clone());
        collect_winget_bin_dirs(&package, &mut dirs, 0);
        assert!(
            find_named_tool(&dirs, "yt-dlp")
                .unwrap()
                .ends_with("yt-dlp.exe")
        );
        assert!(
            find_named_tool(&dirs, "ffmpeg")
                .unwrap()
                .ends_with("ffmpeg.exe")
        );
    }

    #[test]
    fn wayland_desktop_entry_matches_the_application_id() {
        let entry = desktop_entry("/usr/bin/clipforge", "/tmp/clipforge.png");
        assert!(entry.contains("StartupWMClass=clipforge\n"));
        assert!(entry.contains("Icon=/tmp/clipforge.png\n"));
        assert!(entry.contains("Exec=/usr/bin/clipforge\n"));
        assert_eq!(
            desktop_exec("/home/user/My Apps/clipforge"),
            "\"/home/user/My Apps/clipforge\""
        );
    }
}
