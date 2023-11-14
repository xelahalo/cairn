// Based on the simple.rs implementation in the fuser repo

use clap::{crate_version, Arg, Command};
use fuser::{
    Filesystem, KernelConfig, MountOption, ReplyAttr, ReplyData, ReplyDirectory,
    ReplyEmpty, ReplyEntry, ReplyOpen, ReplyStatfs, ReplyWrite, Request, TimeOrNow, 
};
use log::debug;
use log::{error, LevelFilter};
use std::cmp::min;
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader};
use std::os::fd::AsRawFd;
use std::os::raw::c_int;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs as ufs;
use std::os::unix::fs::FileExt;
use std::os::unix::prelude::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{env, fs, io};
use walkdir::WalkDir;
use std::ffi::CString;

type Inode = u64;

#[derive(Copy, Clone, PartialEq)]
enum FileKind {
    File,
    Directory,
    Symlink,
}

impl From<FileKind> for fuser::FileType {
    fn from(kind: FileKind) -> Self {
        match kind {
            FileKind::File => fuser::FileType::RegularFile,
            FileKind::Directory => fuser::FileType::Directory,
            FileKind::Symlink => fuser::FileType::Symlink,
        }
    }
}

fn time_now() -> (i64, u32) {
    time_from_system_time(&SystemTime::now())
}

fn system_time_from_time(secs: i64, nsecs: u32) -> SystemTime {
    if secs >= 0 {
        UNIX_EPOCH + Duration::new(secs as u64, nsecs)
    } else {
        UNIX_EPOCH - Duration::new((-secs) as u64, nsecs)
    }
}

fn time_from_system_time(system_time: &SystemTime) -> (i64, u32) {
    // Convert to signed 64-bit time with epoch at 0
    match system_time.duration_since(UNIX_EPOCH) {
        Ok(duration) => (duration.as_secs() as i64, duration.subsec_nanos()),
        Err(before_epoch_error) => (
            -(before_epoch_error.duration().as_secs() as i64),
            before_epoch_error.duration().subsec_nanos(),
        ),
    }
}

struct InodeAttributes {
    pub metadata: fs::Metadata,
    pub real_path: String,
}

impl From<InodeAttributes> for fuser::FileAttr {
    fn from(attrs: InodeAttributes) -> Self {
        let last_accessed = if let Ok(system_time) = attrs.metadata.accessed() {
            time_from_system_time(&system_time)
        } else {
            time_now()
        };
        let (last_modified, last_metadata_changed) =
            if let Ok(system_time) = attrs.metadata.modified() {
                (
                    time_from_system_time(&system_time),
                    time_from_system_time(&system_time),
                )
            } else {
                (time_now(), time_now())
            };

        let filetype = attrs.metadata.file_type();
        let kind = if filetype.is_file() {
            FileKind::File
        } else if filetype.is_dir() {
            FileKind::Directory
        } else if filetype.is_symlink() {
            FileKind::Symlink
        } else {
            unimplemented!();
        };

        fuser::FileAttr {
            ino: attrs.metadata.ino(),
            size: attrs.metadata.len(),
            blocks: attrs.metadata.blocks(),
            atime: system_time_from_time(last_accessed.0, last_accessed.1),
            mtime: system_time_from_time(last_modified.0, last_modified.1),
            ctime: system_time_from_time(last_metadata_changed.0, last_metadata_changed.1),
            crtime: SystemTime::UNIX_EPOCH,
            kind: kind.into(),
            perm: attrs.metadata.permissions().mode() as u16,
            nlink: attrs.metadata.nlink() as u32,
            uid: attrs.metadata.uid(),
            gid: attrs.metadata.gid(),
            rdev: attrs.metadata.rdev() as u32,
            blksize: attrs.metadata.blksize() as u32,
            flags: 0,
        }
    }
}

// In memory storing of the attributes of the files
struct TracerFS {
    root: String,
    attrs: BTreeMap<u64, InodeAttributes>,
}

