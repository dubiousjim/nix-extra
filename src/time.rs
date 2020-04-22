// Copyright 2020 Dubiousjim <dubiousjim@gmail.com>. All rights reserved. MIT license.
#![allow(dead_code)]

use nix::{errno::Errno, /*Error,*/ NixPath, Result};
use std::os::unix::io::RawFd;

/*
 * Exports:
 *   time_t, ntime_t, utime_t, Duration, TimeSpec, TimeVal
 *   // TimeSpec.tv_sec(), tv_nsec()
 *     libc::timespec { tv_sec: time_t, /* padding? */ tv_nsec: ntime_t }
 *   // TimeVal::from(timeval), tv_sec(), tv_usec()
 *     libc::timeval { tv_sec: time_t, tv_usec: utime_t }
 *   trait TimeValLike:: zero(), minutes/seconds/microseconds/nanoseconds(),
 *                       num_seconds/microseconds/nanoseconds() -> i64
 *   trait ClockLike::now() -> Self, as_rand_bits(&self) -> u64
 *     impl for Duration, TimeSpec, TimeVal
 *   nullary structs Now, Omit
 *   trait TimeLike
 *     impl for Now, Omit, SystemTime, Duration, TimeSpec, TimeVal
 *   futime(RawFd, atime, mtime: &TimeLike) -> nix::Result<()>
 *   utimeat(Option<RawFd>, &NixPath, atime, mtime: &TimeLike, links: Symlink::Follow/Open/Fail) -> nix::Result<()>
 */

pub use nix::sys::time::{time_t, TimeSpec, TimeVal, TimeValLike};
#[allow(non_camel_case_types)]
pub type utime_t = libc::suseconds_t;
pub use std::time::Duration;
#[allow(non_camel_case_types)]
pub type ntime_t = libc::c_long;

use crate::open::{openat, Mode, OFlag, Symlink};
use libc::{timespec, timeval};
use std::time::{SystemTime, UNIX_EPOCH};

pub use inner::*;

#[cfg(all(MACOS_ATLEAST_10_10, not(MACOS_ATLEAST_10_13)))]
mod inner {
  use super::*;

  /// Change the access and modification times of the file specified by a file descriptor.
  ///
  /// # References
  ///
  /// [futimens(3p)](http://pubs.opengroup.org/onlinepubs/9699919799/functions/futimens.html).
  /// [futimes(3)](http://man7.org/linux/man-pages/man3/futimes.3.html).
  // TODO?
  // pub fn futime(fd: RawFd, atime: &dyn TimeLike, mtime: &dyn TimeLike) -> Result<()> {
  pub fn futime<Ta: TimeLike, Tm: TimeLike>(fd: RawFd, atime: &Ta, mtime: &Tm) -> Result<()> {
    use nix::sys::stat::fstat;
    let times = match (atime.kind(), mtime.kind()) {
      (TimeLikeKind::Omit, TimeLikeKind::Omit) => return Ok(()),
      (TimeLikeKind::Omit, _) => {
        let info = fstat(fd)?;
        [
          timeval {
            tv_sec: info.st_atime as time_t,
            tv_usec: (info.st_atime_nsec / 1000) as utime_t,
          },
          mtime.as_timeval(),
        ]
      }
      (_, TimeLikeKind::Omit) => {
        let info = fstat(fd)?;
        [
          atime.as_timeval(),
          timeval {
            tv_sec: info.st_mtime as time_t,
            tv_usec: (info.st_mtime_nsec / 1000) as utime_t,
          },
        ]
      }
      (TimeLikeKind::Now, TimeLikeKind::Now) => {
        let dur = SystemTime::now().duration_since(UNIX_EPOCH).expect("before EPOCH");
        let tv = dur.as_timeval();
        [tv, tv]
      }
      _ => [atime.as_timeval(), mtime.as_timeval()],
    };
    let res = unsafe { libc::futimes(fd, &times[0]) };
    Errno::result(res).map(drop)
  }

  /// Change the access and modification times of a file.
  ///
  /// The file to be changed is determined relative to the directory associated
  /// with the file descriptor `dirfd` or the current working directory
  /// if `dirfd` is `None`.
  ///
  /// If `follow` is `false` and `path` names a symbolic link,
  /// then the mode of the symbolic link is changed.
  ///
  /// # References
  ///
  /// [utimensat(2)](http://pubs.opengroup.org/onlinepubs/9699919799/functions/utimens.html).
  /// [utimes(2)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/utimes.html).
  /// [lutimes(3)](http://man7.org/linux/man-pages/man3/lutimes.3.html).
  pub fn utimeat<P: ?Sized + NixPath, Ta: TimeLike, Tm: TimeLike>(
    dirfd: Option<RawFd>,
    path: &P,
    atime: &Ta,
    mtime: &Tm,
    links: Symlink,
  ) -> Result<()> {
    let fd = openat(dirfd, path, OFlag::O_WRONLY, Mode::empty(), links)?;
    // TODO: may need O_RDWR
    Errno::result(fd).and_then(|fd| match futime(fd, atime, mtime) {
      Ok(()) => {
        let res = unsafe { libc::close(fd) };
        Errno::result(res).map(drop)
      }
      Err(e) => {
        let res = unsafe { libc::close(fd) };
        // alternately, we could ignore res and always return Err(e)
        Errno::result(res).and(Err(e))
      }
    })
  }
}

