// Copyright 2020 Dubiousjim <dubiousjim@gmail.com>. All rights reserved. MIT license.
#![allow(dead_code)]


use nix::{errno::Errno, Error, NixPath, Result};
use std::os::unix::io::RawFd;

use crate::open::Symlink;
#[allow(unused_imports)]
use crate::time::ntime_t;
use nix::sys::stat::mode_t;
use std::mem::{transmute, MaybeUninit};

use stat_imports::*;

#[cfg(any(
  target_os = "freebsd",
  target_os = "openbsd",
  target_os = "netbsd",
  target_os = "macos"
))]
mod stat_imports {
  pub use libc::{blkcnt_t, blksize_t, dev_t, gid_t, ino_t, mode_t, nlink_t, off_t, time_t, uid_t};
  pub(super) use libc::{fstat as fstat64, fstatat as fstatat64, stat as stat64};
  // st_mtime_nsec: ntime_t === c_long, but netbsd calls these st_atimensec..
}

#[cfg(target_env = "uclibc")]
mod stat_imports {
  pub use libc::{blkcnt64_t as blkcnt_t, ino64_t as ino_t, off64_t as off_t};
  pub use libc::{blksize_t, gid_t, mode_t, nlink_t, time_t, uid_t};
  pub(super) use libc::{fstat as fstat64, fstatat as fstatat64, stat as stat64};
  // st_mtime_nsec: ntime_t === c_long, except on uclibc where this stat field is unsigned
  #[cfg(any(target_arch = "arm", all(target_arch = "mips", target_pointer_width = "32")))]
  pub type dev_t = u32; // libc::dev_t is u64
  #[cfg(not(any(target_arch = "arm", all(target_arch = "mips", target_pointer_width = "32"))))]
  pub use libc::dev_t;
}

#[cfg(all(not(target_env = "uclibc"), any(target_os = "linux", target_os = "emscripten")))]
mod stat_imports {
  pub use libc::{blkcnt64_t as blkcnt_t, ino64_t as ino_t, off64_t as off_t};
  pub use libc::{blksize_t, gid_t, mode_t, nlink_t, time_t, uid_t};
  pub(super) use libc::{fstat64, fstatat64, stat64};
  // st_mtime_nsec: ntime_t === c_long
  #[cfg(all(target_env = "gnu", target_pointer_width = "32", target_arch = "mips"))]
  pub type dev_t = u32; // libc::dev_t is u64
  #[cfg(not(all(target_env = "gnu", target_pointer_width = "32", target_arch = "mips")))]
  pub use libc::dev_t;
}

#[cfg(target_os = "android")]
mod stat_imports {
  pub use libc::{blkcnt_t, gid_t, ino_t, off64_t as off_t, time_t, uid_t};
  pub(super) use libc::{fstat as fstat64, fstatat as fstatat64, stat as stat64};
  // st_mtime_nsec: ntime_t === c_long
  #[cfg(target_pointer_width = "32")]
  pub type dev_t = u64; // libc::dev_t is u32
  #[cfg(target_pointer_width = "64")]
  pub use libc::dev_t;
  #[cfg(target_pointer_width = "32")]
  pub type mode_t = u32; // libc::mode_t is u16
  #[cfg(target_pointer_width = "64")]
  pub use libc::mode_t;
  #[cfg(target_pointer_width = "32")]
  pub use libc::nlink_t;
  #[cfg(target_pointer_width = "64")]
  pub type nlink_t = u64; // libc::nlink_t is u32
  #[cfg(target_pointer_width = "32")]
  pub use libc::blksize_t;
  #[cfg(target_pointer_width = "64")]
  pub type blksize_t = i64; // libc::blksize_t is u64
}

// Based on https://github.com/rust-lang/rust/blob/master/src/libstd/sys/unix/weak.rs
#[cfg(all(target_os = "linux", any(target_env = "gnu", target_env = "musl")))]
macro_rules! syscall {
  (fn $name:ident($sysname:ident, $($arg_name:ident: $t:ty),*) -> $ret:ty) => (
    unsafe fn $name($($arg_name:$t),*) -> $ret {
      use libc::*;
      syscall(
        $sysname,
        $($arg_name as c_long),*
      ) as $ret
    }
  )
}

// Based on https://github.com/rust-lang/rust/blob/master/src/libstd/sys/unix/fs.rs