impl TracerFS {
    fn new(root: String) -> TracerFS {
        {
            TracerFS {
                root,
                attrs: BTreeMap::new(),
            }
        }
    }

    fn get_path(self, parent: u64, name: &OsStr) -> PathBuf {
        let parent_context = self.attrs.get(&parent).unwrap();
        let parent_path = Path::new(&parent_context.real_path);
        parent_path.join(name)
    }

    fn lookup_name(&self, parent: u64, name: &OsStr) -> Result<InodeAttributes, c_int> {
        let path = self.get_path(parent, name);
        let metadata = fs::metadata(path);
        match metadata {
            Ok(metadata) => {
                let real_path = path.to_str().unwrap().to_string();
                let context = InodeAttributes {
                    metadata,
                    real_path,
                };
                Ok(context)
            }
            Err(e) => Err(e.raw_os_error().unwrap_or(libc::EIO)),
        }
    }
}

impl Filesystem for TracerFS {
    fn init(
        &mut self,
        _req: &Request,
        #[allow(unused_variables)] config: &mut KernelConfig,
    ) -> Result<(), c_int> {
        for entry in WalkDir::new(&self.root).into_iter().filter_map(|e| e.ok()) {
            let metadata = entry.metadata().unwrap();
            let real_path = entry.path().to_str().unwrap().to_string();
            let inode = metadata.ino();
            let context = InodeAttributes {
                metadata,
                real_path,
            };
            self.attrs.insert(inode, context);
        }

        Ok(())
    }

    fn lookup(&mut self, req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        debug!("lookup(parent={}, name={:?})", parent, name);

        match self.lookup_name(parent, name) {
            Ok(attrs) => {
                reply.entry(&Duration::new(0, 0), &attrs.into(), 0);
            }
            Err(e) => {
                reply.error(e);
            }
        }
    }

    fn forget(&mut self, _req: &Request, _ino: u64, _nlookup: u64) {
        debug!("forget(ino={}, nlookup={})", _ino, _nlookup);
    }

    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        debug!("getattr(ino={})", ino);

