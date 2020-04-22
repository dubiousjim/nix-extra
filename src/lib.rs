// Copyright 2020 Dubiousjim <dubiousjim@gmail.com>. All rights reserved. MIT license.
// This is a staging area for functionality that we'd like to get into
// the nix crate <https://github.com/nix-rust/nix>, but aren't yet present in its v0.17.0.

pub use nix::{errno::Errno, Error, NixPath, Result};
pub use std::os::unix::io::RawFd;
pub use std::path::{Path, PathBuf};

pub use nix::dir::{Dir, Entry as DirEntry, Iter as DirIter, Type as DirEntryType};
pub use nix::fcntl::{fcntl, flock, open, openat, readlink, readlinkat, renameat};
pub use nix::fcntl::{AtFlags, FcntlArg, FdFlag, FlockArg /*OFlag*/};
pub use nix::sys::sendfile::sendfile;
pub use nix::sys::stat::{dev_t, mode_t, FchmodatFlags, SFlag /*Mode, UtimensatFlags*/};
pub use nix::sys::stat::{
  fchmod, fchmodat, fstat as nix_fstat, fstatat as nix_fstatat, lstat, mkdirat as nix_mkdirat, mknod, stat,
  umask, /*utimensat, futimens, utimes, lutimes,*/
};
pub use nix::unistd::{
  access, chown, fchownat, ftruncate, linkat, symlinkat, truncate, unlink, unlinkat as nix_unlinkat,
};
pub use nix::unistd::{chdir, fchdir, getcwd, mkdir, mkstemp};
pub use nix::unistd::{close, dup, dup2, dup3, fsync, lseek, mkfifo, read, write};
pub use nix::unistd::{getegid, geteuid, getgid, gethostname, getpgid, getpgrp, getsid, getuid, tcgetpgrp};
pub use nix::unistd::{getpid, getppid, isatty};
pub use nix::unistd::{setegid, seteuid, setgid, sethostname, setpgid, setsid, setuid, tcsetpgrp};
pub use nix::unistd::{/*AccessFlags,*/ FchownatFlags, LinkatFlags, UnlinkatFlags as nix_UnlinkatFlags, Whence};
pub use nix::unistd::{/*Gid, Uid,*/ Group, Pid, User, ROOT};

#[cfg(target_os = "linux")]
pub use nix::fcntl::{copy_file_range, splice, tee, SpliceFFlags};
#[cfg(target_os = "linux")]
pub use nix::sys::stat::{major, makedev, minor};
#[cfg(target_os = "linux")]
pub use nix::unistd::{
  fdatasync, getgrouplist, getgroups, gettid, initgroups, lseek64, mkfifoat, setgroups, setresgid, setresuid, sync,
};

mod access; // TODO merge into stat?
mod chown;
mod mkdir;
mod open;
// mod scratch;
#[cfg(not(target_env = "musl"))]
mod stat;
mod temp;
mod time;

pub use access::*; // TODO merge into stat?
pub use chown::*;
pub use mkdir::*;
pub use open::*;
// pub use scratch::*;
#[cfg(not(target_env = "musl"))]
pub use stat::*;
pub use temp::*;
pub use time::*;

#[cfg(test)]
mod tests {
  #[test]
  fn report_env() {
    #[cfg(target_os = "linux")]
    eprintln!("Running under linux");
    #[cfg(target_env = "gnu")]
    eprintln!("Running under gnu");
    #[cfg(target_env = "musl")]
    eprintln!("Running under musl");
    #[cfg(target_os = "mac")]
    eprintln!("Running under mac");
  }
}
