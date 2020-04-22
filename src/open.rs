// Copyright 2020 Dubiousjim <dubiousjim@gmail.com>. All rights reserved. MIT license.
#![allow(dead_code)]

use nix::{errno::Errno, /*Error,*/ NixPath, Result};
use std::os::unix::io::RawFd;

pub use nix::fcntl::OFlag;
pub use nix::sys::stat::Mode;

pub enum Symlink {
  Follow,
  Open,
  Fail,
}

pub fn openat<P: ?Sized + NixPath>(
  dirfd: Option<RawFd>,
  path: &P,
  oflags: OFlag,
  mode: Mode,
  links: Symlink,
) -> Result<RawFd> {
  #[allow(unused_mut)]
  // let mut reject_symlink = false;
  let modebits = libc::c_uint::from(mode.bits());
  let oflags = match links {
    Symlink::Follow => oflags.bits(),
    Symlink::Open => {
      #[cfg(any(target_os = "linux"))]
      {
        oflags.bits() | libc::O_PATH | libc::O_NOFOLLOW
      }
      #[cfg(any(target_os = "macos"))]
      {
        // not in libc crate
        const O_SYMLINK: libc::c_int = 0x20_0000;
        oflags.bits() | O_SYMLINK
      }
      #[cfg(not(any(target_os = "linux", target_os = "macos")))]
      {
        use nix::Error;
        return Err(Error::UnsupportedOperation);
      }
    }
    Symlink::Fail => {
      // reject_symlink = true;
      oflags.bits() | libc::O_NOFOLLOW
    }
  };
  let fd = path
    .with_nix_path(|cstr| unsafe { libc::openat(dirfd.unwrap_or(libc::AT_FDCWD), cstr.as_ptr(), oflags, modebits) })?;
  Errno::result(fd)
  /*
  let fd = path.with_nix_path(|cstr| {
    let fd = unsafe { libc::openat(dirfd.unwrap_or(libc::AT_FDCWD), cstr.as_ptr(), oflags, modebits) };
    if reject_symlink && fd != -1 {
      use std::mem;
      let mut stat = mem::MaybeUninit::uninit();
      if unsafe { libc::fstat(fd, stat.as_mut_ptr()) } != -1 {
        let stat = unsafe { stat.assume_init() };
        if (stat.st_mode & libc::S_IFMT) != libc::S_IFLNK {
          reject_symlink = false;
          return fd;
        }
      }
      unsafe { libc::close(fd) };
    }
    fd
  })?;
  if fd == -1 {
    Err(Error::Sys(Errno::last()))
  } else if reject_symlink {
    Err(Error::from_errno(Errno::ELOOP))
  } else {
    Ok(fd)
  }
  */
}

#[cfg(test)]
mod tests {
  use super::*;
  // TODO
}
