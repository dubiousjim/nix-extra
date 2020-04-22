// Copyright 2020 Dubiousjim <dubiousjim@gmail.com>. All rights reserved. MIT license.
#![allow(dead_code)]


use nix::{errno::Errno, Error, NixPath, Result};
use std::os::unix::io::RawFd;

use nix::sys::stat::{mode_t, Mode};
use std::ffi::{CString, OsStr};
use std::path::Path;

#[inline]
fn nix_cstr(path: &Path) -> Result<CString> {
  use std::os::unix::ffi::OsStrExt;
  CString::new(path.as_os_str().as_bytes()).map_err(|_| Error::InvalidPath)
}

// Based on https://github.com/rust-lang/rust/blob/master/src/libstd/fs.rs
// and https://github.com/nix-rust/nix/blob/master/src/sys/stat.rs

pub fn mkdirat<P: ?Sized + NixPath>(dirfd: Option<RawFd>, path: &P, mode: Mode, recursive: bool) -> Result<()> {
  path
    .with_nix_path(|cstr| {
      use std::os::unix::ffi::OsStrExt;
      let path = Path::new(OsStr::from_bytes(cstr.to_bytes()));
      match dirfd {
        Some(dirfd) => {
          if recursive {
            do_allat(dirfd, path, mode.bits() as mode_t)
          } else {
            do_mkdirat(dirfd, path, mode.bits() as mode_t)
          }
        }
        None =>
        // if recursive { do_all(path, mode.bits() as mode_t) } else { do_mkdir(path, mode.bits() as mode_t) },
        {
          if recursive {
            do_allat(libc::AT_FDCWD, path, mode.bits() as mode_t)
          } else {
            do_mkdirat(libc::AT_FDCWD, path, mode.bits() as mode_t)
          }
        }
      }
    })
    .and_then(|ok| ok)
}

fn do_mkdirat(dirfd: RawFd, path: &Path, mode: mode_t) -> Result<()> {
  let cstr = nix_cstr(path)?;
  let res = unsafe { libc::mkdirat(dirfd, cstr.as_ptr(), mode) };
  Errno::result(res).map(drop)
}

fn do_allat(dirfd: RawFd, path: &Path, mode: mode_t) -> Result<()> {
  // path has is_empty via the NixPath trait
  if path.is_empty() {
    return Ok(());
  }
  match do_mkdirat(dirfd, path, mode) {
    Ok(()) => return Ok(()),
    Err(ref e) if e.as_errno() == Some(Errno::ENOENT) => {}
    Err(_) if path.is_dir() => return Ok(()),
    Err(e) => return Err(e),
  }
  match path.parent() {
    Some(parent) => do_allat(dirfd, parent, 0o777)?,
    // failed to create whole tree
    None => {
      return Err(Error::Sys(Errno::EACCES));
    }
  }
  match do_mkdirat(dirfd, path, mode) {
    Ok(()) => Ok(()),
    Err(_) if path.is_dir() => Ok(()),
    Err(e) => Err(e),
  }
}

/*
fn do_mkdir(path: &Path, mode: mode_t) -> Result<()> {
  let cstr = nix_cstr(path)?;
  let res = unsafe { libc::mkdir(cstr.as_ptr(), mode) };
  Errno::result(res).map(drop)
}

fn do_all(path: &Path, mode: mode_t) -> Result<()> {
  // path has is_empty via the NixPath trait
  if path.is_empty() {
    return Ok(());
  }
  match do_mkdir(path, mode) {
    Ok(()) => return Ok(()),
    Err(ref e) if e.as_errno() == Some(Errno::ENOENT) => {}
    Err(_) if path.is_dir() => return Ok(()),
    Err(e) => return Err(e),
  }
  match path.parent() {
    Some(parent) => do_all(parent, 0o777)?,
    // failed to create whole tree
    None => {
      return Err(Error::Sys(Errno::EACCES));
    }
  }
  match do_mkdir(path, mode) {
    Ok(()) => Ok(()),
    Err(_) if path.is_dir() => Ok(()),
    Err(e) => Err(e),
  }
}
*/

#[cfg(test)]
mod tests {
  use super::*;
  // TODO
}
