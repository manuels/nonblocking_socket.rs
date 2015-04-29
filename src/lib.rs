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

extern "C" {
	fn ioctl(fd: c_int, req: c_ulong, res: *mut c_int) -> c_int;
}

struct NonBlockingSocket<T:AsRawFd> {
	sock: T,
}

const FIONREAD: c_ulong = 0x541B;

impl<T:AsRawFd+Read+Write> NonBlockingSocket<T> {
	pub fn new(sock: T) -> NonBlockingSocket<T> {
		NonBlockingSocket {
			sock: sock
		}
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
		if try!(self.pending()) > 0 {
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
