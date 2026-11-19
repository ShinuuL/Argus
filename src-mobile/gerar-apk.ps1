<#
.SYNOPSIS
  Gera o APK do Argus Celular (Tauri 2 + Android).

.DESCRIPTION
  Confere os requisitos, cria o projeto Android na primeira vez, garante o
  `usesCleartextTraffic` (sem ele o Android bloqueia o http:// da rede local),
  compila e mostra onde o arquivo ficou. Sem assinatura configurada o APK sai
  em debug, que instala por USB mas não serve para publicar; com
  `gen/android/key.properties` no lugar, sai assinado em release.

.PARAMETER Instalar
  Instala no aparelho conectado por USB depois de gerar (usa o adb).

.PARAMETER Aab
  Gera também o .aab (formato da Play Store) além do APK.

.PARAMETER Release
  Força o APK de release. Sem assinatura configurada ele sai sem assinar, e o
  Android recusa instalar: serve só para quem vai assinar depois.

.EXAMPLE
  .\gerar-apk.ps1
  .\gerar-apk.ps1 -Instalar
#>
[CmdletBinding()]
param(
    [switch]$Instalar,
    [switch]$Aab,
    [switch]$Release
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$raiz = Split-Path -Parent $MyInvocation.MyCommand.Path
$projeto = Split-Path -Parent $raiz

function Passo($texto) { Write-Host "`n==> $texto" -ForegroundColor Cyan }
function Aviso($texto) { Write-Host "    $texto" -ForegroundColor Yellow }

# ---------------------------------------------------------------- requisitos

Passo 'Conferindo os requisitos'

$faltando = @()
foreach ($v in 'JAVA_HOME', 'ANDROID_HOME', 'NDK_HOME') {
    $valor = [Environment]::GetEnvironmentVariable($v)
    if ([string]::IsNullOrWhiteSpace($valor)) { $faltando += $v }
    elseif (-not (Test-Path $valor)) { $faltando += "$v (aponta para uma pasta que não existe: $valor)" }
}
if ($faltando.Count -gt 0) {
    throw "Faltam variáveis de ambiente: $($faltando -join ', '). Instale o JDK 17 e o Android Studio (SDK + NDK) e defina-as. Veja src-mobile\README.md."
}

if (-not (Get-Command npm -ErrorAction SilentlyContinue)) {
    throw 'npm não encontrado no PATH. Instale o Node.js 20 ou mais novo.'
}
if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) {
    throw 'rustup não encontrado no PATH. Instale o Rust (stable, MSVC).'
}

$alvos = @('aarch64-linux-android', 'armv7-linux-androideabi', 'i686-linux-android', 'x86_64-linux-android')
$instalados = (rustup target list --installed) -split "`r?`n"
# O @(...) é necessário: sem ele, nenhum alvo faltando vira $null e o .Count
# falha com Set-StrictMode.
$faltamAlvos = @($alvos | Where-Object { $instalados -notcontains $_ })
if ($faltamAlvos.Count -gt 0) {
    Aviso "Instalando os alvos Rust do Android: $($faltamAlvos -join ', ')"
    rustup target add @faltamAlvos
    if ($LASTEXITCODE -ne 0) { throw 'Não foi possível instalar os alvos do Android.' }
}

if (-not (Test-Path (Join-Path $projeto 'node_modules'))) {
    Passo 'Instalando as dependências do npm (a CLI do Tauri vem daqui)'
    Push-Location $projeto
    try {
        npm install --no-audit --no-fund
        if ($LASTEXITCODE -ne 0) { throw 'npm install falhou.' }
    } finally { Pop-Location }
}

# ------------------------------------------------------------ projeto Android

Push-Location $raiz
try {
    $gen = Join-Path $raiz 'gen\android'
    if (-not (Test-Path $gen)) {
        Passo 'Criando o projeto Android (primeira vez)'
        npx tauri android init
        if ($LASTEXITCODE -ne 0) { throw 'tauri android init falhou.' }
    }
    else {
        Passo 'Projeto Android já existe (gen\android)'
    }

    # O Android bloqueia http:// desde o Android 9: sem isto o aplicativo só
    # conecta em servidores https, e o Argus da rede local não é https.
    Passo 'Garantindo o usesCleartextTraffic no AndroidManifest'
    $manifesto = Join-Path $gen 'app\src\main\AndroidManifest.xml'
    if (-not (Test-Path $manifesto)) { throw "AndroidManifest.xml não encontrado em $manifesto." }
    $xml = Get-Content $manifesto -Raw
    if ($xml -match 'usesCleartextTraffic') {
        Aviso 'Já estava lá.'
    }
    else {
        $novo = $xml -replace '(<application\b)', '$1
        android:usesCleartextTraffic="true"'
        if ($novo -eq $xml) { throw 'Não achei a tag <application> para acrescentar o usesCleartextTraffic. Edite o arquivo à mão.' }
        Set-Content -Path $manifesto -Value $novo -Encoding utf8
        Aviso 'Acrescentado.'
    }

    # ------------------------------------------------------------- assinatura

    # O release sem `key.properties` sai SEM ASSINATURA, e o Android recusa
    # instalar. Então, sem assinatura configurada, o padrão é o debug — que já
    # vem assinado com a chave de depuração e instala por USB.
    $chaves = Join-Path $gen 'key.properties'
    $assinado = Test-Path $chaves
    $debug = -not ($assinado -or $Release)
    if ($assinado) {
        Passo 'Assinatura de release encontrada (gen\android\key.properties)'
    }
    elseif ($Release) {
        Passo 'Release pedido sem assinatura configurada'
        Aviso 'O APK vai sair SEM ASSINAR: o aparelho recusa instalar até ele ser assinado.'
    }
    else {
        Passo 'Sem assinatura configurada: gerando em debug'
        Aviso 'Instala por USB, mas não serve para publicar. Para assinar, veja "Publicar" no README.'
    }

    # ----------------------------------------------------------------- compilar

    Passo ('Compilando (' + $(if ($debug) { 'debug' } else { 'release' }) + ')')
    $argumentos = @('tauri', 'android', 'build')
    if (-not $Aab) { $argumentos += '--apk' }
    if ($debug) { $argumentos += '--debug' }
    npx @argumentos
    if ($LASTEXITCODE -ne 0) { throw 'A compilação falhou. A saída acima diz o motivo.' }

    $saidas = Join-Path $gen 'app\build\outputs'
    $arquivos = @(Get-ChildItem -Path $saidas -Recurse -Include *.apk, *.aab -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending)
    if ($arquivos.Count -eq 0) { throw "Compilou, mas não achei nenhum .apk em $saidas." }

    Passo 'Pronto'
    foreach ($a in $arquivos | Select-Object -First 4) {
        Write-Host ("    {0}  ({1:N1} MB)" -f $a.FullName, ($a.Length / 1MB))
    }

    if ($Instalar) {
        $apk = @($arquivos | Where-Object { $_.Extension -eq '.apk' }) | Select-Object -First 1
        if ($null -eq $apk) { throw 'Nenhum .apk para instalar (gere sem -Aab).' }
        $adb = Join-Path $env:ANDROID_HOME 'platform-tools\adb.exe'
        if (-not (Test-Path $adb)) { throw "adb não encontrado em $adb." }
        Passo "Instalando no aparelho: $($apk.Name)"
        & $adb install -r $apk.FullName
        if ($LASTEXITCODE -ne 0) { throw 'adb install falhou. Confira se a depuração USB está ligada e autorizada.' }
    }
}
finally {
    Pop-Location
}
