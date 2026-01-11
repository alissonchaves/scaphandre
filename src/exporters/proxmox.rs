use crate::exporters::Exporter;
use crate::sensors::Topology;
use crate::sensors::{utils::ProcessRecord, Sensor};
use std::{fs, io, thread, time};

/// An Exporter that extracts power consumption data of running
/// Qemu/KVM virtual machines on the host and stores those data
/// as folders and files that can be mounted inside the guest VMs.
pub struct ProxmoxExporter {
    topology: Topology,
}

impl Exporter for ProxmoxExporter {
    fn run(&mut self) {
        info!("Starting proxmox exporter");
        let path = "/tmp/scaphandre";

        // Ensure base directory exists
        if fs::read_dir(path).is_err() {
            match fs::create_dir_all(path) {
                Ok(_) => info!("Base directory {} created.", path),
                Err(e) => {
                    error!("Could not create {}: {}", path, e);
                    return;
                }
            }
        }

        let cleaner_step = 120;
        let mut timer = time::Duration::from_secs(cleaner_step);
        loop {
            self.iterate(String::from(path));
            let step = time::Duration::from_secs(1);
            thread::sleep(step);
            if timer > step {
                timer -= step;
            } else {
                self.topology
                    .proc_tracker
                    .clean_terminated_process_records_vectors();
                timer = time::Duration::from_secs(cleaner_step);
            }
        }
    }

    fn kind(&self) -> &str {
        "proxmox"
    }
}

impl ProxmoxExporter {
    pub fn new(sensor: &dyn Sensor) -> ProxmoxExporter {
        let topology = sensor
            .get_topology()
            .expect("sensor topology should be available");
        ProxmoxExporter { topology }
    }

    pub fn iterate(&mut self, path: String) {
        trace!("path: {}", path);

        self.topology.refresh();
        if let Some(topo_energy) = self.topology.get_records_diff_power_microwatts() {
            let processes = self.topology.proc_tracker.get_alive_processes();
            let qemu_processes = ProxmoxExporter::filter_qemu_vm_processes(&processes);
            for qp in qemu_processes {
                if qp.len() > 2 {
                    let last = qp.first().unwrap();
                    let vm_name = ProxmoxExporter::get_vm_name_from_cmdline(
                        &last.process.cmdline(&self.topology.proc_tracker).unwrap(),
                    );
                    let first_domain_path = format!("{path}/{vm_name}/intel-rapl:0:0");
                    if fs::read_dir(&first_domain_path).is_err() {
                        match fs::create_dir_all(&first_domain_path) {
                            Ok(_) => info!("Created directory {}", &first_domain_path),
                            Err(error) => panic!("Couldn't create {}: {}", &first_domain_path, error),
                        }
                    }
                    if let Some(ratio) =
                        self.topology.get_process_cpu_usage_percentage(last.process.pid)
                    {
                        let uj_to_add = ratio.value.parse::<f64>().unwrap()
                            * topo_energy.value.parse::<f64>().unwrap()
                            / 100.0;
                        let complete_path = format!("{path}/{vm_name}/intel-rapl:0");
                        match ProxmoxExporter::add_or_create(&complete_path, uj_to_add as u64) {
                            Ok(result) => {
                                trace!("{:?}", result);
                                debug!("Updated {}", complete_path);
                            }
                            Err(err) => {
                                error!(
                                    "Could not edit {}. Check file permissions: {}",
                                    complete_path, err
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    fn get_vm_name_from_cmdline(cmdline: &[String]) -> String {
        let mut vmid = String::new();
        let mut name = String::new();

        let mut iter = cmdline.iter();
        while let Some(arg) = iter.next() {
            if arg == "-id" {
                if let Some(val) = iter.next() {
                    vmid = val.clone();
                }
            }
            if arg == "-name" {
                if let Some(val) = iter.next() {
                    name = val.clone();
                }
            }
        }

        let clean_name = name.split(',').next().unwrap_or(&name);

        if !vmid.is_empty() && !clean_name.is_empty() {
            format!("{}-{}", vmid, clean_name)
        } else {
            String::from("unknown-vm")
        }
    }

fn add_or_create(path: &str, uj_value: u64) -> io::Result<()> {
    // Garante que o diretório exista
    if fs::read_dir(path).is_err() {
        match fs::create_dir_all(path) {
            Ok(_) => info!("Created directory {}", path),
            Err(error) => panic!("Couldn't create {}: {}", path, error),
        }
    }

    // Define o caminho completo do arquivo
    let file_path = format!("{}/{}", path, "energy_uj");

    // Lê o valor atual se o arquivo já existir, senão começa do zero
    let content = if let Ok(file) = fs::read_to_string(&file_path) {
        file.parse::<u64>().unwrap_or(0) + uj_value
    } else {
        uj_value
    };

    // Escreve o novo valor no arquivo
    fs::write(file_path, content.to_string())
}


    fn filter_qemu_vm_processes(processes: &[&Vec<ProcessRecord>]) -> Vec<Vec<ProcessRecord>> {
        let mut qemu_processes: Vec<Vec<ProcessRecord>> = vec![];
        trace!("Filtering {} processes", processes.len());
        for vecp in processes.iter() {
            if !vecp.is_empty() {
                if let Some(pr) = vecp.first() {
                    if let Some(res) = pr
                        .process
                        .cmdline
                        .iter()
                        .find(|x| x.contains("qemu-system") || x.contains("/usr/bin/kvm"))
                    {
                        debug!("Found QEMU process with command: {}", res);
                        qemu_processes.push(vecp.to_vec());
                    }
                }
            }
        }
        qemu_processes
    }
}
