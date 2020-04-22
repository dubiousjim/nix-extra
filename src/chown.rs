// Copyright 2020 Dubiousjim <dubiousjim@gmail.com>. All rights reserved. MIT license.
#![allow(dead_code)]


use nix::{errno::Errno, /*Error, NixPath,*/ Result};
use std::os::unix::io::RawFd;

pub use nix::unistd::{Gid, Uid};

/// Change the ownership of the file specified by a file descriptor to be owned
/// by the specified `owner` (user) and `group` (see
/// [fchown(2)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/fchown.html).
///
/// The owner/group for the provided path name will not be modified if `None` is
/// provided for that argument.  Ownership change will be attempted for the fd
/// only if `Some` owner/group is provided.
pub fn fchown(fd: RawFd, owner: Option<Uid>, group: Option<Gid>) -> Result<()> {
  // According to the POSIX specification, -1 is used to indicate that owner and group
  // are not to be changed.  Since uid_t and gid_t are unsigned types, we have to wrap
  // around to get -1.
  let uid: libc::uid_t = owner
    .map(Into::into)
    .unwrap_or_else(|| (0 as libc::uid_t).wrapping_sub(1));
  let gid: libc::gid_t = group
    .map(Into::into)
    .unwrap_or_else(|| (0 as libc::gid_t).wrapping_sub(1));

  let res = unsafe { libc::fchown(fd, uid, gid) };
  Errno::result(res).map(drop)
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::fs::File;

  #[test]
  fn test_fchown() {
    use std::os::unix::io::AsRawFd;
    // Testing for anything other than our own UID/GID is hard.
    let uid = Some(nix::unistd::getuid());
    let gid = Some(nix::unistd::getgid());

    let tempdir = tempfile::tempdir().unwrap();
    let path = tempdir.path().join("file");
    let file = File::create(&path).unwrap();
    let fd = file.as_raw_fd();
    // let fd = open(&path, OFlag::empty(), Mode::empty()).unwrap();

    fchown(fd, uid, gid).unwrap();
    fchown(fd, uid, None).unwrap();
    fchown(fd, None, gid).unwrap();
    // std::fs::remove_file(&path).unwrap();
  }
}
