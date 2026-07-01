//use std::fs::File;
use log::LevelFilter;
use std::mem::size_of;
use std::sync::atomic::{AtomicU8, AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

//use sysinfo::MINIMUM_CPU_UPDATE_INTERVAL;
use daemonize::Daemonize;
use shared_memory::{Shmem, ShmemConf};
use sysinfo::{CpuRefreshKind, RefreshKind, System};

use log4rs::append::rolling_file::RollingFileAppender;
use log4rs::append::rolling_file::policy::compound::CompoundPolicy;
use log4rs::append::rolling_file::policy::compound::roll::fixed_window::FixedWindowRoller;
use log4rs::append::rolling_file::policy::compound::trigger::size::SizeTrigger;
use log4rs::config::{Appender, Config, Root};
use log4rs::encode::pattern::PatternEncoder;

use crate::error::CpuBarError;
use crate::utils::now_realtime;

pub const DAEMON_SHM: &str = "/tmp/cpubars.shm";
pub const DAEMON_PID: &str = "/tmp/cpubars.pid";
pub const DAEMON_LOG: &str = "/tmp/cpubars.log";
pub const DAEMON_ARC: &str = "/tmp/cpubars.{}.log";
pub const LOG_KB: u64 = 8 * 1024; // MAGIC: 8KB for testing

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
    pub fn init_logger() -> Result<(), CpuBarError> {
        let trigger = SizeTrigger::new(LOG_KB);

        let roller = FixedWindowRoller::builder()
            .build(DAEMON_ARC, 1)
            .expect("couldn't create window roller");

        let policy = CompoundPolicy::new(Box::new(trigger), Box::new(roller));
        let file_appender = RollingFileAppender::builder()
            .encoder(Box::new(PatternEncoder::new("{d} [{t}] {l} - {m}{n}")))
            .build(DAEMON_LOG, Box::new(policy))
            .expect("couldn't create rolling appender");
        let config = Config::builder()
            .appender(Appender::builder().build("my_file_logger", Box::new(file_appender)))
            .build(
                Root::builder()
                    .appender("my_file_logger")
                    .build(LevelFilter::Info),
            )
            .expect("couldn't configure appender");

        // 6. Initialize the global logger
        log4rs::init_config(config).expect("couldn't init log");

        Ok(())
    }

    pub fn start() -> Result<(), CpuBarError> {
        Self::init_logger().unwrap();
        log::info!("starting server");

        let daemonize = Daemonize::new()
            .pid_file(DAEMON_PID) // File to store the process ID
            .chown_pid_file(true); // Update PID file ownership

        match daemonize.start() {
            Ok(_) => {
                Server::new().expect("couldn't create server").run();
            }
            Err(e) => eprintln!("Error, failed to daemonize: {}", e),
        }

        log::info!("starting server... DONE");
        Ok(())
    }

    pub fn new() -> Result<Self, CpuBarError> {
        let shm_res = ShmemConf::new()
            .size(size_of::<Buffer>())
            .flink(DAEMON_SHM)
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
                    .flink(DAEMON_SHM)
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
        let usage: Vec<u8> = self
            .sys_info
            .cpus()
            .iter()
            .map(|c| c.cpu_usage() as u8)
            .collect();

        log::info!("  usage {:?}", usage);
        let braw = self.shm.as_ptr() as *mut Buffer;
        unsafe {
            for (src, dst) in usage.iter().zip((*braw).cpu_usage.iter()) {
                dst.store(*src, Ordering::Release);
            }
        }
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
