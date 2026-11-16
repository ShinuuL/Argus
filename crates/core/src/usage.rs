//! Uso de CPU, memória e GPU por dono (sessão/terminal): soma da árvore de processos
//! rastreada em `procs` (agente, filhos e servidores vinculados).
//!
//! - CPU/memória: `sysinfo` com um `System` persistente (CPU exige duas amostras; a
//!   primeira chamada devolve 0 % e as seguintes a média desde a anterior).
//! - GPU (só Windows): classe WMI `Win32_PerfFormattedData_GPUPerformanceCounters_GPUEngine`
//!   (mesmos dados do contador PDH `\GPU Engine(*)\Utilization Percentage`, mas com nome
//!   não traduzido). Falha silenciosa: `gpu = None`.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use serde::Serialize;

use crate::procs::{self, Owner};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OwnerUsage {
    pub owner: Owner,
    /// % do total da máquina (0–100).
    pub cpu: f32,
    /// Memória residente em bytes.
    pub memory: u64,
    /// % de GPU (motor mais ocupado), se disponível.
    pub gpu: Option<f32>,
    pub processes: usize,
}

/// Amostra de um processo: (cpu % por núcleo somado, bytes).
pub type ProcSample = (f32, u64);

/// PID de uma instância `pid_1234_luid_..._engtype_3D`.
pub fn gpu_instance_pid(name: &str) -> Option<u32> {
    let rest = name.trim().strip_prefix("pid_")?;
    rest.split('_').next()?.parse().ok()
}

fn gpu_engtype(name: &str) -> &str {
    name.rsplit_once("engtype_").map(|(_, t)| t.trim()).unwrap_or("")
}

/// Parse de linhas `instância<TAB>percentual`. Por PID: soma por tipo de motor e
/// fica o maior tipo (como o Gerenciador de Tarefas).
pub fn parse_gpu_lines(out: &str) -> HashMap<u32, f32> {
    let mut by: HashMap<(u32, String), f32> = HashMap::new();
    for l in out.lines() {
        let Some((name, val)) = l.split_once('\t') else { continue };
        let (Some(pid), Ok(v)) = (gpu_instance_pid(name), val.trim().replace(',', ".").parse::<f32>()) else {
            continue;
        };
        *by.entry((pid, gpu_engtype(name).to_string())).or_default() += v;
    }
    let mut out: HashMap<u32, f32> = HashMap::new();
    for ((pid, _), v) in by {
        let e = out.entry(pid).or_default();
        *e = e.max(v);
    }
    out
}

/// Soma da árvore. `cpus` normaliza o % por núcleo do sysinfo para % da máquina.
pub fn aggregate(
    owner: Owner,
    pids: &[u32],
    samples: &HashMap<u32, ProcSample>,
    gpu: Option<&HashMap<u32, f32>>,
    cpus: usize,
) -> OwnerUsage {
    let mut cpu = 0.0;
    let mut memory = 0;
    let mut n = 0;
    for p in pids {
        if let Some((c, m)) = samples.get(p) {
            cpu += c;
            memory += m;
            n += 1;
        }
    }
    let gpu = gpu.map(|g| pids.iter().filter_map(|p| g.get(p)).sum::<f32>().min(100.0));
    OwnerUsage { owner, cpu: (cpu / cpus.max(1) as f32).min(100.0), memory, gpu, processes: n }
}

#[cfg(windows)]
fn gpu_sample() -> Option<HashMap<u32, f32>> {
    let script = "Get-CimInstance Win32_PerfFormattedData_GPUPerformanceCounters_GPUEngine -ErrorAction Stop | \
                  ForEach-Object { \"$($_.Name)`t$($_.UtilizationPercentage)\" }";
    let (ok, out) = crate::agents::provider::run_capture_stdout(
        std::path::Path::new("powershell"),
        &["-NoProfile", "-NonInteractive", "-Command", script],
    )?;
    ok.then(|| parse_gpu_lines(&out))
}

#[cfg(not(windows))]
fn gpu_sample() -> Option<HashMap<u32, f32>> {
    None
}

struct Sampler {
    sys: Option<sysinfo::System>,
}

fn sampler() -> &'static Mutex<Sampler> {
    static S: OnceLock<Mutex<Sampler>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Sampler { sys: sysinfo::System::new().ok() }))
}

/// Uso de cada dono rastreado. Bloqueante (GPU chama o PowerShell): rode fora da UI.
pub fn usage_all(with_gpu: bool) -> Vec<OwnerUsage> {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate};
    let owned = procs::tracker().owned();
    let mut groups: HashMap<Owner, Vec<u32>> = HashMap::new();
    for p in &owned {
        groups.entry(p.owner).or_default().push(p.pid);
    }
    let samples: HashMap<u32, ProcSample> = {
        let mut g = sampler().lock().unwrap_or_else(|e| e.into_inner());
        let Some(sys) = g.sys.as_mut() else { return Vec::new() };
        let pids: Vec<sysinfo::Pid> = owned.iter().map(|p| sysinfo::Pid::from_u32(p.pid)).collect();
        sys.refresh_processes_specifics(
            ProcessesToUpdate::Some(&pids),
            true,
            ProcessRefreshKind::nothing().with_cpu().with_memory(),
        );
        pids.iter()
            .filter_map(|p| sys.process(*p).map(|x| (p.as_u32(), (x.cpu_usage(), x.memory()))))
            .collect()
    };
    let gpu = if with_gpu { gpu_sample() } else { None };
    let cpus = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    let mut v: Vec<OwnerUsage> =
        groups.into_iter().map(|(o, pids)| aggregate(o, &pids, &samples, gpu.as_ref(), cpus)).collect();
    v.sort_by_key(|u| format!("{:?}", u.owner));
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pid_da_instancia_gpu() {
        assert_eq!(gpu_instance_pid("pid_1234_luid_0x00000000_0x0000D1B5_phys_0_eng_0_engtype_3D"), Some(1234));
        assert_eq!(gpu_instance_pid("luid_0x0_phys_0"), None);
        assert_eq!(gpu_instance_pid("pid_abc_luid"), None);
    }

    #[test]
    fn gpu_soma_por_motor_e_pega_o_maior() {
        let out = "pid_10_luid_0x0_phys_0_eng_0_engtype_3D\t20\n\
                   pid_10_luid_0x0_phys_0_eng_1_engtype_3D\t15\n\
                   pid_10_luid_0x0_phys_0_eng_2_engtype_VideoDecode\t30\n\
                   pid_11_luid_0x0_phys_0_eng_0_engtype_Copy\t2,5\n\
                   lixo\n";
        let g = parse_gpu_lines(out);
        assert_eq!(g[&10], 35.0);
        assert_eq!(g[&11], 2.5);
    }

    #[test]
    fn soma_da_arvore() {
        let s: HashMap<u32, ProcSample> = [(1, (50.0, 100)), (2, (30.0, 50)), (9, (99.0, 1))].into();
        let g: HashMap<u32, f32> = [(2, 12.0)].into();
        let u = aggregate(Owner::Session(7), &[1, 2, 3], &s, Some(&g), 4);
        assert_eq!(u.cpu, 20.0);
        assert_eq!(u.memory, 150);
        assert_eq!(u.processes, 2);
        assert_eq!(u.gpu, Some(12.0));
        assert_eq!(aggregate(Owner::Session(7), &[1], &s, None, 4).gpu, None);
    }
}