#[cfg(not(all(MACOS_ATLEAST_10_10, not(MACOS_ATLEAST_10_13))))]
mod inner {
  use super::*;

  /// Change the access and modification times of the file specified by a file descriptor.
  ///
  /// # References
  ///
  /// [futimens(3p)](http://pubs.opengroup.org/onlinepubs/9699919799/functions/futimens.html).
  /// [futimes(3)](http://man7.org/linux/man-pages/man3/futimes.3.html).
  pub fn futime<Ta: TimeLike, Tm: TimeLike>(fd: RawFd, atime: &Ta, mtime: &Tm) -> Result<()> {
    let res = if atime.kind() == TimeLikeKind::TimeVal && mtime.kind() == TimeLikeKind::TimeVal {
      let times: [timeval; 2] = [atime.as_timeval(), mtime.as_timeval()];
      unsafe { libc::futimes(fd, &times[0]) }
    } else {
      let times: [timespec; 2] = [atime.as_timespec(), mtime.as_timespec()];
      unsafe { libc::futimens(fd, &times[0]) }
    };
    Errno::result(res).map(drop)
  }

  /// Change the access and modification times of a file.
  ///
  /// The file to be changed is determined relative to the directory associated
  /// with the file descriptor `dirfd` or the current working directory
  /// if `dirfd` is `None`.
  ///
  /// If `follow` is `false` and `path` names a symbolic link,
  /// then the mode of the symbolic link is changed.
  ///
  /// # References
  ///
  /// [utimensat(2)](http://pubs.opengroup.org/onlinepubs/9699919799/functions/utimens.html).
  /// [utimes(2)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/utimes.html).
  /// [lutimes(3)](http://man7.org/linux/man-pages/man3/lutimes.3.html).
  pub fn utimeat<P: ?Sized + NixPath, Ta: TimeLike, Tm: TimeLike>(
    dirfd: Option<RawFd>,
    path: &P,
    atime: &Ta,
    mtime: &Tm,
    links: Symlink,
  ) -> Result<()> {
    let flag = match links {
      Symlink::Follow => 0,
      Symlink::Open => libc::AT_SYMLINK_NOFOLLOW,
      Symlink::Fail => {
        let fd = openat(dirfd, path, OFlag::O_WRONLY, Mode::empty(), links)?;
        // TODO: may need O_RDWR
        match futime(fd, atime, mtime) {
          Ok(()) => {
            let res = unsafe { libc::close(fd) };
            return Errno::result(res).map(drop);
          }
          Err(e) => {
            let res = unsafe { libc::close(fd) };
            // alternately, we could ignore res and always return Err(e)
            return Errno::result(res).and(Err(e));
          }
        }
      }
    };
    let times: [timespec; 2] = [atime.as_timespec(), mtime.as_timespec()];
    let res = path.with_nix_path(|cstr| unsafe {
      libc::utimensat(dirfd.unwrap_or(libc::AT_FDCWD), cstr.as_ptr(), &times[0], flag)
    })?;

    Errno::result(res).map(drop)
  }
}

pub struct Now;
pub struct Omit;

#[derive(Clone, Copy, PartialEq)]
pub enum TimeLikeKind {
  Omit,
  Now,
  TimeVal,
  Other,
  // TimeSpec,
  // Duration,
  // SystemTime,
}

pub trait TimeLike {
  #[inline]
  fn kind(&self) -> TimeLikeKind {
    TimeLikeKind::Other
  }
  fn as_timeval(&self) -> timeval;
  fn as_timespec(&self) -> timespec;
}

impl TimeLike for TimeSpec {
  // fn kind(&self) -> TimeLikeKind { TimeLikeKind::TimeSpec }
  #[inline]
  fn as_timeval(&self) -> timeval {
    timeval {
      tv_sec: self.tv_sec(),
      tv_usec: (self.tv_nsec() / 1000) as utime_t,
    }
  }
  #[inline]
  fn as_timespec(&self) -> timespec {
    *self.as_ref()
  }
}

impl TimeLike for TimeVal {
  #[inline]
  fn kind(&self) -> TimeLikeKind {
    TimeLikeKind::TimeVal
  }
  #[inline]
  fn as_timeval(&self) -> timeval {
    *self.as_ref()
  }
  #[inline]
  fn as_timespec(&self) -> timespec {
    timespec {
      tv_sec: self.tv_sec(),
      tv_nsec: (self.tv_usec() as ntime_t) * 1000,
    }
  }
}

