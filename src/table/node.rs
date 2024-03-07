use std::ops::Range;

use bytes::BytesMut;

use crate::{
    btree::slot::Increment,
    page::{PageBuf, PageId, PAGE_SIZE},
    storable::Storable,
};

/*
    TablePage:
    NextPageID | NumTuples | NumDeletedTuples | Slots | Free | Tuples

    Slot:
    TupleInfo

    Tuple:
    RId | Data
*/

#[derive(Debug, PartialEq, Copy, Clone, PartialOrd, Eq, Ord)]
pub struct RId {
    pub page_id: PageId,
    pub slot_id: u32,
}

#[derive(Debug, PartialEq, Clone, PartialOrd, Eq, Ord)]
pub struct Tuple {
    pub r_id: RId,
    pub data: BytesMut,
}

impl Increment for Tuple {
    fn increment(&mut self) {
        todo!()
    }

    fn next(&self) -> Self {
        todo!()
    }
}

// TODO
impl Storable for Tuple {
    const SIZE: usize = 0;

    type ByteArray = [u8; 0];

    fn into_bytes(self) -> Self::ByteArray {
        todo!()
    }

    fn from_bytes(bytes: &[u8]) -> Self {
        todo!()
    }

    fn write_to(&self, dst: &mut [u8], pos: usize) {
        todo!()
    }
}

impl Storable for RId {
    const SIZE: usize = 0;

    type ByteArray = [u8; 0];

    fn into_bytes(self) -> Self::ByteArray {
        todo!()
    }

    fn from_bytes(bytes: &[u8]) -> Self {
        todo!()
    }

    fn write_to(&self, dst: &mut [u8], pos: usize) {
        todo!()
    }
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub struct TupleMeta {
    pub deleted: bool,
}

impl From<&[u8]> for TupleMeta {
    fn from(value: &[u8]) -> Self {
        let deleted = u8::from_be_bytes(value[0..1].try_into().unwrap()) > 1;

        Self { deleted }
    }
}

pub const OFFSET: Range<usize> = 0..4;
pub const LEN: Range<usize> = 4..8;
pub const META: Range<usize> = 8..Slot::SIZE;

#[derive(Debug, PartialEq, Copy, Clone)]
pub struct Slot {
    offset: u32,
    len: u32,
    meta: TupleMeta,
}

impl From<&[u8]> for Slot {
    fn from(buf: &[u8]) -> Self {
        let offset = u32::from_be_bytes(buf[OFFSET].try_into().unwrap());
        let len = u32::from_be_bytes(buf[LEN].try_into().unwrap());
        let meta = TupleMeta::from(&buf[META]);

        Self { offset, len, meta }
    }
}

type TupleInfoBuf = [u8; Slot::SIZE];
impl From<&Slot> for TupleInfoBuf {
    fn from(value: &Slot) -> Self {
        let mut ret = [0; Slot::SIZE];

        ret[OFFSET].copy_from_slice(&value.offset.to_be_bytes());
        ret[LEN].copy_from_slice(&value.len.to_be_bytes());
        ret[META].copy_from_slice(&[value.meta.deleted as u8]);

        ret
    }
}

impl Slot {
    const SIZE: usize = 9;
}

pub const NEXT_PAGE_ID: Range<usize> = 0..4;
pub const TUPLES_LEN: Range<usize> = 4..8;
pub const DELETED_TUPLES_LEN: Range<usize> = 8..12;
pub const SLOTS_START: usize = 12;

#[derive(Debug, PartialEq)]
pub struct Node {
    page_start: *mut u8,
    pub next_page_id: PageId,
    tuples_len: u32,
    deleted_tuples_len: u32,
    slots: Vec<Slot>,
}

impl From<&PageBuf> for Node {
    fn from(buf: &PageBuf) -> Self {
        let page_start = buf.as_ptr() as *mut u8;
        let next_page_id = i32::from_be_bytes(buf[NEXT_PAGE_ID].try_into().unwrap());
        let tuples_len = u32::from_be_bytes(buf[TUPLES_LEN].try_into().unwrap());
        let deleted_tuples_len = u32::from_be_bytes(buf[DELETED_TUPLES_LEN].try_into().unwrap());

        let mut slots = Vec::new();
        const SLOT_SIZE: usize = Slot::SIZE;
        let left = &buf[SLOTS_START..];
        let mut from = 0;
        let mut rem = tuples_len;
        while rem > 0 {
            let bytes = &left[from..from + SLOT_SIZE];
            let slot = Slot::from(bytes);
            slots.push(slot);
            from = from + SLOT_SIZE;
            rem -= 1;
        }

        Self {
            page_start,
            // next_page_id: if next_page_id == 0 { -1 } else { next_page_id },
            next_page_id,
            tuples_len,
            deleted_tuples_len,
            slots,
        }
    }
}

impl From<&Node> for PageBuf {
    fn from(table: &Node) -> Self {
        let mut ret: PageBuf = [0; PAGE_SIZE];

        ret[NEXT_PAGE_ID].copy_from_slice(&table.next_page_id.to_be_bytes());
        ret[TUPLES_LEN].copy_from_slice(&table.tuples_len.to_be_bytes());
        ret[DELETED_TUPLES_LEN].copy_from_slice(&table.deleted_tuples_len.to_be_bytes());

        const SLOT_SIZE: usize = Slot::SIZE;
        let mut from = SLOTS_START;
        for slot in &table.slots {
            let slot = TupleInfoBuf::from(slot);
            ret[from..from + SLOT_SIZE].copy_from_slice(&slot);
            from += SLOT_SIZE;
        }

        let offset = match table.slots.last() {
            Some(o) => o.offset as usize,
            None => return ret,
        };
        assert!(offset < PAGE_SIZE);

        unsafe {
            let tuples_ptr = table.page_start.add(offset);
            let tuples = std::slice::from_raw_parts(tuples_ptr, PAGE_SIZE - offset);
            ret[offset..].copy_from_slice(tuples);
        }

        ret
    }
}

impl Node {
    const HEADER_SIZE: usize = 12;