        match self.attrs.get(&ino) {
            Some(attrs) => {
                reply.attr(&Duration::new(0, 0), &(*attrs).into());
            }
            None => {
                reply.error(libc::ENOENT);
            }
        }
    }

    fn setattr(
        &mut self,
        req: &Request<'_>,
        ino: u64,
        mode: Option<u32>,
        uid: Option<u32>,
        gid: Option<u32>,
        size: Option<u64>,
        atime: Option<TimeOrNow>,
        mtime: Option<TimeOrNow>,
        ctime: Option<SystemTime>,
        fh: Option<u64>,
        crtime: Option<SystemTime>,
        chgtime: Option<SystemTime>,
        bkuptime: Option<SystemTime>,
        flags: Option<u32>,
        reply: ReplyAttr,
    ) {
        // get attrs and handle it properly
        let mut attrs = match self.attrs.get(&ino) {
            Some(attrs) => attrs,
            None => {
                reply.error(libc::ENOENT);
                return;
            }
        };

        if let Some(mode) = mode {
            debug!("chmod() called with {:?}, {:o}", ino, mode);
            if req.uid() != 0 && req.uid() != attrs.metadata.uid() {
                reply.error(libc::EPERM);
                return;
            }

            // change file modified time
            utime::set_file_times(
                &attrs.real_path,
                time_from_system_time(&attrs.metadata.accessed().unwrap()).0,
                time_now().0,
            )
            .unwrap();

            // load new metadata
            attrs.metadata = fs::metadata(&attrs.real_path).unwrap();

            // set the mode on the new metadata
            attrs.metadata.permissions().set_mode(mode);

            // save
            self.attrs.insert(ino, *attrs);
            reply.attr(&Duration::new(0, 0), &(*attrs).into());
            return;
        }

        if uid.is_some() || gid.is_some() {
            debug!("chown() called with {:?} {:?} {:?}", ino, uid, gid);

            ufs::chown(
                &attrs.real_path,
                uid,
                gid,
            );

            attrs.metadata = fs::metadata(&attrs.real_path).unwrap();
            self.attrs.insert(ino, *attrs);
            reply.attr(&Duration::new(0, 0), &(*attrs).into());
            return;
        }

        if let Some(size) = size {
            debug!("truncate() called with {:?} {:?}", ino, size);

            // open file and truncate it
            let mut file = match OpenOptions::new()
                .write(true)
                .open(&attrs.real_path) {
                    Ok(file) => file,
                    Err(err) => match err.kind() {
                        std::io::ErrorKind::NotFound => {
                            reply.error(libc::ENOENT);
                            return;
                        }
                        std::io::ErrorKind::PermissionDenied => {
                            reply.error(libc::EACCES);
                            return;
                        }
                        std::io::ErrorKind::AlreadyExists => {
                            reply.error(libc::EEXIST);
                            return;
                        }
                        std::io::ErrorKind::InvalidInput => {
                            reply.error(libc::EINVAL);
                            return;
                        }
                        _ => {
                            reply.error(libc::EIO);
                            return;
                        }
                    },
                };

            file.set_len(size).unwrap();
            attrs.metadata = file.metadata().unwrap();
            self.attrs.insert(ino, *attrs);
            return;
        }
        

        let now = time_now();
        if let Some(atime) = atime {
            debug!("utimens() called with {:?} {:?}", ino, atime);

            utime::set_file_times(
                &attrs.real_path,
                match atime {
                    TimeOrNow::SpecificTime(atime) => time_from_system_time(&atime).0,
                    TimeOrNow::Now => now.0,
                },
                time_from_system_time(&attrs.metadata.modified().unwrap()).0
            );

            attrs.metadata = fs::metadata(&attrs.real_path).unwrap();
            self.attrs.insert(ino, *attrs);
            return;
        }

        if let Some(mtime) = mtime {
            debug!("utimens() called with {:?} {:?}", ino, mtime);

            utime::set_file_times(
                &attrs.real_path,
                time_from_system_time(&attrs.metadata.accessed().unwrap()).0,
                match mtime {
                    TimeOrNow::SpecificTime(mtime) => time_from_system_time(&mtime).0,
                    TimeOrNow::Now => now.0,
                },
            );

            attrs.metadata = fs::metadata(&attrs.real_path).unwrap();
            self.attrs.insert(ino, *attrs);
            return;
        }
    }

    fn readlink(&mut self, _req: &Request<'_>, ino: u64, reply: ReplyData) {
        debug!("readlink(ino={})", ino);

        match self.attrs.get(&ino) {
            Some(attrs) => {
                if attrs.metadata.file_type().is_symlink() {
                    let path = Path::new(&attrs.real_path);
                    let link = fs::read_link(path).unwrap();
                    reply.data(link.as_os_str().as_bytes());
                } else {
                    reply.error(libc::EINVAL);
                }
            }
            None => {
                reply.error(libc::ENOENT);
            }
        } 
    }

    fn mknod(
            &mut self,
            _req: &Request<'_>,
            parent: u64,
            name: &OsStr,
            mode: u32,
            umask: u32,
            rdev: u32,
            reply: ReplyEntry,
        ) {
        debug!("mknod(parent={}, name={:?}, mode={}, rdev={})", parent, name, mode, rdev);
        let path = self.get_path(parent, name);
        
        // idk how else to do it 
        unsafe { libc::mknod(
            CString::new(path.to_str().unwrap()).unwrap().as_ptr(),
            mode as u16,
            umask as i32,
        ) }; 
        
        let metadata = fs::metadata(path);
        match metadata {
            Ok(metadata) => {
                let real_path = path.to_str().unwrap().to_string();
                let context = InodeAttributes {
                    metadata,
                    real_path,
                };
                self.attrs.insert(metadata.ino(), context);
                reply.entry(&Duration::new(0, 0), &context.into(), 0);
            }
            Err(e) => {
                reply.error(e.raw_os_error().unwrap_or(libc::EIO));
            }
        }
    }

    fn mkdir(
            &mut self,
            _req: &Request<'_>,
            parent: u64,
            name: &OsStr,
            mode: u32,
            umask: u32,
            reply: ReplyEntry,
        ) {
        debug!("mkdir(parent={}, name={:?}, mode={})", parent, name, mode);
        let path = self.get_path(parent, name);

        fs::create_dir(path);

        let metadata = fs::metadata(path);
        match metadata {
            Ok(metadata) => {
                let real_path = path.to_str().unwrap().to_string();
                let context = InodeAttributes {
                    metadata,
                    real_path,
                };
                self.attrs.insert(metadata.ino(), context);
                reply.entry(&Duration::new(0, 0), &context.into(), 0);
            }
            Err(e) => {
                reply.error(e.raw_os_error().unwrap_or(libc::EIO));
            }
        }
    }

    // TODO: remove inodes from self.attrs
    fn unlink(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        debug!("unlink(parent={}, name={:?})", parent, name);
        let path = self.get_path(parent, name);

        fs::remove_file(path);

        reply.ok();
    }

    // TODO: remove inodes from self.attrs
    fn rmdir(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        debug!("rmdir(parent={}, name={:?})", parent, name);
        let path = self.get_path(parent, name);

        fs::remove_dir(path);

        reply.ok();
    }

    fn symlink(
            &mut self,
            _req: &Request<'_>,
            parent: u64,
            name: &OsStr,
            link: &Path,
            reply: ReplyEntry,
        ) {
        debug!("symlink(parent={}, name={:?}, link={:?})", parent, name, link);
        let path = self.get_path(parent, name);

        ufs::symlink(link, path);

        let metadata = fs::metadata(path);
        match metadata {
            Ok(metadata) => {
                let real_path = path.to_str().unwrap().to_string();
                let context = InodeAttributes {
                    metadata,
                    real_path,
                };
                self.attrs.insert(metadata.ino(), context);
                reply.entry(&Duration::new(0, 0), &context.into(), 0);
            }
            Err(e) => {
                reply.error(e.raw_os_error().unwrap_or(libc::EIO));
            }
        }
    }

    fn rename(
            &mut self,
            _req: &Request<'_>,
            parent: u64,
            name: &OsStr,
            newparent: u64,
            newname: &OsStr,
            flags: u32,
            reply: ReplyEmpty,
        ) {
        debug!("rename(parent={}, name={:?}, newparent={}, newname={:?})", parent, name, newparent, newname);
        let path = self.get_path(parent, name);
        let newpath = self.get_path(newparent, newname);

        fs::rename(path, newpath);

        reply.ok();
    }

    fn link(
            &mut self,
            _req: &Request<'_>,
            ino: u64,
            newparent: u64,
            newname: &OsStr,
            reply: ReplyEntry,
        ) {
        debug!("link(ino={}, newparent={}, newname={:?})", ino, newparent, newname);
        let path = self.get_path(ino, OsStr::new(""));
        let newpath = self.get_path(newparent, newname);

        fs::hard_link(path, newpath);

        let metadata = fs::metadata(newpath);
        match metadata {
            Ok(metadata) => {
                let real_path = newpath.to_str().unwrap().to_string();
                let context = InodeAttributes {
                    metadata,
                    real_path,
                };
                self.attrs.insert(metadata.ino(), context);
                reply.entry(&Duration::new(0, 0), &context.into(), 0);
            }
            Err(e) => {
                reply.error(e.raw_os_error().unwrap_or(libc::EIO));
            }
        }
    }

    fn open(&mut self, _req: &Request<'_>, ino: u64, flags: i32, reply: ReplyOpen) {
        debug!("open(ino={}, flags={})", ino, flags);
        let (access_mask, read, write) = match flags & libc::O_ACCMODE {
            libc::O_RDONLY => {
                // Behavior is undefined, but most filesystems return EACCES
                if flags & libc::O_TRUNC != 0 {
                    reply.error(libc::EACCES);
                    return;
                }
                // FMODE_EXEC is 0x20
                if flags & 0x20 != 0 {
                    // Open is from internal exec syscall
                    (libc::X_OK, true, false)
                } else {
                    (libc::R_OK, true, false)
                }
            }
            libc::O_WRONLY => (libc::W_OK, false, true),
            libc::O_RDWR => (libc::R_OK | libc::W_OK, true, true),
            // Exactly one access mode flag must be specified
            _ => {
                reply.error(libc::EINVAL);
                return;
            }
        };
        
        match self.attrs.get(&ino) {
            Some(attrs) => {
                if attrs.metadata.file_type().is_file() {
                    let mut file = OpenOptions::new()
                        .read(read)
                        .write(write)
                        .open(&attrs.real_path)
                        .unwrap();

                    let file_handle = file.as_raw_fd() as u64;
                    reply.opened(file_handle, 0);
                } else {
                    reply.error(libc::EISDIR);
                }
            }
            None => {
                reply.error(libc::ENOENT);
            }
        }
    }

    fn read(
            &mut self,
            _req: &Request<'_>,
            ino: u64,
            fh: u64,
            offset: i64,
            size: u32,
            flags: i32,
            lock_owner: Option<u64>,
            reply: ReplyData,
        ) {
        debug!("read(ino={}, fh={}, offset={}, size={})", ino, fh, offset, size);
        match self.attrs.get(&ino) {
            Some(attrs) => {
                if attrs.metadata.file_type().is_file() {
                    if let Ok(file) = File::open(&attrs.real_path) {
                        let file_size = file.metadata().unwrap().len();
                        let read_size = min(size, file_size.saturating_sub(offset as u64) as u32);
                        let mut buffer = vec![0; size as usize];

                        file.read_exact_at(&mut buffer, offset as u64).unwrap();
                        reply.data(&buffer);
                    } else {
                        reply.error(libc::ENOENT)
                    }
                } else {
                    reply.error(libc::EISDIR);
                }
            }
            None => {
                reply.error(libc::ENOENT);
            }
        }
    }

    fn write(
            &mut self,
            _req: &Request<'_>,
            ino: u64,
            fh: u64,
            offset: i64,
            data: &[u8],
            write_flags: u32,
            flags: i32,
            lock_owner: Option<u64>,
            reply: ReplyWrite,
        ) {
    
        let attrs = self.attrs.get(&ino).unwrap();
        if let Ok(mut file) = OpenOptions::new().write(true).open(attrs.real_path) {
            file.write_all_at(data, offset as u64).unwrap();
            attrs.metadata = file.metadata().unwrap();
            self.attrs.insert(ino, *attrs);
            reply.written(data.len() as u32);
        } else {
            reply.error(libc::ENOENT);
        }
    }

    fn release(
            &mut self,
            _req: &Request<'_>,
            ino: u64,
            fh: u64,
            flags: i32,
            lock_owner: Option<u64>,
            flush: bool,
            reply: ReplyEmpty,
        ) {
        debug!("release(ino={}, fh={}, flags={})", ino, fh, flags);
        reply.ok();
    }

    fn opendir(&mut self, _req: &Request<'_>, ino: u64, flags: i32, reply: ReplyOpen) {
        debug!("opendir(ino={}, flags={})", ino, flags);
        let (access_mask, read, write) = match flags & libc::O_ACCMODE {
            libc::O_RDONLY => {
                // Behavior is undefined, but most filesystems return EACCES
                if flags & libc::O_TRUNC != 0 {
                    reply.error(libc::EACCES);
                    return;
                }
                if flags & FMODE_EXEC != 0 {
                    // Open is from internal exec syscall
                    (libc::X_OK, true, false)
                } else {
                    (libc::R_OK, true, false)
                }
            }
            libc::O_WRONLY => (libc::W_OK, false, true),
            libc::O_RDWR => (libc::R_OK | libc::W_OK, true, true),
            // Exactly one access mode flag must be specified
            _ => {
                reply.error(libc::EINVAL);
                return;
            }
        };

        match self.attrs.get(&ino) {
            Some(attrs) => {
                if attrs.metadata.file_type().is_dir() {
                    let mut file = OpenOptions::new()
                        .write(write)
                        .read(read)
                        .open(&attrs.real_path)
                        .unwrap();

                    let file_handle = file.as_raw_fd() as u64;
                    reply.opened(file_handle, 0);
                } else {
                    reply.error(libc::ENOTDIR);
                }
            }
            None => {
                reply.error(libc::ENOENT);
            }
        }
    }

    fn readdir(
            &mut self,
            _req: &Request<'_>,
            ino: u64,
            fh: u64,
            offset: i64,
            mut reply: ReplyDirectory,
        ) {
        debug!("readdir(ino={}, fh={}, offset={})", ino, fh, offset);
        if let Some(attrs) = self.attrs.get(&ino) {
            if attrs.metadata.file_type().is_dir() {
                let mut entries = Vec::new();
                for entry in fs::read_dir(&attrs.real_path).unwrap() {
                    let entry = entry.unwrap();
                    let metadata = entry.metadata().unwrap();
                    let kind = as_file_kind(metadata.mode());
                    let name = entry.file_name().to_str().unwrap();
                    let inode = metadata.ino();

                    entries.push((inode, kind, name));
                }

                for (i, (inode, kind, name)) in entries.into_iter().enumerate() {
                    if i as i64 >= offset {
                        let full_name = OsStr::new(name);
                        let buffer_full = reply.add(
                            inode,
                            offset + i as i64 + 1,
                            kind.into(),
                            &full_name,
                        );
                        if buffer_full {
                            break;
                        }
                    }
                }
                reply.ok();
            } else {
                reply.error(libc::ENOTDIR);
            }
        } else {
            reply.error(libc::ENOENT);
        }
    }

    fn releasedir(
            &mut self,
            _req: &Request<'_>,
            ino: u64,
            fh: u64,
            flags: i32,
            reply: ReplyEmpty,
        ) {
        debug!("releasedir(ino={}, fh={}, flags={})", ino, fh, flags);
        reply.ok();
    }

    fn statfs(&mut self, _req: &Request<'_>, _ino: u64, reply: ReplyStatfs) {
        debug!("statfs(ino={})", _ino);

        let mut statfs: libc::statvfs = unsafe { std::mem::zeroed() };
        let attrs = self.attrs.get(&_ino).unwrap();
        let path = Path::new(&attrs.real_path);
        let fd = path.as_os_str().to_str().unwrap();

        unsafe {
            libc::statvfs(fd.as_ptr() as *const i8, &mut statfs);
        }

        reply.statfs(
            statfs.f_blocks.into(),
            statfs.f_bfree.into(),
            statfs.f_bavail.into(),
            statfs.f_files.into(),
            statfs.f_ffree.into(),
            statfs.f_bsize as u32,
            statfs.f_namemax as u32,
            statfs.f_frsize as u32,
        ); 
    }
}

