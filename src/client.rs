use std::cmp;
use std::fs::OpenOptions;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;
//use std::time::{SystemTime, UNIX_EPOCH};

//use sysinfo::MINIMUM_CPU_UPDATE_INTERVAL;
use rustix::fs::{FlockOperation, flock};
use shared_memory::{Shmem, ShmemConf};

use crate::error::CpuBarError;
use crate::server::{Buffer, DAEMON_SHM, Server};
use crate::utils::now_realtime;

// NOTE: 1/8 is U+2581, .., 8/8 U+2588
pub const BARS: [char; 9] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Read the CPU usage from the server
///
/// TMUX will call the client program at rate
///
/// * `shm`: shared memory handle
pub struct Client {
    pub shm: Shmem,
    pub bars: String,
}

impl Client {
    // Connect to an existing server
    pub fn connect() -> Result<Self, CpuBarError> {
        let shm = ShmemConf::new().flink(DAEMON_SHM).open().map_err(|err| {
            let msg = format!("couldn't open shmem {:?}", err);
            CpuBarError::FileNotFound(msg)
        })?;

        let nlogical = num_cpus::get();
        let bars = "?".repeat(nlogical);
        let client = Self { shm, bars };
        Ok(client)
    }

    // Spawn a server if it's not running and connect
    // TODO: revisit this and try the Daemonize crate?
    pub fn start_and_connect() -> Result<Self, CpuBarError> {
        // prevent clients from starting multiple
        let lock = OpenOptions::new()
            .create(true)
            .write(true)
            .open(format!("/tmp/{}.lock", DAEMON_SHM))
            .map_err(|_err| CpuBarError::ClientBusy)?;

        if flock(lock, FlockOperation::NonBlockingLockExclusive).is_err() {
            return Err(CpuBarError::ClientBusy);
        }

        let client = match Self::connect() {
            Ok(client) => {
                log::info!("client connected");
                client
            }
            Err(_err) => {
                Server::start()?;
                Self::wait_for_server()?;
                Self::connect()?
            }
        };
        Ok(client)
    }

    pub fn usage_to_bars(usage: impl IntoIterator<Item = u8>) -> String {
        // MAGIC: [0..100) to indices for BARS [0..8]
        let usage: String = usage
            .into_iter()
            .map(|u| cmp::min(u, 99) as usize)
            .map(|u| u * BARS.len() / 100)
            .map(|u| BARS[u])
            .collect();
        usage
    }

    pub fn read(&self) -> String {
        let braw = self.shm.as_ptr() as *const Buffer;
        let mut usage: Vec<u8> = vec![];
        let bars = unsafe {
            let n_cpus = (*braw).core_count.load(Ordering::Acquire);
            let _time_w = (*braw).last_write.load(Ordering::Acquire);
            let cores = (*braw).cpu_usage.as_slice();
            usage = cores[0..n_cpus]
                .iter()
                .map(|c| c.load(Ordering::Acquire))
                .collect();
            Self::usage_to_bars(usage)
        };
        self.load_read().expect("couldn't write from client");
        bars
    }

    pub fn load_read(&self) -> Result<(), CpuBarError> {
        let (sec, _nsec) = now_realtime()?;
        let braw = self.shm.as_ptr() as *mut Buffer;
        unsafe {
            (*braw).last_read.store(sec, Ordering::Release);
        }
        Ok(())
    }

    pub fn wait_for_server() -> Result<(), CpuBarError> {
        for _ in 0..4 {
            if Self::connect().is_ok() {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(8));
        }
        Err(CpuBarError::ServerNotFound)
    }
}
