# Argus Celular (APK Android, Tauri 2)

Aplicativo do aparelho. A casca é Tauri 2 — a mesma pilha do Argus do computador
(`src-tauri`). A única tela que vem dentro do APK é a de **pareamento**
(`web/index.html`): endereço e token do computador. Depois de parear, a webview
vai para o próprio servidor do Argus e a interface é a mesma que já roda no
navegador do celular (`crates/core/assets/remote`), inclusive o anexo de imagens
da galeria pelo seletor de arquivos do Android.

Por que assim: a interface do celular fica num lugar só. Quando o Argus do
computador é atualizado, o aplicativo acompanha sem gerar outro APK.

## Requisitos (uma vez, na máquina que vai gerar o APK)

- JDK 17, Android Studio com **SDK**, **NDK** e **Platform-Tools**.
- Variáveis `JAVA_HOME`, `ANDROID_HOME` e `NDK_HOME`.
- Node.js 20+ e Rust (stable, MSVC). Os alvos Android do Rust o script instala
  sozinho, se faltarem.

## Gerar

```powershell
cd src-mobile
.\gerar-apk.ps1                 # confere tudo, compila e mostra onde ficou o APK
.\gerar-apk.ps1 -Instalar       # instala no aparelho ligado por USB
.\gerar-apk.ps1 -Aab            # gera também o .aab (Play Store)
```

O script faz, nesta ordem: confere requisitos e variáveis, instala os alvos Rust
que faltarem, roda `tauri android init` na primeira vez, garante o
`android:usesCleartextTraffic="true"` no `AndroidManifest.xml` (sem isso o
Android bloqueia o `http://` da rede local e o aplicativo só conecta em
servidores `https://`), compila e lista os arquivos gerados.

À mão, se preferir — os mesmos passos estão nos scripts do `package.json` da
raiz (`npm run celular:init | celular:dev | celular:apk`):

```powershell
cd src-mobile
npx tauri android init
npx tauri android dev       # roda no aparelho, com recarga
npx tauri android build     # APK/AAB em gen/android/app/build/outputs
```

## Publicar

Sem assinatura o APK sai em **debug**: instala por USB, mas não serve para
distribuir. Para sair assinado em **release**:

1. Crie o keystore (uma vez, e guarde num cofre de senhas — perder significa não
   conseguir mais atualizar o aplicativo publicado):

   ```powershell
   & "$env:JAVA_HOME\bin\keytool" -genkeypair -v -keystore argus-celular.jks `
     -keyalg RSA -keysize 2048 -validity 10000 -alias argus
   ```

2. Copie `key.properties.exemplo` para `gen/android/key.properties` e preencha
   com o caminho do `.jks` e as senhas. **Não versione** nenhum dos dois.

3. Em `gen/android/app/build.gradle.kts`, use o keystore na variante de release
   (o `tauri android init` já deixa o bloco preparado; se não deixar, acrescente
   o `signingConfigs` lendo o `key.properties` e aponte `buildTypes.release`
   para ele).

4. Rode `.\gerar-apk.ps1` de novo: ele avisa quando acha o `key.properties`.

Distribuição:

- **Direto (o caso de uso daqui):** anexe o `.apk` à release do GitHub, junto do
  `Argus_X.Y.Z_x64-setup.exe` do computador, e instale no aparelho pelo link, por
  `adb install -r`, ou pela aba **Dispositivos** do próprio Argus.
- **Play Store:** use o `.aab` (`-Aab`). O aplicativo pede acesso à rede local e
  a conexão é com o computador do próprio usuário — vale descrever isso na ficha
  da loja para não ser recusado.

A versão fica no `tauri.conf.json` daqui e deve andar junto com as outras três do
projeto (ver `CHANGELOG.md`).

## O que o aplicativo guarda

Só o endereço pareado, no `localStorage` da tela de pareamento. O token fica no
cookie do servidor do Argus, como no navegador. Nada vai para a internet: a
conversa é direta com o computador, pela rede local ou por VPN.