    pub fn len(&self) -> u32 {
        self.tuples_len
    }

    pub fn next_tuple_offset(&self, tuple_data: &BytesMut) -> Option<usize> {
        let offset = match self.slots.last() {
            Some(slot) => slot.offset as usize,
            None => PAGE_SIZE,
        };

        let tuple_offset = offset - tuple_data.len();

        // Ensure tuple isn't written over header/slots
        let size = Self::HEADER_SIZE + Slot::SIZE * (self.tuples_len as usize + 1);
        if tuple_offset < size {
            return None;
        }

        Some(tuple_offset)
    }

    pub fn insert(&mut self, tuple_data: &BytesMut, meta: &TupleMeta) -> Option<u32> {
        let offset = self.next_tuple_offset(tuple_data)?;
        let slot_id = self.tuples_len;
        self.slots.push(Slot {
            offset: offset as u32,
            len: tuple_data.len() as u32,
            meta: *meta,
        });
        self.tuples_len += 1;

        unsafe {
            let tuples_ptr = self.page_start.add(offset);
            let tuples = std::slice::from_raw_parts_mut(tuples_ptr, PAGE_SIZE - offset);
            tuples[..tuple_data.len()].copy_from_slice(&tuple_data);
        }

        Some(slot_id)
    }

    pub fn get(&self, r_id: &RId) -> Option<(TupleMeta, Tuple)> {
        let slot_id = r_id.slot_id;
        if slot_id > self.tuples_len {
            todo!()
        }

        let Slot { offset, len, meta } = self.slots[slot_id as usize];
        let mut tuple = Tuple {
            r_id: *r_id,
            data: BytesMut::zeroed(len as usize),
        };

        unsafe {
            let tuple_ptr = self.page_start.add(offset as usize);
            let tuple_data = std::slice::from_raw_parts(tuple_ptr, len as usize);
            tuple.data[..].copy_from_slice(tuple_data);
        }

        Some((meta, tuple))
    }
}

#[cfg(test)]
mod test {
    use bytes::BytesMut;

    use crate::{
        page::{PageBuf, PAGE_SIZE},
        table::node::{Node, RId, Slot, Tuple, TupleMeta},
    };

    #[test]
    fn test_from() {
        let mut buf = [0; PAGE_SIZE];

        let tuple_a = std::array::from_fn::<u8, 10, _>(|i| (i * 2) as u8);
        let tuple_b = std::array::from_fn::<u8, 15, _>(|i| (i * 3) as u8);

        buf[PAGE_SIZE - 10..].copy_from_slice(&tuple_a);
        buf[PAGE_SIZE - 25..PAGE_SIZE - 10].copy_from_slice(&tuple_b);

        let mut table = Node {
            page_start: buf.as_mut_ptr(),
            next_page_id: 10,
            tuples_len: 2,
            deleted_tuples_len: 0,
            slots: vec![
                Slot {
                    offset: (PAGE_SIZE - 10) as u32,
                    len: 10,
                    meta: TupleMeta { deleted: false },
                },
                Slot {
                    offset: (PAGE_SIZE - 25) as u32,
                    len: 15,
                    meta: TupleMeta { deleted: false },
                },
            ],
        };

        let bytes = PageBuf::from(&table);

        let mut table2 = Node::from(&bytes);

        let offset = table.slots.last().unwrap().offset as usize;
        let tuples = unsafe {
            let tuples_ptr = table2.page_start.add(offset);
            std::slice::from_raw_parts(tuples_ptr, PAGE_SIZE - offset)
        };
        assert_eq!(&tuples[0..15], &tuple_b);
        assert_eq!(&tuples[15..], &tuple_a);

        table.page_start = std::ptr::null_mut();
        table2.page_start = std::ptr::null_mut();

        assert_eq!(table, table2);
    }

    #[test]
    fn test_insert() {
        let mut buf = [0; PAGE_SIZE];

        let mut table = Node {
            page_start: buf.as_mut_ptr(),
            next_page_id: 0,
            tuples_len: 0,
            deleted_tuples_len: 0,
            slots: Vec::new(),
        };

        let meta = TupleMeta { deleted: false };

        let r_id_a = RId {
            page_id: 0,
            slot_id: 0,
        };
        let tuple_a = BytesMut::from(&std::array::from_fn::<u8, 10, _>(|i| (i * 2) as u8)[..]);

        let r_id_b = RId {
            page_id: 0,
            slot_id: 1,
        };
        let tuple_b = BytesMut::from(&std::array::from_fn::<u8, 15, _>(|i| (i * 3) as u8)[..]);

        table.insert(&tuple_a, &meta);
        table.insert(&tuple_b, &meta);

        let (_, have_a) = table.get(&r_id_a).unwrap();
        let (_, have_b) = table.get(&r_id_b).unwrap();
        assert_eq!(
            Tuple {
                data: tuple_a,
                r_id: r_id_a
            },
            have_a
        );
        assert_eq!(
            Tuple {
                data: tuple_b,
                r_id: r_id_b
            },
            have_b
        )
    }
}