impl TimeLike for Duration {
  // fn kind(&self) -> TimeLikeKind { TimeLikeKind::Duration }
  #[inline]
  fn as_timeval(&self) -> timeval {
    timeval {
      tv_sec: self.as_secs() as time_t,
      tv_usec: self.subsec_micros() as utime_t,
    }
  }
  #[inline]
  fn as_timespec(&self) -> timespec {
    timespec {
      tv_sec: self.as_secs() as time_t,
      tv_nsec: self.subsec_nanos() as ntime_t,
    }
  }
}

impl TimeLike for SystemTime {
  // fn kind(&self) -> TimeLikeKind { TimeLikeKind::SystemTime }
  #[inline]
  fn as_timeval(&self) -> timeval {
    let dur = self.duration_since(UNIX_EPOCH).expect("before EPOCH");
    timeval {
      tv_sec: dur.as_secs() as time_t,
      tv_usec: dur.subsec_micros() as utime_t,
    }
  }
  #[inline]
  fn as_timespec(&self) -> timespec {
    let dur = self.duration_since(UNIX_EPOCH).expect("before EPOCH");
    timespec {
      tv_sec: dur.as_secs() as time_t,
      tv_nsec: dur.subsec_nanos() as ntime_t,
    }
  }
}

impl TimeLike for Now {
  #[inline]
  fn kind(&self) -> TimeLikeKind {
    TimeLikeKind::Now
  }
  #[inline]
  fn as_timeval(&self) -> timeval {
    let dur = SystemTime::now().duration_since(UNIX_EPOCH).expect("before EPOCH");
    timeval {
      tv_sec: dur.as_secs() as time_t,
      tv_usec: dur.subsec_micros() as utime_t,
    }
  }
  #[inline]
  fn as_timespec(&self) -> timespec {
    timespec {
      tv_sec: 0,
      tv_nsec: libc::UTIME_NOW,
    }
  }
}

impl TimeLike for Omit {
  #[inline]
  fn kind(&self) -> TimeLikeKind {
    TimeLikeKind::Omit
  }
  #[inline]
  fn as_timeval(&self) -> timeval {
    unreachable!()
  }
  #[inline]
  fn as_timespec(&self) -> timespec {
    timespec {
      tv_sec: 0,
      tv_nsec: libc::UTIME_OMIT,
    }
  }
}

pub trait ClockLike: TimeLike {
  fn now() -> Self;
  fn as_rand_bits(&self) -> u64;
}

impl ClockLike for Duration {
  /// Makes a new `Duration` from UNIX_EPOCH to current SystemTime.
  #[inline]
  fn now() -> Self {
    // Linux uses clock_gettime(CLOCK_REALTIME, &timespec)
    // OSX doesn't have that call until 10.12
    // Rust for macos/ios uses instead (for all versions): gettimeofday(&timeval, NULL)
    SystemTime::now().duration_since(UNIX_EPOCH).expect("before EPOCH")
  }
  #[inline]
  fn as_rand_bits(&self) -> u64 {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    return ((self.subsec_micros() as u64) * 65537) ^ (self.as_secs() as u64);
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    return ((self.subsec_nanos() as u64) * 65537) ^ (self.as_secs() as u64);
  }
}

impl ClockLike for TimeSpec {
  /// Makes a new `TimeSpec` from current SystemTime.
  #[inline]
  fn now() -> Self {
    let dur = Duration::now();
    // as_secs -> u64, as_nanos -> u128, subsec_nanos -> u32
    // TimeSpec::from(libc::timespec { tv_sec: dur.as_secs() as time_t, tv_nsec: dur.subsec_nanos() as ntime_t })
    let nanos: u128 = dur.as_nanos();
    use std::convert::TryInto;
    let nanos: i64 = nanos.try_into().expect("nanos out of bounds");
    TimeSpec::nanoseconds(nanos)
  }
  #[inline]
  fn as_rand_bits(&self) -> u64 {
    ((self.tv_nsec() as u64) * 65537) ^ (self.tv_sec() as u64)
  }
}

impl ClockLike for TimeVal {
  /// Makes a new `TimeVal` from current SystemTime.
  #[inline]
  fn now() -> Self {
    let dur = Duration::now();
    // as_secs -> u64, as_micros -> u128, subsec_micros -> u32
    TimeVal::from(libc::timeval {
      tv_sec: dur.as_secs() as time_t,
      tv_usec: dur.subsec_micros() as utime_t,
    })
  }
  #[inline]
  fn as_rand_bits(&self) -> u64 {
    ((self.tv_usec() as u64) * 65537) ^ (self.tv_sec() as u64)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  // TODO
}