macro_rules! cfg_has_statx {
  ({ $($then_tt:tt)* } else { $($else_tt:tt)* }) => {
    cfg_if::cfg_if! {
      if #[cfg(all(target_os = "linux", any(target_env = "gnu", target_env = "musl")))] {
        $($then_tt)*
      } else {
        $($else_tt)*
      }
    }
  };
  ($($block_inner:tt)*) => {
    #[cfg(all(target_os = "linux", any(target_env = "gnu", target_env = "musl")))]
    {
      $($block_inner)*
    }
  };
}

#[cfg(any(target_os = "freebsd", target_os = "openbsd", target_os = "macos"))]
mod node_entry {
  use super::{stat64, transmute, MaybeUninit};

  #[derive(Clone, Copy, Debug)]
  #[repr(C)]
  pub struct NodeEntry {
    pub(super) head: stat64,
  }

  // see https://stackoverflow.com/questions/61318595
  #[repr(C)]
  pub(super) struct NodeEntryUninit {
    pub(super) head: MaybeUninit<stat64>,
  }

  impl NodeEntry {
    pub(super) fn initialize(stat: NodeEntryUninit) -> Self {
      unsafe { transmute(stat) }
    }
  }
}

#[cfg(target_os = "netbsd")]
mod node_entry {
  use super::{stat64, transmute, MaybeUninit};

  #[derive(Clone, Copy, Debug)]
  #[repr(C)]
  pub struct NodeEntry {
    pub(super) head: stat64,
    pub st_birthtime_nsec: ntime_t,
  }

  // see https://stackoverflow.com/questions/61318595
  #[repr(C)]
  pub(super) struct NodeEntryUninit {
    pub(super) head: MaybeUninit<stat64>,
    pub(super) st_birthtime_nsec: MaybeUninit<ntime_t>,
  }

  impl NodeEntry {
    pub(super) fn initialize(stat: NodeEntryUninit) -> Self {
      let mut stat = stat;
      let mut stat = unsafe {
        stat.st_birthtime_nsec = MaybeUninit::new(0);
        transmute(stat)
      };
      stat.st_birthtime_nsec = stat.head.st_birthtimensec;
      stat
    }
  }
}

#[cfg(not(any(
  target_os = "netbsd",
  target_os = "freebsd",
  target_os = "openbsd",
  target_os = "macos"
)))]
mod node_entry {
  use super::{ntime_t, stat64, time_t, transmute, MaybeUninit};

  #[derive(Clone, Copy, Debug)]
  #[repr(C)]
  pub struct NodeEntry {
    /*
      st_nlink: nlink_t,
      st_dev: dev_t,
      st_ino: ino64_t,
      st_rdev: dev_t,
      st_size: off64_t,
      st_blocks: blkcnt64_t,
      st_blksize: blksize_t,
      st_mode: mode_t,
      st_uid: uid_t,
      st_gid: gid_t,
      st_atime: time_t,
      st_mtime: time_t,
      st_ctime: time_t,
      st_atime_nsec: ntime_t, // same as c_long; but on uclibc this field is unsigned; also netbsd calls st_atimensec
      st_mtime_nsec: ntime_t, // same as c_long; but on uclibc this field is unsigned; also netbsd calls st_mtimensec
      st_ctime_nsec: ntime_t, // same as c_long; but on uclibc this field is unsigned; also netbsd calls st_ctimensec
    */
    pub(super) head: stat64,
    pub st_birthtime: time_t,
    pub st_birthtime_nsec: ntime_t,
  }

  // see https://stackoverflow.com/questions/61318595
  #[repr(C)]
  pub(super) struct NodeEntryUninit {
    pub(super) head: MaybeUninit<stat64>,
    pub(super) st_birthtime: MaybeUninit<time_t>,
    pub(super) st_birthtime_nsec: MaybeUninit<ntime_t>,
  }

  impl NodeEntry {
    pub(super) fn initialize(stat: NodeEntryUninit) -> Self {
      let mut stat = stat;
      unsafe {
        stat.st_birthtime = MaybeUninit::new(0);
        stat.st_birthtime_nsec = MaybeUninit::new(0);
        transmute(stat)
      }
    }
  }
}

pub use node_entry::*;

impl AsRef<stat64> for NodeEntry {
  fn as_ref(&self) -> &stat64 {
    &self.head
  }
}

// https://doc.rust-lang.org/std/ops/trait.Deref.html
// recommends only implementing for smart pointers (boxed?)
impl std::ops::Deref for NodeEntry {
  type Target = stat64;
  fn deref(&self) -> &Self::Target {
    &self.head
  }
}

