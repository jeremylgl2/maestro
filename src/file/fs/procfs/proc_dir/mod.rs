//! This module implements the directory of a process in the procfs.

mod cwd;
mod mounts;

use crate::errno::Errno;
use crate::file::DirEntry;
use crate::file::FileContent;
use crate::file::FileType;
use crate::file::Gid;
use crate::file::Mode;
use crate::file::Uid;
use crate::file::fs::kernfs::KernFS;
use crate::file::fs::kernfs::node::KernFSNode;
use crate::process::Process;
use crate::process::pid::Pid;
use crate::util::boxed::Box;
use crate::util::container::hashmap::HashMap;
use crate::util::container::string::String;
use crate::util::io::IO;
use crate::util::ptr::cow::Cow;
use cwd::Cwd;
use mounts::Mounts;

/// Structure representing the directory of a process.
pub struct ProcDir {
	/// The PID of the process.
	pid: Pid,

	/// The content of the directory. This will always be a Directory variant.
	content: FileContent,
}

impl ProcDir {
	/// Creates a new instance for the process with the given PID `pid`.
	/// The function adds every nodes to the given kernfs `fs`.
	pub fn new(pid: Pid, fs: &mut KernFS) -> Result<Self, Errno> {
		let mut entries = HashMap::new();

		// TODO Add every nodes
		// TODO On fail, remove previously inserted nodes

		// Creating /proc/<pid>/cwd
		let node = Cwd {
			pid
		};
		let inode = fs.add_node(Box::new(node)?)?;
		entries.insert(String::from(b"cwd")?, DirEntry {
			inode,
			entry_type: FileType::Link,
		})?;

		// Creating /proc/<pid>/mounts
		let node = Mounts {
			pid
		};
		let inode = fs.add_node(Box::new(node)?)?;
		entries.insert(String::from(b"mounts")?, DirEntry {
			inode,
			entry_type: FileType::Regular,
		})?;

		Ok(Self {
			pid,

			content: FileContent::Directory(entries),
		})
	}
}

impl KernFSNode for ProcDir {
	fn get_mode(&self) -> Mode {
		0o555
	}

	fn get_uid(&self) -> Uid {
		let proc_mutex = Process::get_by_pid(self.pid).unwrap();
		let proc_guard = proc_mutex.lock();
		let proc = proc_guard.get();

		proc.get_euid()
	}

	fn get_gid(&self) -> Gid {
		let proc_mutex = Process::get_by_pid(self.pid).unwrap();
		let proc_guard = proc_mutex.lock();
		let proc = proc_guard.get();

		proc.get_egid()
	}

	fn get_content<'a>(&'a self) -> Cow<'a, FileContent> {
		Cow::from(&self.content)
	}
}

impl IO for ProcDir {
	fn get_size(&self) -> u64 {
		0
	}

	fn read(&mut self, _offset: u64, _buff: &mut [u8]) -> Result<(u64, bool), Errno> {
		Err(errno!(EINVAL))
	}

	fn write(&mut self, _offset: u64, _buff: &[u8]) -> Result<u64, Errno> {
		Err(errno!(EINVAL))
	}

	fn poll(&mut self, _mask: u32) -> Result<u32, Errno> {
		// TODO
		todo!();
	}
}

impl Drop for ProcDir {
	fn drop(&mut self) {
		// TODO Remove every nodes
	}
}
