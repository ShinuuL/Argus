//! Instalação de APK pelo adb real, sem instalar nada: o serial não existe, então o
//! adb recusa depois de achar o APK e o adb. Precisa de `ARGUS_TEST_APK_PROJECT` (um
//! projeto com APK gerado):
//! `cargo test -p argus-core --test apk_install_live -- --ignored --nocapture`.
use argus_core::devices::{adb_path, find_apks, ScreenMirror};
use std::path::PathBuf;

#[test]
#[ignore]
fn reaches_adb_with_project_apk() {
    let project = PathBuf::from(std::env::var("ARGUS_TEST_APK_PROJECT").expect("ARGUS_TEST_APK_PROJECT"));
    let apks = find_apks(&project);
    println!("APKs: {:?}", apks.iter().map(|a| &a.label).collect::<Vec<_>>());
    let apk = apks.first().expect("o projeto não tem APK gerado");
    println!("adb: {:?}", adb_path(None));
    let err = ScreenMirror::new(None).install_apk(None, &project, &apk.path, "serial-que-nao-existe").unwrap_err().to_string();
    println!("erro esperado: {err}");
    assert!(err.to_lowercase().contains("not found") || err.contains("serial-que-nao-existe"), "{err}");
}
