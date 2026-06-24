use std::fs::OpenOptions;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;
//use std::time::{SystemTime, UNIX_EPOCH};

//use sysinfo::MINIMUM_CPU_UPDATE_INTERVAL;
use rustix::fs::{FlockOperation, flock};
use shared_memory::{Shmem, ShmemConf};

use crate::error::CpuBarError;
use crate::server::{Buffer, SHMEM_ID, Server};
use crate::utils::now_realtime;

/// Read the CPU usage from the server
///
/// TMUX will call the client program at rate
///
/// * `shm`: shared memory handle
pub struct Client {
    pub shm: Shmem,
}

impl Client {
    // Connect to an existing server
    pub fn connect() -> Result<Self, CpuBarError> {
        let shm = ShmemConf::new().flink(SHMEM_ID).open().map_err(|err| {
            let msg = format!("couldn't open shmem {:?}", err);
            CpuBarError::FileNotFound(msg)
        })?;

        let client = Self { shm };
        Ok(client)
    }

    // Spawn a server if it's not running and connect
    // TODO: revisit this and try the Daemonize crate?
    pub fn start_and_connect() -> Result<Self, CpuBarError> {
        // prevent clients from starting multiple
        let lock = OpenOptions::new()
            .create(true)
            .write(true)
            .open(format!("/tmp/{}.lock", SHMEM_ID))
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

    pub fn read(&self) {
        let braw = self.shm.as_ptr() as *const Buffer;
        unsafe {
            let n_cpus = (*braw).core_count.load(Ordering::Acquire);
            let time_w = (*braw).last_write.load(Ordering::Acquire);
            log::info!("cpu count {}, written {}", n_cpus, time_w);
        }
        self.load_read().expect("couldn't write from client");
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
