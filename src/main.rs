use std::fs::OpenOptions;
use std::mem::size_of;
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;
//use std::time::{SystemTime, UNIX_EPOCH};

//use sysinfo::MINIMUM_CPU_UPDATE_INTERVAL;
use env_logger::Env;
use fork::{Fork, daemon};
use rustix::fd::AsRawFd;
use rustix::fs::{FlockOperation, flock};
use shared_memory::{Shmem, ShmemConf};
use thiserror::Error;

const SHMEM_ID: &str = "CPU_BAR_SHEM_ID";

#[derive(Error, Debug)]
pub enum CpuBarError {
    #[error("Not implemented")]
    NotImplemented,

    #[error("Access denied: insufficient permissions.")]
    PermissionDenied,

    #[error("I/O error: {0}")]
    IOError(String),

    #[error("File Does Not Exist at {0}")]
    FileNotFound(String),

    #[error("Server not running")]
    ServerNotFound,

    #[error("Another client is starting the server")]
    ClientBusy,
}

#[repr(C)]
pub struct Buffer {
    pub core_count: AtomicUsize,
    //pub read_period_ms: AtomicU16, // target period between writes
    //pub last_read: AtomicI64,  // time last read, sec from unix epoch
    //pub last_write: AtomicI64,  // time last written, sec from unix epoch
    pub cpu_usage: [AtomicU8; 256], // MAGIC: a DGX A100 has 256 logical threads
}

pub struct Server {
    pub shm: Shmem,
    pub period: Duration,
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
        let mut server = Self { shm, period };
        server.write();
        Ok(server)
    }

    pub fn write(&mut self) {
        let nlog = num_cpus::get();
        {
            let braw = self.shm.as_ptr() as *mut Buffer;
            unsafe {
                (*braw).core_count.store(nlog, Ordering::Release);
            }
            println!("stored cpu count {}", nlog);
        }
    }

    pub fn run(&mut self) {
        loop {
            std::thread::sleep(self.period);
            self.write();
        }
    }
}

// Read the CPU usage
//
// TMUX will call the client program at rate
//
pub struct Client {
    pub shm: Shmem,
}

impl Client {
    // Connect to an existing server
    pub fn connect() -> Result<Self, CpuBarError> {
        let shm = ShmemConf::new().os_id(SHMEM_ID).open().map_err(|err| {
            let msg = format!("couldn't open shmem {:?}", err);
            CpuBarError::FileNotFound(msg)
        })?;

        let client = Self { shm };
        Ok(client)
    }

    // Spawn a server if it's not running and connect
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
            log::info!("cpu count {}", n_cpus);
        }
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

fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let producer = thread::spawn(move || {
        let mut server = Server::new().expect("couldn't open server");
        server.run();
    });

    let consumer = thread::spawn(move || {
        let client = Client::start_and_connect().expect("couldn't start and connect");
        loop {
            client.read();
            thread::sleep(Duration::from_secs(1));
        }
    });

    consumer.join();
    log::info!("client joined");
    //producer.join();
    //log::info!("server joined");
}