#[cfg(all(target_os = "linux", target_env = "musl"))]
mod linux_imports {
  pub struct statx {
    pub stx_mask: u32,
    pub stx_blksize: u32,
    pub stx_attributes: u64,
    pub stx_nlink: u32,
    pub stx_uid: u32,
    pub stx_gid: u32,
    pub stx_mode: u16,
    pub __statx_pad1: [u16; 1],
    pub stx_ino: u64,
    pub stx_size: u64,
    pub stx_blocks: u64,
    pub stx_attributes_mask: u64,
    pub stx_atime: ::statx_timestamp,
    pub stx_btime: ::statx_timestamp,
    pub stx_ctime: ::statx_timestamp,
    pub stx_mtime: ::statx_timestamp,
    pub stx_rdev_major: u32,
    pub stx_rdev_minor: u32,
    pub stx_dev_major: u32,
    pub stx_dev_minor: u32,
    pub __statx_pad2: [u64; 14],
  }

  pub struct statx_timestamp {
    pub tv_sec: i64,
    pub tv_nsec: u32,
    pub __statx_timestamp_pad1: [i32; 1],
  }

  use std::os::raw::{c_int, c_uint};
  pub const STATX_ALL: c_uint = 0x0fff;
  pub const AT_STATX_SYNC_AS_STAT: c_int = 0x0000;
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
mod linux_imports {
  pub use libc::AT_STATX_SYNC_AS_STAT;
  pub use libc::STATX_ALL;
}

cfg_has_statx! {{
  use linux_imports::*;

  // We prefer `statx` on Linux if available, which contains file creation time.
  // Default `stat64` contains no creation time.
  unsafe fn try_statx(
    dirfd: libc::c_int,
    path: *const libc::c_char,
    flags: i32,
    mask: u32,
  ) -> Option<Result<NodeEntry>> {
    use std::sync::atomic::{AtomicU8, Ordering};
    use std::ptr;
    use nix::Error;

    // Linux kernel prior to 4.11 or glibc prior to glibc 2.28 don't support `statx`
    // We store the availability in global to avoid unnecessary syscalls.
    // 0: Unknown
    // 1: Not available
    // 2: Available
    static STATX_STATE: AtomicU8 = AtomicU8::new(0);
    syscall! {
        fn statx(
            SYS_statx,
            fd: libc::c_int,
            pathname: *const libc::c_char,
            flags: libc::c_int,
            mask: libc::c_uint,
            statxbuf: *mut libc::statx
        ) -> libc::c_int
    }

    match STATX_STATE.load(Ordering::Relaxed) {
      0 => {
        // It is a trick to call `statx` with NULL pointers to check if the syscall
        // is available. According to the manual, it is expected to fail with EFAULT.
        // We do this mainly for performance, since it is nearly hundreds times
        // faster than a normal successful call.
        let res = statx(0, ptr::null(), 0, STATX_ALL, ptr::null_mut());
        /*
        let err = Errno::result(res)
          .err() // Result<T, E> -> Option<E>
          .and_then(|e| e.as_errno());
        */
        let err = if res == -1 {
          Some(Errno::last())
        } else {
          None
        };
        // We don't check `err == Some(ENOSYS)` because the syscall may be limited
        // and returns `EPERM`. Listing all possible errors seems not a good idea.
        // See: https://github.com/rust-lang/rust/issues/65662
        if err != Some(Errno::EFAULT) {
          STATX_STATE.store(1, Ordering::Relaxed);
          return None;
        }
        STATX_STATE.store(2, Ordering::Relaxed);
      }
      1 => return None,
      _ => {}
    }

    let mut buf = MaybeUninit::uninit();
    let res = statx(dirfd, path, flags, mask, buf.as_mut_ptr());
    if res == -1 { return Some(Err(Error::last())); }
    let buf = buf.assume_init(); // this whole function is unsafe

    // We cannot fill `stat64` exhaustively because of private padding fields.
    use std::mem::zeroed;
    let mut entry: NodeEntry = zeroed();
    entry.head.st_dev = libc::makedev(buf.stx_dev_major, buf.stx_dev_minor);
    entry.head.st_ino = buf.stx_ino as ino_t;
    entry.head.st_nlink = buf.stx_nlink as nlink_t;
    entry.head.st_mode = buf.stx_mode as mode_t;
    entry.head.st_uid = buf.stx_uid as uid_t;
    entry.head.st_gid = buf.stx_gid as gid_t;
    entry.head.st_rdev = libc::makedev(buf.stx_rdev_major, buf.stx_rdev_minor);
    entry.head.st_size = buf.stx_size as off_t;
    entry.head.st_blksize = buf.stx_blksize as blksize_t;
    entry.head.st_blocks = buf.stx_blocks as blkcnt_t;
    entry.head.st_atime = buf.stx_atime.tv_sec as time_t;
    entry.head.st_mtime = buf.stx_mtime.tv_sec as time_t;
    entry.head.st_ctime = buf.stx_ctime.tv_sec as time_t;
    // we know that we're neither in target_os = "netbsd" nor target_env = "uclibc"
    entry.head.st_atime_nsec = buf.stx_atime.tv_nsec as ntime_t;
    entry.head.st_mtime_nsec = buf.stx_mtime.tv_nsec as ntime_t;
    entry.head.st_ctime_nsec = buf.stx_ctime.tv_nsec as ntime_t;
    entry.st_birthtime = buf.stx_btime.tv_sec as libc::time_t;
    entry.st_birthtime_nsec = buf.stx_btime.tv_nsec as ntime_t;
    Some(Ok(entry))
  }
} else {}}

pub fn fstatat<P: ?Sized + NixPath>(dirfd: Option<RawFd>, path: &P, links: Symlink) -> Result<NodeEntry> {
  path
    .with_nix_path(|cstr| {
      let mut reject_symlink = false;
      let flag = match links {
        Symlink::Follow => 0,
        Symlink::Open => libc::AT_SYMLINK_NOFOLLOW,
        Symlink::Fail => {
          reject_symlink = true;
          libc::AT_SYMLINK_NOFOLLOW
        }
      };
      let dirfd = dirfd.unwrap_or(libc::AT_FDCWD);

      cfg_has_statx! {
        if let Some(ret) = unsafe { try_statx(
          dirfd,
          cstr.as_ptr(),
          flag | AT_STATX_SYNC_AS_STAT,
          STATX_ALL,
        ) } {
          return match ret {
              Err(e) => Err(e),
              Ok(bstat) => if reject_symlink && (bstat.head.st_mode & libc::S_IFMT) == libc::S_IFLNK {
                Err(Error::from_errno(Errno::ELOOP))
              } else {
                  Ok(bstat)
              }
          }
        } // if
      }

      #[allow(clippy::uninit_assumed_init)]
      let mut stat = unsafe { MaybeUninit::<NodeEntryUninit>::uninit().assume_init() };
      let res = unsafe { fstatat64(dirfd, cstr.as_ptr(), stat.head.as_mut_ptr(), flag) };
      Errno::result(res)?;
      let stat = NodeEntry::initialize(stat);
      if reject_symlink && (stat.st_mode & libc::S_IFMT) == libc::S_IFLNK {
        Err(Error::from_errno(Errno::ELOOP))
      } else {
        Ok(stat)
      }
    })
    .and_then(|ok| ok)
}

pub fn fstat(fd: RawFd) -> Result<NodeEntry> {
  cfg_has_statx! {
    use std::ffi::CString;
    let empty = CString::new("").unwrap();
    if let Some(ret) = unsafe { try_statx(
      fd,
      empty.as_ptr(),
      AT_STATX_SYNC_AS_STAT | libc::AT_EMPTY_PATH,
      STATX_ALL,
    ) } {
      return ret;
    }
  }
  #[allow(clippy::uninit_assumed_init)]
  let mut stat = unsafe { MaybeUninit::<NodeEntryUninit>::uninit().assume_init() };
  let res = unsafe { fstat64(fd, stat.head.as_mut_ptr()) };
  Errno::result(res)?;
  Ok(NodeEntry::initialize(stat))
}

/// Queries only the high bits of st_mode
pub fn filetypeat<P: ?Sized + NixPath>(dirfd: RawFd, path: &P, links: Symlink) -> Result<mode_t> {
  // TODO: Option<RawFd>?
  // let dirfd = dirfd.unwrap_or(libc::AT_FDCWD);
  path
    .with_nix_path(|cstr| {
      let mut reject_symlink = false;
      let flag = match links {
        Symlink::Follow => 0,
        Symlink::Open => libc::AT_SYMLINK_NOFOLLOW,
        Symlink::Fail => {
          reject_symlink = true;
          libc::AT_SYMLINK_NOFOLLOW
        }
      };
      let mut stat = MaybeUninit::uninit();
      let res = unsafe { fstatat64(dirfd, cstr.as_ptr(), stat.as_mut_ptr(), flag) };
      Errno::result(res)?;
      let stat = unsafe { stat.assume_init() };
      let res = stat.st_mode & libc::S_IFMT;
      if reject_symlink && res == libc::S_IFLNK {
        Err(Error::from_errno(Errno::ELOOP))
      } else {
        Ok(res)
      }
    })
    .and_then(|ok| ok)
}

#[cfg(test)]
mod tests {
  use super::*;
  // TODO
}
