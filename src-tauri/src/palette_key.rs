//! Ctrl+K em qualquer frame da janela.
//!
//! O atalho da paleta é ouvido no JavaScript da página, mas um iframe de outra
//! origem (o painel do Screen-Mirror na aba de espelhamento) fica com as teclas
//! e a página nunca as vê. O WebView2 avisa as teclas de atalho de todos os
//! frames em `AcceleratorKeyPressed`: o Ctrl+K é tratado aqui e vira o evento
//! `argus://palette` para a tela.

#[cfg(windows)]
pub fn hook(win: &tauri::WebviewWindow) {
    use tauri::{Emitter, Manager};
    let app = win.app_handle().clone();
    let _ = win.with_webview(move |wv| unsafe {
        use webview2_com::AcceleratorKeyPressedEventHandler;
        use webview2_com::Microsoft::Web::WebView2::Win32::*;
        use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL, VK_MENU, VK_SHIFT};
        let ctrl = wv.controller();
        let handler = AcceleratorKeyPressedEventHandler::create(Box::new(move |_, args| {
            let Some(args) = args else { return Ok(()) };
            let mut kind = COREWEBVIEW2_KEY_EVENT_KIND::default();
            args.KeyEventKind(&mut kind)?;
            let mut vk = 0u32;
            args.VirtualKey(&mut vk)?;
            let down = |k: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY| GetKeyState(k.0 as i32) < 0;
            if kind == COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN && vk == u32::from(b'K') && down(VK_CONTROL) && !down(VK_SHIFT) && !down(VK_MENU) {
                args.SetHandled(true)?;
                let _ = app.emit("argus://palette", ());
            }
            Ok(())
        }));
        let mut token = Default::default();
        if let Err(e) = ctrl.add_AcceleratorKeyPressed(&handler, &mut token) {
            eprintln!("atalho da paleta: {e}");
        }
    });
}

#[cfg(not(windows))]
pub fn hook(_win: &tauri::WebviewWindow) {}
