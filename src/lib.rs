#![allow(dead_code)]

extern crate libc;

use std::os::unix::io::AsRawFd;
use std::io::{Read, Write, Result, Error};

use libc::{c_int, c_ulong};

#[cfg(test)]
use std::net::{TcpListener, TcpStream};
#[cfg(test)]
use std::thread;
#[cfg(test)]
use std::sync::mpsc::channel;

const O_NONBLOCK: c_int = 00004000;
const F_GETFL: c_int = 3;
const F_SETFL: c_int = 4;

mod syscall {
	use libc::c_int;

	extern "C" {
		pub fn fcntl(fd: c_int, cmd: c_int, flags: c_int) -> c_int;
	}
}

pub fn set_blocking(fd: c_int, blocking: bool) -> Result<()> {
	let flags = unsafe { syscall::fcntl(fd, F_GETFL, 0) };
	if flags < 0 {
		return Err(Error::last_os_error());
	}

	let flags = if blocking { flags & !O_NONBLOCK } else { flags|O_NONBLOCK };
	let res = unsafe { syscall::fcntl(fd, F_SETFL, flags) };
	if res != 0 {
		return Err(Error::last_os_error());
	}

	Ok(())
}

extern "C" {
	fn ioctl(fd: c_int, req: c_ulong, res: *mut c_int) -> c_int;
}

pub struct NonBlockingSocket<T:AsRawFd> {
	sock: T,
	blocking: bool,
}

const FIONREAD: c_ulong = 0x541B;

impl<T:AsRawFd+Read+Write> NonBlockingSocket<T> {
	pub fn new(sock: T) -> NonBlockingSocket<T> {
		NonBlockingSocket {
			sock: sock,
			blocking: true,
		}
	}

	pub fn set_blocking(&mut self, blocking: bool) -> Result<()> {
		let fd = self.sock.as_raw_fd();
		try!(set_blocking(fd, blocking));
		self.blocking = blocking;

		Ok(())
	}

	pub fn pending(&self) -> Result<usize> {
		let fd = self.sock.as_raw_fd();
		
		let mut data = 0 as c_int;
		let ptr = &mut data;

		let res = unsafe {
			ioctl(fd, FIONREAD, ptr)
		};

		if res == 0 {
			let count = *ptr as usize;
			Ok(count)
		} else {
			Err(Error::last_os_error())
		}
	}
}

impl<T:AsRawFd+Read+Write> Read for NonBlockingSocket<T> {
	fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
		if try!(self.pending()) > 0 || self.blocking {
			self.sock.read(buf)
		} else {
			Ok(0)
		}
	}
}

impl<T:AsRawFd+Read+Write> Write for NonBlockingSocket<T> {
	fn write(&mut self, buf: &[u8]) -> Result<usize> {
		self.sock.write(buf)
	}

	fn flush(&mut self) -> Result<()> {
		self.sock.flush()
	}
}

#[test]
fn it_works() {
	let server = TcpListener::bind("127.0.0.1:34254").unwrap();
	let (tx,rx) = channel();

	thread::spawn(move|| {
		let (stream, _) = server.accept().unwrap();
		tx.send(stream).unwrap();
	});

	let client = TcpStream::connect("127.0.0.1:34254").unwrap();;
	let mut stream = rx.recv().unwrap();

	let mut nonblocking = NonBlockingSocket::new(client);

	let mut buf = [0, 0, 0];

	assert_eq!(nonblocking.read(&mut buf).unwrap(), 0);
	stream.write(&buf[..]).unwrap();
	assert_eq!(nonblocking.pending().unwrap(), buf.len());
	assert_eq!(nonblocking.read(&mut buf).unwrap(), buf.len());
}
