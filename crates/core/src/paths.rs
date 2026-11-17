//! Nome da pasta de dados do Argus.
//!
//! O app instalado usa `argus`. A cópia de desenvolvimento (`npm run app:dev`
//! e qualquer build de depuração) usa `argus-dev`, para que testes e
//! recompilações nunca toquem nas sessões de verdade do usuário. O desktop
//! escolhe o perfil ao iniciar, pela variável `ARGUS_PROFILE`.

/// Variável que escolhe o perfil de dados: `dev` usa a pasta `argus-dev`.
pub const PROFILE_VAR: &str = "ARGUS_PROFILE";

/// Nome da pasta de dados (dentro de `%APPDATA%` ou `~/.config`).
pub fn dir_name() -> &'static str {
    dir_name_for(std::env::var(PROFILE_VAR).ok().as_deref())
}

fn dir_name_for(profile: Option<&str>) -> &'static str {
    match profile.map(str::trim) {
        Some(p) if p.eq_ignore_ascii_case("dev") => "argus-dev",
        _ => "argus",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_picks_the_folder() {
        assert_eq!(dir_name_for(None), "argus");
        assert_eq!(dir_name_for(Some("")), "argus");
        assert_eq!(dir_name_for(Some("outro")), "argus");
        assert_eq!(dir_name_for(Some("dev")), "argus-dev");
        assert_eq!(dir_name_for(Some(" DEV ")), "argus-dev");
    }
}
