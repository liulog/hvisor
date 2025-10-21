// Copyright (c) 2025 Syswonder
// hvisor is licensed under Mulan PSL v2.
// You can use this software according to the terms and conditions of the Mulan PSL v2.
// You may obtain a copy of Mulan PSL v2 at:
//     http://license.coscl.org.cn/MulanPSL2
// THIS SOFTWARE IS PROVIDED ON AN "AS IS" BASIS, WITHOUT WARRANTIES OF ANY KIND, EITHER
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO NON-INFRINGEMENT, MERCHANTABILITY OR
// FIT FOR A PARTICULAR PURPOSE.
// See the Mulan PSL v2 for more details.
//
// Syswonder Website:
//      https://www.syswonder.org
//
// Authors:
//      Jingyu Liu <liujingyu24s@ict.ac.cn>

#![allow(dead_code)]

use modular_bitfield::specifiers::{
    B1, B10, B118, B14, B16, B18, B2, B20, B24, B3, B32, B52, B6, B62, B64, B8,
};
use modular_bitfield::{bitfield, Specifier};

#[derive(Specifier)]
#[bits = 7]
pub enum OpCode {
    IotInval = 1,
    IoFence = 2,
    IoDir = 3,
    Ats = 4,
}

#[derive(Specifier)]
#[bits = 3]
pub enum IotInvalFunc {
    Vma = 0,
    Gvma = 1,
}

#[derive(Specifier)]
#[bits = 3]
pub enum IoFenceFunc {
    C = 0,
}

#[derive(Specifier)]
#[bits = 3]
pub enum IoDirFunc {
    InvalDdt = 0,
    InvalPdt = 1,
}

#[derive(Specifier)]
#[bits = 3]
pub enum AtsFunc {
    Inval = 0,
    Prgr = 1,
}

/// Base Format of Command Queue Entry
#[bitfield(bits = 128)]
pub struct CqEntry {
    pub opcode: OpCode,
    pub func3: B3,
    pub operands: B118,
}

/// IOTINVAL Command Queue Entry
#[rustfmt::skip]
#[bitfield(bits = 128)]
pub struct CqEntryIotinval {
    pub opcode: OpCode,      // IOTINVAL: 0x1
    pub func3: IotInvalFunc, // VMA: 0x0, GVMA: 0x1
    pub av: B1,
    #[skip] rsvd1: B1,
    pub pscid: B20,
    pub pscv: B1,
    pub gv: B1,
    #[skip] rsvd2: B10,
    pub gscid: B16,
    #[skip] rsvd3: B14,
    pub addr: B52,
    #[skip] rsvd4: B2,
}

/// IOFENCE Command Queue Entry
#[rustfmt::skip]
#[bitfield(bits = 128)]
pub struct CqEntryIofence {
    pub opcode: OpCode,     // IOFENCE: 0x2
    pub func3: IoFenceFunc, // C: 0x0
    pub av: B1,
    pub wsi: B1,
    pub pr: B1,
    pub pw: B1,
    #[skip] rsvd1: B18,
    pub data: B32,
    pub addr: B62,
    #[skip] rsvd2: B2,
}

/// IODIR Command Queue Entry
#[rustfmt::skip]
#[bitfield(bits = 128)]
pub struct CqEntryIodir {
    pub opcode: OpCode,    // IODIR: 0x3
    pub func3: IoDirFunc, // INVAL_DDT: 0x0, INVAL_PDT: 0x1
    #[skip] rsvd1: B2,
    pub pid: B20,
    #[skip] rsvd2: B1,
    pub dv: B1,
    #[skip] rsvd3: B6,
    pub did: B24,
    #[skip] rsvd4: B64,
}

/// IODIR Command Queue Entry
#[rustfmt::skip]
#[bitfield(bits = 128)]
pub struct CqEntryAts {
    pub opcode: OpCode, // ATS: 0x4
    pub func3: AtsFunc, // INVAL: 0x0, PRGR: 0x1
    #[skip] rsvd1: B2,
    pub pid: B20,
    pub pv: B1,
    pub dsv: B1,
    #[skip] rsvd2: B6,
    pub rid: B16,
    pub dseg: B8,
    pub payload: B64,
}

// --------------------
// CqEntry <-> CqEntryIotinval
// --------------------
impl From<CqEntry> for CqEntryIotinval {
    fn from(entry: CqEntry) -> Self {
        CqEntryIotinval::from_bytes(entry.into_bytes())
    }
}

impl From<CqEntryIotinval> for CqEntry {
    fn from(iotval: CqEntryIotinval) -> Self {
        CqEntry::from_bytes(iotval.into_bytes())
    }
}

// --------------------
// CqEntry <-> CqEntryIofence
// --------------------
impl From<CqEntry> for CqEntryIofence {
    fn from(entry: CqEntry) -> Self {
        CqEntryIofence::from_bytes(entry.into_bytes())
    }
}

impl From<CqEntryIofence> for CqEntry {
    fn from(iofence: CqEntryIofence) -> Self {
        CqEntry::from_bytes(iofence.into_bytes())
    }
}

// --------------------
// CqEntry <-> CqEntryIodir
// --------------------
impl From<CqEntry> for CqEntryIodir {
    fn from(entry: CqEntry) -> Self {
        CqEntryIodir::from_bytes(entry.into_bytes())
    }
}

impl From<CqEntryIodir> for CqEntry {
    fn from(iodir: CqEntryIodir) -> Self {
        CqEntry::from_bytes(iodir.into_bytes())
    }
}

// --------------------
// CqEntry <-> CqEntryAts
// --------------------
impl From<CqEntry> for CqEntryAts {
    fn from(entry: CqEntry) -> Self {
        CqEntryAts::from_bytes(entry.into_bytes())
    }
}

impl From<CqEntryAts> for CqEntry {
    fn from(ats: CqEntryAts) -> Self {
        CqEntry::from_bytes(ats.into_bytes())
    }
}

pub enum Command {
    IotInval(CqEntryIotinval),
    IoFence(CqEntryIofence),
    IoDir(CqEntryIodir),
    Ats(CqEntryAts), // Note: ATS not implemented yet.
    Unknown,
}

impl CqEntry {
    pub fn parse(self) -> Command {
        match self.opcode_or_err() {
            Ok(OpCode::IotInval) => Command::IotInval(self.into()),
            Ok(OpCode::IoFence) => Command::IoFence(self.into()),
            Ok(OpCode::IoDir) => Command::IoDir(self.into()),
            Ok(OpCode::Ats) => Command::Ats(self.into()),
            Err(_) => Command::Unknown,
        }
    }
}