fn as_file_kind(mut mode: u32) -> FileKind {
    mode &= libc::S_IFMT as u32;

    if mode == libc::S_IFREG as u32 {
        return FileKind::File;
    } else if mode == libc::S_IFLNK as u32 {
        return FileKind::Symlink;
    } else if mode == libc::S_IFDIR as u32 {
        return FileKind::Directory;
    } else {
        unimplemented!("{}", mode);
    }
}

// fn main() {
//     let matches = Command::new("Fuser")
//         .version(crate_version!())
//         .author("Christopher Berner")
//         .arg(
//             Arg::new("data-dir")
//                 .long("data-dir")
//                 .value_name("DIR")
//                 .default_value("/tmp/fuser")
//                 .help("Set local directory used to store data")
//                 .takes_value(true),
//         )
//         .arg(
//             Arg::new("mount-point")
//                 .long("mount-point")
//                 .value_name("MOUNT_POINT")
//                 .default_value("")
//                 .help("Act as a client, and mount FUSE at given path")
//                 .takes_value(true),
//         )
//         .arg(
//             Arg::new("direct-io")
//                 .long("direct-io")
//                 .requires("mount-point")
//                 .help("Mount FUSE with direct IO"),
//         )
//         .arg(Arg::new("fsck").long("fsck").help("Run a filesystem check"))
//         .arg(
//             Arg::new("suid")
//                 .long("suid")
//                 .help("Enable setuid support when run as root"),
//         )
//         .arg(
//             Arg::new("v")
//                 .short('v')
//                 .multiple_occurrences(true)
//                 .help("Sets the level of verbosity"),
//         )
//         .get_matches();
//
//     let verbosity: u64 = matches.occurrences_of("v");
//     let log_level = match verbosity {
//         0 => LevelFilter::Error,
//         1 => LevelFilter::Warn,
//         2 => LevelFilter::Info,
//         3 => LevelFilter::Debug,
//         _ => LevelFilter::Trace,
//     };
//     env_logger::builder()
//         .format_timestamp_nanos()
//         .filter_level(log_level)
//         .init();
//
//     let mut options = vec![MountOption::FSName("fuser".to_string())];
//
//     #[cfg(feature = "abi-7-26")]
//     {
//         if matches.is_present("suid") {
//             info!("setuid bit support enabled");
//             options.push(MountOption::Suid);
//         } else {
//             options.push(MountOption::AutoUnmount);
//         }
//     }
//     #[cfg(not(feature = "abi-7-26"))]
//     {
//         options.push(MountOption::AutoUnmount);
//     }
//     if let Ok(enabled) = fuse_allow_other_enabled() {
//         if enabled {
//             options.push(MountOption::AllowOther);
//         }
//     } else {
//         eprintln!("Unable to read /etc/fuse.conf");
//     }
//
//     let data_dir: String = matches.value_of("data-dir").unwrap_or_default().to_string();
//
//     let mountpoint: String = matches
//         .value_of("mount-point")
//         .unwrap_or_default()
//         .to_string();
//
//     let result = fuser::mount2(
//         TracerFS::new(
//             data_dir,
//             matches.is_present("direct-io"),
//             matches.is_present("suid"),
//         ),
//         mountpoint,
//         &options,
//     );
//     if let Err(e) = result {
//         // Return a special error code for permission denied, which usually indicates that
//         // "user_allow_other" is missing from /etc/fuse.conf
//         if e.kind() == ErrorKind::PermissionDenied {
//             error!("{}", e.to_string());
//             std::process::exit(2);
//         }
//     }
// }

fn main() {
    let matches = Command::new("Cairn")
        .author("xelahalo <xelahalo@gmail.com>")
        .version(crate_version!())
        .about("Filesystem implementation for tracing I/O operations for forward build systems")
        .arg(
            Arg::new("root")
                .help("Root directory for the filesystem")
                .required(true),
        )
        .arg(
            Arg::new("mount-point")
                .help("Mountpoint for the filesystem")
                .required(true),
        )
        .arg(Arg::new("v").short('v').help("Sets the level of verbosity"))
        .get_matches();

    let verbosity = matches.get_one::<u64>("v").unwrap();
    let log_level = match verbosity {
        0 => LevelFilter::Error,
        1 => LevelFilter::Warn,
        2 => LevelFilter::Info,
        3 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };

    env_logger::builder()
        .format_timestamp_nanos()
        .filter_level(log_level)
        .init();

    let root = matches.get_one::<String>("root").unwrap().to_string();
    let mountpoint = matches.get_one::<String>("mount-point").unwrap();
    let options = vec![
        MountOption::FSName("cairn-fuse".to_string()),
    ];

    let result = fuser::mount2(TracerFS::new(root), mountpoint, &options);

    if let Err(e) = result {
        error!("Error mounting filesystem: {}", e);
    }
}
