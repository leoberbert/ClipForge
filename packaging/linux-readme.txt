ClipForge for Linux x86_64

Rust is not required. This AppImage needs glibc from Ubuntu 24.04 or a
newer distribution, such as current Fedora. If it does not start, run:

    ./ClipForge-*.AppImage --appimage-extract-and-run

Install these programs from your distribution before loading or exporting
a video:

- yt-dlp
- FFmpeg, which also provides ffprobe
- VLC, MPV, or FFplay, for the preview window

Deno is recommended. Current yt-dlp releases use it to read YouTube pages;
without it, fewer formats may be available.

The first launch registers the ClipForge icon for Wayland and X11.
yt-dlp and FFmpeg stay outside this AppImage so the distribution can
update them.

---

ClipForge para Linux x86_64

Esta versão não exige Rust. O AppImage precisa da glibc do Ubuntu 24.04
ou de uma distribuição mais nova, como o Fedora atual. Se ele não abrir,
execute:

    ./ClipForge-*.AppImage --appimage-extract-and-run

Antes de carregar ou exportar um vídeo, instale pelo sistema:

- yt-dlp
- FFmpeg, que também traz o ffprobe
- VLC, MPV ou FFplay, para a janela de pré-visualização

O Deno é recomendado. As versões atuais do yt-dlp usam ele para ler as
páginas do YouTube; sem ele, pode haver menos formatos disponíveis.

A primeira execução registra o ícone do ClipForge no Wayland e no X11.
yt-dlp e FFmpeg ficam de fora deste AppImage para serem atualizados pelo
sistema.
