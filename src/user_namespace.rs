use nix::unistd::Pid;
use procfs;
use std::fs::File;
use std::io::BufReader;
use std::io::prelude::*;
use types::{Result, Error};

#[derive(Clone, Copy, Debug)]
struct Extent {
    first: u32,
    lower_first: u32,
    count: u32,
}

#[derive(Clone, Copy, Debug)]
enum Kind {
    UidMap,
    GidMap,
}

#[derive(Clone, Copy, Debug)]
pub struct IdMap {
    kind: Kind,
    nr_extents: usize,
    extent: [Extent; 5], // 5 == UID_GID_MAP_MAX_EXTENTS
}

impl Into<&'static str> for Kind {
    fn into(self) -> &'static str {
        match self {
            Kind::UidMap => "uid_map",
            Kind::GidMap => "gid_map",
        }
    }
}
impl From<&'static str> for Kind {
    fn from(s: &'static str) -> Kind {
        match s {
            "uid_map" => Kind::UidMap,
            _ => Kind::GidMap,
        }
    }
}

const DEFAULT_EXTENT: Extent = Extent {
    first: 0,
    lower_first: 0,
    count: 4_294_967_295,
};

pub const DEFAULT_ID_MAP: IdMap = IdMap {
    kind: Kind::UidMap,
    nr_extents: 1,
    extent: [DEFAULT_EXTENT; 5],
};

impl IdMap {
    fn _new_from_pid(pid: Pid, kind: Kind) -> Result<IdMap> {
        let what: &str = kind.into();
        let path = procfs::get_path().join(pid.to_string()).join(what);
        let f = tryfmt!(File::open(&path), "failed to open {}", path.display());
        let buf_reader = BufReader::new(f);
        let mut id_map = IdMap {
            kind,
            nr_extents: 0,
            extent: [DEFAULT_EXTENT; 5],
        };
        for line in buf_reader.lines() {
            let line = tryfmt!(line, "failed to read {}", path.display());
            let cols: Vec<&str> = line.split_whitespace().collect();
            assert!(cols.len() == 3);
            assert!(id_map.nr_extents < id_map.extent.len());
            id_map.extent[id_map.nr_extents] = Extent {
                first: tryfmt!(
                    cols[0].parse::<u32>(),
                    "invalid id value in {}: {}",
                    what,
                    line
                ),
                lower_first: tryfmt!(
                    cols[1].parse::<u32>(),
                    "invalid id value in {}: {}",
                    what,
                    line
                ),
                count: tryfmt!(
                    cols[2].parse::<u32>(),
                    "invalid id value in {}: {}",
                    what,
                    line
                ),
            };
            id_map.nr_extents += 1;
        }
        Ok(id_map)
    }

    pub fn new_from_pid(pid: Pid) -> Result<(IdMap, IdMap)> {
        let uid_map = tryfmt!(
            IdMap::_new_from_pid(pid, Kind::UidMap),
            "failed to read uid_map"
        );
        let gid_map = tryfmt!(
            IdMap::_new_from_pid(pid, Kind::GidMap),
            "failed to read uid_map"
        );
        Ok((uid_map, gid_map))
    }

    pub fn map_id_down(&self, id: u32) -> u32 {
        for idx in 0..self.nr_extents {
            let first = self.extent[idx].first;
            let last = first + self.extent[idx].count - 1;
            if id >= first && id <= last {
                return id - first + self.extent[idx].lower_first;
            }
        }
        // FIXME: should be replaced by overflowgid/overflowuid
        65_534
    }
    pub fn map_id_up(&self, id: u32) -> u32 {
        for idx in 0..self.nr_extents {
            let first = self.extent[idx].lower_first;
            let last = first + self.extent[idx].count - 1;
            if id >= first && id <= last {
                return id - first + self.extent[idx].first;
            }
        }
        // FIXME: should be replaced by overflowgid/overflowuid
        65_534
    }
}
