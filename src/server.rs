use std::mem::size_of;
use std::sync::atomic::{AtomicU8, AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

//use sysinfo::MINIMUM_CPU_UPDATE_INTERVAL;
use fork::{Fork, daemon};
use shared_memory::{Shmem, ShmemConf};
use sysinfo::{CpuRefreshKind, RefreshKind, System};

use crate::error::CpuBarError;
use crate::utils::now_realtime;

// TODO: if using flink should use an absolute path, e.g. /tmp/shmem...
pub const SHMEM_ID: &str = "CPU_BAR_SHEM_ID_TOO";

#[repr(C)]
pub struct Buffer {
    pub core_count: AtomicUsize,
    //pub read_period_ms: AtomicU16, // target period between writes
    pub last_read: AtomicU64,       // time last read, sec from unix epoch
    pub last_write: AtomicU64,      // time last written, sec from unix epoch
    pub cpu_usage: [AtomicU8; 256], // MAGIC: a DGX A100 has 256 logical threads
}
// TODO: add getters/setters?
// NOTE: maybe use a lock instead of atomics?
// SEE: shared_memory/examples/mutex.rs

pub struct Server {
    pub shm: Shmem,
    pub period: Duration,
    pub sys_info: System,
}

impl Server {
    pub fn start() -> Result<(), CpuBarError> {
        log::info!("starting server");
        if let Ok(Fork::Child) = daemon(false, false) {
            let mut server = Self::new().unwrap();
            server.run();
        }
        log::info!("starting server... DONE");
        Ok(())
    }

    pub fn new() -> Result<Self, CpuBarError> {
        let shm_res = ShmemConf::new()
            .size(size_of::<Buffer>())
            .flink(SHMEM_ID)
            .create()
            .map_err(|err| {
                let msg = format!("couldn't create shmem {:?}", err);
                CpuBarError::IOError(msg)
            });

        let mut shm = match shm_res {
            Ok(shm) => shm,
            Err(_err) => {
                log::warn!("shmem file exists, overwriting it!");
                ShmemConf::new()
                    .size(size_of::<Buffer>())
                    .flink(SHMEM_ID)
                    .force_create_flink()
                    .open()
                    .expect("couldn't open existing")
            }
        };

        shm.set_owner(true);

        // TODO: implement to/from ShmemError

        let period = Duration::from_millis(500);
        let sys_info = System::new_with_specifics(
            RefreshKind::nothing().with_cpu(CpuRefreshKind::everything()),
        );
        let mut server = Self {
            shm,
            period,
            sys_info,
        };
        server.write()?;
        Ok(server)
    }

    pub fn refresh(&mut self) {
        self.sys_info.refresh_cpu_all();
        let usage: Vec<f32> = self.sys_info.cpus().iter().map(|c| c.cpu_usage()).collect();
        log::info!("cpus: {:?}", usage);
    }

    pub fn store_time_written(&mut self) -> Result<(), CpuBarError> {
        let (sec, _nsec) = now_realtime()?;
        let braw = self.shm.as_ptr() as *mut Buffer;
        unsafe {
            (*braw).last_write.store(sec, Ordering::Release);
        }
        Ok(())
    }

    pub fn write(&mut self) -> Result<(), CpuBarError> {
        let nlog = num_cpus::get();
        {
            let braw = self.shm.as_ptr() as *mut Buffer;
            unsafe {
                (*braw).core_count.store(nlog, Ordering::Release);
            }
        }
        self.store_time_written()?;

        Ok(())
    }

    pub fn read(&self) {
        let braw = self.shm.as_ptr() as *const Buffer;
        unsafe {
            let n_cpus = (*braw).core_count.load(Ordering::Acquire);
            let time_r = (*braw).last_read.load(Ordering::Acquire);
            log::info!("cpu count {}, read {}", n_cpus, time_r);
        }
    }

    pub fn run(&mut self) {
        loop {
            self.refresh();

            let werr = self.write();
            if werr.is_err() {
                log::error!("couldn't write {:?}", werr);
            }
            self.read();

            std::thread::sleep(self.period);
        }
    }
}
