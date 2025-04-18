use std::os::unix::prelude::{AsRawFd, RawFd};

use crate::{
    generated::{bpf_attach_type::BPF_LIRC_MODE2, bpf_prog_type::BPF_PROG_TYPE_LIRC_MODE2},
    programs::{load_program, Link, LinkRef, ProgramData, ProgramError},
    sys::{bpf_prog_attach, bpf_prog_detach},
};

use libc::{close, dup};

/// A program used to decode IR into key events for a lirc device.
///
/// [`LircMode2`] programs can be used to inspect infrared pulses, spaces,
/// and timeouts received by a lirc IR receiver.
///
/// [lirc]: https://www.kernel.org/doc/html/latest/userspace-api/media/rc/lirc-dev.html
///
/// # Minimum kernel version
///
/// The minimum kernel version required to use this feature is 4.18.
///
/// # Examples
///
/// ```no_run
/// # #[derive(thiserror::Error, Debug)]
/// # enum Error {
/// #     #[error(transparent)]
/// #     IO(#[from] std::io::Error),
/// #     #[error(transparent)]
/// #     Map(#[from] aya::maps::MapError),
/// #     #[error(transparent)]
/// #     Program(#[from] aya::programs::ProgramError),
/// #     #[error(transparent)]
/// #     Bpf(#[from] aya::BpfError)
/// # }
/// # let mut bpf = aya::Bpf::load(&[], None)?;
/// use std::fs::File;
/// use std::convert::TryInto;
/// use aya::programs::LircMode2;
///
/// let file = File::open("/dev/lirc0")?;
/// let mut bpf = aya::Bpf::load_file("imon_rsc.o")?;
/// let decoder: &mut LircMode2 = bpf.program_mut("imon_rsc")?.try_into().unwrap();
/// decoder.load()?;
/// decoder.attach(file)?;
/// # Ok::<(), Error>(())
/// ```
#[derive(Debug)]
#[doc(alias = "BPF_PROG_TYPE_LIRC_MODE2")]
pub struct LircMode2 {
    pub(crate) data: ProgramData,
}

impl LircMode2 {
    /// Loads the program inside the kernel.
    ///
    /// See also [`Program::load`](crate::programs::Program::load).
    pub fn load(&mut self) -> Result<(), ProgramError> {
        load_program(BPF_PROG_TYPE_LIRC_MODE2, &mut self.data)
    }

    /// Returns the name of the program.
    pub fn name(&self) -> String {
        self.data.name.to_string()
    }

    /// Attaches the program to the given lirc device.
    pub fn attach<T: AsRawFd>(&mut self, lircdev: T) -> Result<LinkRef, ProgramError> {
        let prog_fd = self.data.fd_or_err()?;
        let lircdev_fd = lircdev.as_raw_fd();

        bpf_prog_attach(prog_fd, lircdev_fd, BPF_LIRC_MODE2).map_err(|(_, io_error)| {
            ProgramError::SyscallError {
                call: "bpf_prog_attach".to_owned(),
                io_error,
            }
        })?;

        Ok(self.data.link(LircLink::new(prog_fd, lircdev_fd)))
    }
}

#[derive(Debug)]
struct LircLink {
    prog_fd: Option<RawFd>,
    target_fd: Option<RawFd>,
}

impl LircLink {
    pub(crate) fn new(prog_fd: RawFd, target_fd: RawFd) -> LircLink {
        LircLink {
            prog_fd: Some(prog_fd),
            target_fd: Some(unsafe { dup(target_fd) }),
        }
    }
}

impl Link for LircLink {
    fn detach(&mut self) -> Result<(), ProgramError> {
        if let Some(prog_fd) = self.prog_fd.take() {
            let target_fd = self.target_fd.take().unwrap();
            let _ = bpf_prog_detach(prog_fd, target_fd, BPF_LIRC_MODE2);
            unsafe { close(target_fd) };
            Ok(())
        } else {
            Err(ProgramError::AlreadyDetached)
        }
    }
}

impl Drop for LircLink {
    fn drop(&mut self) {
        if let Some(target_fd) = self.target_fd.take() {
            unsafe { close(target_fd) };
        }
    }
}
