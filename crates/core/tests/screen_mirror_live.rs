//! Teste com o Screen-Mirror de verdade (precisa de Node e de `ARGUS_SCREEN_MIRROR`):
//! `cargo test -p argus-core --test screen_mirror_live -- --ignored --nocapture`.
use argus_core::devices::ScreenMirror;
use std::net::TcpStream;

#[test]
#[ignore]
fn starts_lists_and_stops() {
    let sm = ScreenMirror::new(None);
    let st = sm.start(None).expect("start");
    let url = st.url.clone().expect("url");
    println!("no ar em {url} (pid {:?}); aparelhos: {:?}", st.pid, sm.devices().expect("devices"));
    assert_eq!(sm.start(None).unwrap().url, Some(url.clone()), "segunda chamada reaproveita o processo");
    sm.stop();
    assert_eq!(sm.status(None).url, None);
    let addr = url.trim_start_matches("http://");
    assert!(TcpStream::connect(addr).is_err(), "servidor ainda responde depois do stop");
}
