// Based on https://github.com/cberner/fuser/blob/master/examples/simple.rs

use clap::{crate_version, Arg, Command};
use env_logger::fmt::Formatter;
use env_logger::Builder;
use fuser::{
    Filesystem, KernelConfig, MountOption, ReplyAttr, ReplyData, ReplyDirectory, ReplyEmpty,
    ReplyEntry, ReplyOpen, ReplyStatfs, ReplyWrite, Request, TimeOrNow, FUSE_ROOT_ID,
};
use log::{debug, info, LevelFilter};
use log::{warn, Record};
use std::cmp::min;
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs::{File, Metadata, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::num::Wrapping;
use std::os::fd::AsRawFd;
use std::os::raw::c_int;
use std::os::unix::fs as ufs;
use std::os::unix::fs::FileExt;
use std::os::unix::prelude::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{env, fs, io};
use walkdir::WalkDir;

const FMODE_EXEC: i32 = 0x20;

#[derive(Copy, Clone, PartialEq)]
enum FileKind {
    File,
    Directory,
    Symlink,
}

enum Reply {
    Entry(ReplyEntry),
    Attr(ReplyAttr),
    // Data(ReplyData),
    // Directory(ReplyDirectory),
    Empty(ReplyEmpty),
    // Open(ReplyOpen),
    // Write(ReplyWrite),
    // Statfs(ReplyStatfs),
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

#[derive(Clone)]
struct InodeAttributes {
    // pub metadata: fs::Metadata,
    pub ino: u64,
    pub uid: u32,
    pub gid: u32,
    pub mode: u32,
    pub atime: (i64, u32),
    pub mtime: (i64, u32),
    pub kind: FileKind,
    pub len: u64,
    pub nlinks: u64,
    pub blksize: u64,
    pub blocks: u64,
    pub rdev: u64,
    pub real_path: String,
}

impl From<(fs::Metadata, String)> for InodeAttributes {
    fn from(payload: (fs::Metadata, String)) -> Self {
        let ino = payload.0.ino();
        let uid = payload.0.uid();
        let gid = payload.0.gid();
        let mode = payload.0.mode();
        let kind = as_file_kind(payload.0.mode());
        let len = payload.0.len();
        let nlinks = payload.0.nlink();
        let blksize = payload.0.blksize();
        let blocks = payload.0.blocks();
        let rdev = payload.0.rdev();
        let real_path = payload.1;

        let atime = time_from_system_time(&match payload.0.accessed() {
            Ok(x) => x,
            Err(_) => panic!("Access time not supported on this platform."),
        });
        let mtime = time_from_system_time(&match payload.0.modified() {
            Ok(x) => x,
            Err(_) => panic!("Modification time not supported on this platform."),
        });

        InodeAttributes {
            ino,
            uid,
            gid,
            mode,
            atime,
            mtime,
            kind,
            len,
            nlinks,
            blksize,
            blocks,
            rdev,
            real_path,
        }
    }
}

impl From<InodeAttributes> for fuser::FileAttr {
    fn from(attrs: InodeAttributes) -> Self {
        fuser::FileAttr {
            ino: attrs.ino,
            size: attrs.len,
            blocks: attrs.blocks,
            atime: system_time_from_time(attrs.atime.0, attrs.atime.1),
            mtime: system_time_from_time(attrs.mtime.0, attrs.mtime.1),
            ctime: system_time_from_time(attrs.mtime.0, attrs.mtime.1),
            crtime: SystemTime::UNIX_EPOCH,
            kind: attrs.kind.into(),
            perm: attrs.mode as u16,
            nlink: attrs.nlinks as u32,
            uid: attrs.uid,
            gid: attrs.gid,
            rdev: attrs.rdev as u32,
            blksize: attrs.blksize as u32,
            flags: 0,
        }
    }
}

// In memory storing of the attributes of the files
struct TracerFS {
    root: String,
    attrs: BTreeMap<u64, InodeAttributes>,
    destroy: Sender<()>,
}

impl TracerFS {
    fn new(root: String, destroy: Sender<()>) -> TracerFS {
        {
            TracerFS {
                root,
                attrs: BTreeMap::new(),
                destroy,
            }
        }
    }

    fn get_path(&mut self, parent: u64, name: &OsStr) -> Result<PathBuf, c_int> {
        let parent_context = match self.attrs.get(&parent) {
            Some(x) => x,
            None => {
                return Err(libc::ENOENT);
            }
        };
        let parent_path = Path::new(&parent_context.real_path);
        Ok(parent_path.join(name))
    }

    fn lookup_name(&mut self, parent: u64, name: &OsStr) -> Result<InodeAttributes, c_int> {
        let path = match self.get_path(parent, name) {
            Ok(x) => x,
            Err(c) => {
                return Err(c);
            }
        };
        let metadata = fs::metadata(path.clone());
        match metadata {
            Ok(metadata) => {
                let real_path = path.to_str().unwrap().to_string();
                let attrs: InodeAttributes = (metadata, real_path).into();
                Ok(attrs)
            }
            Err(e) => Err(e.raw_os_error().unwrap_or(libc::EIO)),
        }
    }

    fn handle_metadata_on_removal<T>(
        &mut self,
        metadata: io::Result<fs::Metadata>,
        result: io::Result<T>,
        reply: ReplyEmpty,
    ) {
        match result {
            Ok(_) => match metadata {
                Ok(metadata) => {
                    self.attrs.remove(&metadata.ino());
                    reply.ok();
                }
                Err(e) => {
                    reply.error(e.raw_os_error().unwrap_or(libc::EIO));
                }
            },
            Err(e) => {
                reply.error(e.raw_os_error().unwrap_or(libc::EIO));
            }
        }
    }
    fn handle_metadata_on_change<T>(
        &mut self,
        path: &PathBuf,
        result: io::Result<T>,
        reply: Reply,
    ) {
        let handle_error = |e: io::Error, r: Reply| match r {
            Reply::Entry(r) => {
                r.error(e.raw_os_error().unwrap_or(libc::EIO));
            }
            Reply::Empty(r) => {
                r.error(e.raw_os_error().unwrap_or(libc::EIO));
            }
            Reply::Attr(r) => {
                r.error(e.raw_os_error().unwrap_or(libc::EIO));
            }
        };

        match result {
            Ok(_) => match fs::metadata(path) {
                Ok(metadata) => {
                    let real_path = path.to_str().unwrap().to_string();
                    let ino = metadata.ino();
                    let new_attrs: InodeAttributes = (metadata, real_path).into();
                    self.attrs.insert(ino, new_attrs.clone());
                    match reply {
                        Reply::Entry(reply) => {
                            reply.entry(&Duration::new(0, 0), &new_attrs.into(), 0);
                        }
                        Reply::Attr(reply) => {
                            reply.attr(&Duration::new(0, 0), &new_attrs.into());
                        }
                        Reply::Empty(reply) => {
                            reply.ok();
                        }
                    }
                }
                Err(e) => {
                    handle_error(e, reply);
                }
            },
            Err(e) => {
                handle_error(e, reply);
            }
        }
    }
}

impl Filesystem for TracerFS {
    fn init(&mut self, _req: &Request, _config: &mut KernelConfig) -> Result<(), c_int> {
        for entry in WalkDir::new(&self.root).into_iter().filter_map(|e| e.ok()) {
            debug!("init() entry: {:?}", entry);
            let metadata = entry.metadata().unwrap();
            let real_path = entry.path().to_str().unwrap().to_string();

            let inode = if real_path != self.root {
                metadata.ino()
            } else {
                FUSE_ROOT_ID
            };

            let attrs: InodeAttributes = (metadata, real_path).into();

            self.attrs.insert(inode, attrs);
        }

        File::create(".cairn-fuse-ready").expect("Failed to create .cairn-fuse-ready");

        Ok(())
    }

    fn destroy(&mut self) {
        debug!("destroy()");
        self.destroy.send(()).unwrap();
    }

    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        debug!("lookup(parent={}, name={:?})", parent, name);

        match self.lookup_name(parent, name) {
            Ok(attrs) => {
                self.attrs.insert(attrs.ino, attrs.clone());
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
                reply.attr(&Duration::new(0, 0), &(*attrs).clone().into());
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
        _ctime: Option<SystemTime>,
        _fh: Option<u64>,
        _crtime: Option<SystemTime>,
        _chgtime: Option<SystemTime>,
        _bkuptime: Option<SystemTime>,
        _flags: Option<u32>,
        reply: ReplyAttr,
    ) {
        let attrs = match self.attrs.get(&ino) {
            Some(attrs) => attrs,
            None => {
                reply.error(libc::ENOENT);
                return;
            }
        };

        if let Some(mode) = mode {
            debug!("chmod() called with {:?}, {:o}", ino, mode);
            if req.uid() != 0 && req.uid() != attrs.uid {
                reply.error(libc::EPERM);
                return;
            }

            trace(req.pid(), 'w', &["chmod", &attrs.real_path]);

            self.handle_metadata_on_change(
                &PathBuf::from(&attrs.real_path),
                fs::set_permissions(&attrs.real_path, PermissionsExt::from_mode(mode)),
                Reply::Attr(reply),
            );

            return;
        }

        if uid.is_some() || gid.is_some() {
            debug!("chown() called with {:?} {:?} {:?}", ino, uid, gid);

            trace(req.pid(), 'w', &["chown", &attrs.real_path]);

            self.handle_metadata_on_change(
                &PathBuf::from(&attrs.real_path),
                ufs::chown(&attrs.real_path, uid, gid),
                Reply::Attr(reply),
            );

            return;
        }

        if let Some(size) = size {
            debug!("truncate() called with {:?} {:?}", ino, size);

            // open file and truncate it
            let file = match OpenOptions::new().write(true).open(&attrs.real_path) {
                Ok(file) => file,
                Err(err) => match err.kind() {
                    io::ErrorKind::NotFound => {
                        reply.error(libc::ENOENT);
                        return;
                    }
                    io::ErrorKind::PermissionDenied => {
                        reply.error(libc::EACCES);
                        return;
                    }
                    io::ErrorKind::AlreadyExists => {
                        reply.error(libc::EEXIST);
                        return;
                    }
                    io::ErrorKind::InvalidInput => {
                        reply.error(libc::EINVAL);
                        return;
                    }
                    _ => {
                        reply.error(libc::EIO);
                        return;
                    }
                },
            };

            trace(req.pid(), 'w', &["truncate", &attrs.real_path]);

            self.handle_metadata_on_change(
                &PathBuf::from(&attrs.real_path),
                file.set_len(size),
                Reply::Attr(reply),
            );

            return;
        }

        let now = time_now();
        if let Some(atime) = atime {
            debug!("utime() called with {:?} {:?}", ino, atime);

            trace(req.pid(), 't', &["utime", &attrs.real_path]);

            self.handle_metadata_on_change(
                &PathBuf::from(&attrs.real_path),
                utime::set_file_times(
                    &attrs.real_path,
                    match atime {
                        TimeOrNow::SpecificTime(atime) => time_from_system_time(&atime).0,
                        TimeOrNow::Now => now.0,
                    },
                    attrs.mtime.0,
                ),
                Reply::Attr(reply),
            );

            return;
        }

        if let Some(mtime) = mtime {
            debug!("utime() called with {:?} {:?}", ino, mtime);

            trace(req.pid(), 't', &["utime", &attrs.real_path]);

            self.handle_metadata_on_change(
                &PathBuf::from(&attrs.real_path),
                utime::set_file_times(
                    &attrs.real_path,
                    attrs.atime.0,
                    match mtime {
                        TimeOrNow::SpecificTime(mtime) => time_from_system_time(&mtime).0,
                        TimeOrNow::Now => now.0,
                    },
                ),
                Reply::Attr(reply),
            );

            return;
        }
    }

    fn readlink(&mut self, _req: &Request<'_>, ino: u64, reply: ReplyData) {
        debug!("readlink(ino={})", ino);

        match self.attrs.get(&ino) {
            Some(attrs) => {
                if attrs.kind == FileKind::Symlink {
                    let path = Path::new(&attrs.real_path);
                    let link = match fs::read_link(path) {
                        Ok(x) => x,
                        Err(_) => {
                            reply.error(libc::EIO);
                            return;
                        }
                    };

                    // open file at link and read it
                    if let Ok(mut file) = File::open(link.clone()) {
                        let file_size = match file.metadata() {
                            Ok(x) => x,
                            Err(_) => {
                                reply.error(libc::EIO);
                                return;
                            }
                        }
                        .len();
                        let mut buffer = vec![0; file_size as usize];
                        match file.read_exact(&mut buffer) {
                            Ok(x) => x,
                            Err(_) => {
                                reply.error(libc::EIO);
                                return;
                            }
                        };

                        //trace(req.pid(), 'r', &["readlink", &link.to_str().unwrap()]);

                        reply.data(&buffer);
                        return;
                    } else {
                        reply.error(libc::ENOENT);
                        return;
                    }
                } else {
                    reply.error(libc::EINVAL);
                    return;
                }
            }
            None => {
                reply.error(libc::ENOENT);
                return;
            }
        }
    }

    fn mknod(
        &mut self,
        _req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        mode: u32,
        _umask: u32,
        rdev: u32,
        reply: ReplyEntry,
    ) {
        debug!(
            "mknod(parent={}, name={:?}, mode={}, rdev={})",
            parent, name, mode, rdev
        );
        let path = match self.get_path(parent, name) {
            Ok(x) => x,
            Err(c) => {
                reply.error(c);
                return;
            }
        };

        let file_type = mode & libc::S_IFMT as u32;
        if file_type != libc::S_IFREG as u32
            && file_type != libc::S_IFLNK as u32
            && file_type != libc::S_IFDIR as u32
        {
            // TODO
            warn!("mknod() implementation is incomplete. Only supports regular files, symlinks, and directories. Got {:o}", mode);
            reply.error(libc::ENOSYS);
            return;
        }

        // check if file already exists
        if self.lookup_name(parent, name).is_ok() {
            reply.error(libc::EEXIST);
            return;
        }

        let result = File::create(path.clone());
        self.handle_metadata_on_change(&path, result, Reply::Entry(reply));
    }

    fn mkdir(
        &mut self,
        _req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        mode: u32,
        _umask: u32,
        reply: ReplyEntry,
    ) {
        debug!("mkdir(parent={}, name={:?}, mode={})", parent, name, mode);
        let path = match self.get_path(parent, name) {
            Ok(x) => x,
            Err(c) => {
                reply.error(c);
                return;
            }
        };

        self.handle_metadata_on_change(&path, fs::create_dir(path.clone()), Reply::Entry(reply));
    }

    fn unlink(&mut self, req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        debug!("unlink(parent={}, name={:?})", parent, name);
        let path = match self.get_path(parent, name) {
            Ok(x) => x,
            Err(c) => {
                reply.error(c);
                return;
            }
        };
        let metadata = fs::metadata(path.clone());

        trace(req.pid(), 'd', &["unlink", &path.to_str().unwrap()]);
        self.handle_metadata_on_removal(metadata, fs::remove_file(path.clone()), reply);
    }

    fn rmdir(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        debug!("rmdir(parent={}, name={:?})", parent, name);
        let path = match self.get_path(parent, name) {
            Ok(x) => x,
            Err(c) => {
                reply.error(c);
                return;
            }
        };
        let metadata = fs::metadata(path.clone());

        self.handle_metadata_on_removal(metadata, fs::remove_dir(path), reply);
    }

    fn symlink(
        &mut self,
        _req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        link: &Path,
        reply: ReplyEntry,
    ) {
        debug!(
            "symlink(parent={}, name={:?}, link={:?})",
            parent, name, link
        );
        let path = match self.get_path(parent, name) {
            Ok(x) => x,
            Err(c) => {
                reply.error(c);
                return;
            }
        };

        self.handle_metadata_on_change(
            &path,
            ufs::symlink(link, path.clone()),
            Reply::Entry(reply),
        );
    }

    fn rename(
        &mut self,
        req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        newparent: u64,
        newname: &OsStr,
        _flags: u32,
        reply: ReplyEmpty,
    ) {
        debug!(
            "rename(parent={}, name={:?}, newparent={}, newname={:?})",
            parent, name, newparent, newname
        );
        let path = match self.get_path(parent, name) {
            Ok(x) => x,
            Err(c) => {
                reply.error(c);
                return;
            }
        };
        let newpath = match self.get_path(newparent, newname) {
            Ok(x) => x,
            Err(c) => {
                reply.error(c);
                return;
            }
        };

        trace(
            req.pid(),
            'm',
            &[
                "rename",
                &path.to_str().unwrap(),
                &newpath.to_str().unwrap(),
            ],
        );

        self.handle_metadata_on_change(
            &newpath,
            fs::rename(path.clone(), newpath.clone()),
            Reply::Empty(reply),
        );
    }

    fn link(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        newparent: u64,
        newname: &OsStr,
        reply: ReplyEntry,
    ) {
        debug!(
            "link(ino={}, newparent={}, newname={:?})",
            ino, newparent, newname
        );
        let path = match self.get_path(ino, OsStr::new("")) {
            Ok(x) => x,
            Err(c) => {
                reply.error(c);
                return;
            }
        };
        let newpath = match self.get_path(newparent, newname) {
            Ok(x) => x,
            Err(c) => {
                reply.error(c);
                return;
            }
        };

        self.handle_metadata_on_change(
            &newpath,
            fs::hard_link(path.clone(), newpath.clone()),
            Reply::Entry(reply),
        );
    }

    fn open(&mut self, req: &Request<'_>, ino: u64, flags: i32, reply: ReplyOpen) {
        debug!("open(ino={}, flags={})", ino, flags);
        let (_access_mask, read, write) = match flags & libc::O_ACCMODE {
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
                if attrs.kind == FileKind::File {
                    let file = match OpenOptions::new()
                        .read(read)
                        .write(write)
                        .open(&attrs.real_path)
                    {
                        Ok(x) => x,
                        Err(_) => {
                            reply.error(libc::EIO);
                            return;
                        }
                    };

                    let file_handle = file.as_raw_fd() as u64;

                    // access mode has already been checked, so we can safely default to a read trace
                    let mode = if write { 'w' } else { 'r' };
                    trace(req.pid(), mode, &["open", &attrs.real_path]);
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
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyData,
    ) {
        debug!(
            "read(ino={}, fh={}, offset={}, size={})",
            ino, fh, offset, size
        );
        match self.attrs.get(&ino) {
            Some(attrs) => {
                if attrs.kind == FileKind::File {
                    let read = |file: File| -> io::Result<Vec<u8>> {
                        let file_size = file.metadata()?.len();
                        let read_size = min(size, file_size.saturating_sub(offset as u64) as u32);
                        let mut buffer = vec![0; read_size as usize];
                        file.read_exact_at(&mut buffer, offset as u64)?;
                        Ok(buffer)
                    };

                    if let Ok(file) = File::open(&attrs.real_path) {
                        match read(file) {
                            Ok(buffer) => {
                                reply.data(&buffer);

                                // trace(req.pid(), 'r', &["read", &attrs.real_path]);
                            }
                            Err(e) => {
                                reply.error(e.raw_os_error().unwrap_or(libc::EIO));
                            }
                        }
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
        _fh: u64,
        offset: i64,
        data: &[u8],
        _write_flags: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyWrite,
    ) {
        debug!(
            "write(ino={}, fh={}, offset={}, size={})",
            ino,
            _fh,
            offset,
            data.len()
        );
        let attrs = match self.attrs.get(&ino) {
            Some(x) => x,
            None => {
                reply.error(libc::ENOENT);
                return;
            }
        };

        let write = || -> io::Result<Metadata> {
            let mut file = OpenOptions::new().write(true).open(&attrs.real_path)?;
            file.seek(SeekFrom::Start(offset as u64))?;
            file.write_all(data)?;
            let metadata = file.metadata()?;
            Ok(metadata)
        };

        match write() {
            Ok(metadata) => {
                // //trace(req.pid(), 'w', &["write", &attrs.real_path]);

                self.attrs
                    .insert(ino, (metadata, attrs.real_path.clone()).into());
                reply.written(data.len() as u32);
            }
            Err(e) => {
                reply.error(e.raw_os_error().unwrap_or(libc::EIO));
            }
        }
    }

    fn release(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        fh: u64,
        flags: i32,
        _lock_owner: Option<u64>,
        _flush: bool,
        reply: ReplyEmpty,
    ) {
        debug!("release(ino={}, fh={}, flags={})", ino, fh, flags);
        reply.ok();
    }

    fn opendir(&mut self, _req: &Request<'_>, ino: u64, flags: i32, reply: ReplyOpen) {
        debug!("opendir(ino={}, flags={})", ino, flags);
        let (_access_mask, read, write) = match flags & libc::O_ACCMODE {
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
                if attrs.kind == FileKind::Directory {
                    let file = match OpenOptions::new()
                        .write(write)
                        .read(read)
                        .open(&attrs.real_path)
                    {
                        Ok(x) => x,
                        Err(_) => {
                            reply.error(libc::EIO);
                            return;
                        }
                    };

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
            if attrs.kind == FileKind::Directory {
                let mut entries = Vec::new();
                for entry in match fs::read_dir(&attrs.real_path) {
                    Ok(x) => x,
                    Err(_) => {
                        reply.error(libc::EIO);
                        return;
                    }
                } {
                    let entry = match entry {
                        Ok(x) => x,
                        Err(_) => {
                            reply.error(libc::EIO);
                            return;
                        }
                    };
                    let metadata = match entry.metadata() {
                        Ok(x) => x,
                        Err(_) => {
                            reply.error(libc::EIO);
                            return;
                        }
                    };
                    let kind = as_file_kind(metadata.mode());
                    let file_name = entry.file_name();
                    let inode = metadata.ino();

                    entries.push((inode, kind, file_name));
                }

                for (i, (inode, kind, name)) in entries.into_iter().enumerate() {
                    if i as i64 >= offset {
                        let full_name = OsStr::new(&name).to_owned();
                        let buffer_full =
                            reply.add(inode, offset + i as i64 + 1, kind.into(), &full_name);
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

    fn releasedir(&mut self, _req: &Request<'_>, ino: u64, fh: u64, flags: i32, reply: ReplyEmpty) {
        debug!("releasedir(ino={}, fh={}, flags={})", ino, fh, flags);
        reply.ok();
    }

    fn statfs(&mut self, req: &Request<'_>, ino: u64, reply: ReplyStatfs) {
        debug!("statfs(ino={})", ino);

        let mut statfs: libc::statvfs = unsafe { std::mem::zeroed() };
        let attrs = match self.attrs.get(&ino) {
            Some(x) => x,
            None => {
                reply.error(libc::ENOENT);
                return;
            }
        };
        let path = Path::new(&attrs.real_path);
        let fd = match path.as_os_str().to_str() {
            Some(x) => x,
            None => {
                reply.error(libc::ENOENT);
                return;
            }
        };

        unsafe {
            libc::statvfs(fd.as_ptr() as *const i8, &mut statfs);
        }

        trace(req.pid(), 'q', &["statfs", &attrs.real_path]);

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

    fn access(&mut self, req: &Request<'_>, ino: u64, mask: i32, reply: ReplyEmpty) {
        debug!("access(ino={}, mask={})", ino, mask);
        match self.attrs.get(&ino) {
            Some(attrs) => {
                if check_access(attrs.uid, attrs.gid, attrs.mode, req.uid(), req.gid(), mask) {
                    reply.ok();
                } else {
                    reply.error(libc::EACCES);
                }
            }
            None => {
                reply.error(libc::ENOENT);
            }
        }
    }

    // No need to implement this, as it will call mknod() and open() instead
    // fn create(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, mode: u32, umask: u32, flags: i32, reply: ReplyCreate)

    fn fallocate(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        _fh: u64,
        _offset: i64,
        _length: i64,
        _mode: i32,
        _reply: ReplyEmpty,
    ) {
        todo!("fallocate()")
    }

    fn copy_file_range(
        &mut self,
        _req: &Request<'_>,
        _ino_in: u64,
        _fh_in: u64,
        _offset_in: i64,
        _ino_out: u64,
        _fh_out: u64,
        _offset_out: i64,
        _len: u64,
        _flags: u32,
        _reply: ReplyWrite,
    ) {
        todo!("copy_file_range()")
    }
}

fn check_access(
    file_uid: u32,
    file_gid: u32,
    file_mode: u32,
    uid: u32,
    gid: u32,
    mut access_mask: i32,
) -> bool {
    // F_OK tests for existence of file
    if access_mask == libc::F_OK {
        return true;
    }

    let file_mode: i32 = Wrapping(file_mode as i32).0;

    // root is allowed to read & write anything
    if uid == 0 {
        // root only allowed to exec if one of the X bits is set
        access_mask &= libc::X_OK;
        access_mask -= access_mask & (file_mode >> 6);
        access_mask -= access_mask & (file_mode >> 3);
        access_mask -= access_mask & file_mode;
        return access_mask == 0;
    }

    if uid == file_uid {
        access_mask -= access_mask & (file_mode >> 6);
    } else if gid == file_gid {
        access_mask -= access_mask & (file_mode >> 3);
    } else {
        access_mask -= access_mask & file_mode;
    }

    return access_mask == 0;
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

fn create_new(path: &str) -> io::Result<File> {
    if !Path::new(&path).exists() {
        return File::create(path);
    }

    return OpenOptions::new().read(true).write(true).open(path);
}

fn get_logger_format() -> impl Fn(&mut Formatter, &Record) -> io::Result<()> {
    return |buf: &mut Formatter, record: &Record| {
        writeln!(buf, "[{}] {}", record.level(), record.args())
    };
}

fn trace(pid: u32, op: char, paths: &[&str]) {
    let path_str = paths.join("|");
    let ppid_result = std::process::Command::new("ps")
        .args(&["-o", "ppid= ", &pid.to_string()])
        .output();
    let ppid: i32 = match ppid_result {
        Ok(output) => {
            let ppid_str = String::from_utf8_lossy(&output.stdout);
            ppid_str.trim().parse::<i32>().unwrap_or_else(|_| -1)
        }
        Err(_) => -1,
    };
    let time = time_from_system_time(&SystemTime::now());
    info!("-> {}: {}|{}|{}|{}", time.0, pid, ppid, op, path_str)
}

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
        // .arg(Arg::new("v").short('v').help("Sets the level of verbosity"))
        .get_matches();

    let level_filter = LevelFilter::Trace;
    let root = matches.get_one::<String>("root").unwrap().to_string();
    let mountpoint = matches.get_one::<String>("mount-point").unwrap();
    let target = Box::new(create_new(format!("{root}/tracer.log").as_str()).unwrap());

    if level_filter >= LevelFilter::Debug {
        File::create("1_parsed_matches").expect("Failed to create 1");
    }

    Builder::new()
        .format(get_logger_format())
        .target(env_logger::Target::Pipe(target))
        .filter_level(level_filter)
        .init();

    if level_filter >= LevelFilter::Debug {
        File::create("2_init_logger").expect("Failed to create 2");
    }

    // unmount filesystem automatically when SIGINT is received
    let (drop_send, drop_recv) = std::sync::mpsc::channel();
    let ctrlc = drop_send.clone();
    let destroy = drop_send.clone();

    // handle graceful shutdown on ctrl-c
    ctrlc::set_handler(move || {
        debug!("Received SIGINT, unmounting filesystem");
        ctrlc.send(()).unwrap();
    })
    .unwrap();

    if level_filter >= LevelFilter::Debug {
        File::create("3_create_channel").expect("Failed to create 3");
    }

    let mount_options = [
        MountOption::AllowOther,
        MountOption::FSName("cairn-fuse".to_string()),
    ];
    let guard = match fuser::spawn_mount2(
        TracerFS::new(root.clone(), destroy),
        mountpoint,
        mount_options.as_slice(),
    ) {
        Ok(x) => x,
        Err(_) => todo!(),
    };

    if level_filter >= LevelFilter::Debug {
        File::create("4_mount").expect("Failed to create 4");
    }

    let () = drop_recv.recv().unwrap();
    let _ = fs::remove_file(".cairn-fuse-ready");
    drop(guard);
}

// todo make sure that all the tests can be run in parallel
#[cfg(test)]
mod tests {
    use super::TracerFS;
    use fuser::MountOption;
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::process::Command;
    use std::{fs, panic, thread};

    const DIRS: [&str; 2] = ["./temp/mnt", "./temp/root"];

    fn run_test<T>(test: T, target: &str) -> ()
    where
        T: FnOnce() -> () + panic::UnwindSafe,
    {
        setup();

        let (send, _) = std::sync::mpsc::channel();
        let mount_options = [
            MountOption::AllowOther,
            MountOption::FSName("cairn-fuse-test".to_string()),
        ];

        let destroy = send.clone();
        thread::spawn(move || {
            let guard = fuser::spawn_mount2(
                TracerFS::new(DIRS[0].to_string(), destroy),
                DIRS[1],
                &mount_options,
            )
            .unwrap();

            drop(guard);
            teardown();
        });

        // wait for the filesystem to be mounted
        // TODO: remove this and wait for session to be mounted
        thread::sleep(std::time::Duration::from_secs(1));

        let result = panic::catch_unwind(|| {
            test();
        });

        send.send(()).unwrap();

        // assert equality of the log files
        match compare_contents(get_current_log_path(target), get_previous_log_path(target)) {
            Ok(are_equal) => {
                assert!(are_equal);
                return;
            }
            Err(_) => {
                // Some of the paths didn't exist, in that case ignore
            }
        }

        // assert that logfile contains the target
        let contents = fs::read_to_string(get_current_log_path(target)).unwrap();
        assert!(contents.contains(target));

        if result.is_ok() {
            let contents = fs::read_to_string(get_current_log_path(target)).unwrap();

            let mut f = OpenOptions::new()
                .write(true)
                .truncate(true)
                .create(true)
                .open(get_previous_log_path(target))
                .unwrap();
            f.write_all(contents.as_bytes()).unwrap();
            f.flush().unwrap();

            fs::remove_file(get_current_log_path(target)).unwrap();

            assert!(true);
            return;
        }

        assert!(false)
    }

    fn setup() {
        for dir in DIRS.iter() {
            Command::new("mkdir").args(&["-p", dir]).output().unwrap();
        }

        // INIT.call_once(|| {
        //     let target = Box::new(create_new(&path).unwrap());
        //     env_logger::Builder::new()
        //         .format(super::get_logger_format())
        //         .target(env_logger::Target::Pipe(target))
        //         .filter_level(log::LevelFilter::Trace)
        //         .is_test(true)
        //         .init();
        // })
    }

    fn teardown() {
        // somehow unmounting is not working as expected so I have to call the umount util manually
        Command::new("umount").args(&[DIRS[0]]).output().unwrap();
        for dir in DIRS.iter() {
            Command::new("rm").args(&["-rf", dir]).output().unwrap();
        }
    }

    fn compare_contents(old: String, new: String) -> std::io::Result<bool> {
        let old_contents = fs::read_to_string(old)?;
        let new_contents = fs::read_to_string(new)?;

        Ok(old_contents == new_contents)

        // // let d = normalized_damerau_levenshtein(&old_contents, &new_contents);
        // // let min_d = std::cmp::min(old_contents.len(), new_contents.len());
        // // let d = hamming(&old_contents, &new_contents).expect("Could not compare contents");
        // // let sim = 1.0 - (d as f64 / min_d as f64);
        // let d = jaro(&old_contents, &new_contents);
        // println!("Distance: {}", d);
        // Ok((1.0 - d) < 0.15)
    }

    fn get_current_log_path(target: &str) -> String {
        return format!("./test-dir/{target}.log");
    }

    fn get_previous_log_path(target: &str) -> String {
        return format!("./test-dir/previous/{target}.log");
    }

    #[test]
    fn init() {
        run_test(|| {}, "init")
    }

    #[test]
    fn touch() {
        run_test(
            || {
                Command::new("touch")
                    .args(&[format!("{}/touch.txt", DIRS[1])])
                    .output()
                    .unwrap();
            },
            "touch",
        )
    }

    #[test]
    fn mkdir() {
        run_test(
            || {
                Command::new("mkdir")
                    .args(&[format!("{}/mkdir", DIRS[1])])
                    .output()
                    .unwrap();
            },
            "mkdir",
        )
    }

    // #[test]
    // fn echo_with_output_redirection() {
    //     run_test(
    //         || {
    //             Command::new("echo")
    //                 .args(&[
    //                     "hello world",
    //                     ">",
    //                     //format!("{}/echo_with_output_redirection.txt", DIRS[1]),
    //                     "/tmp/echo_with_output_redirection.txt",
    //                 ])
    //                 .output()
    //                 .unwrap();
    //         },
    //         "echo_with_output_redirection",
    //     )
    // }
}
