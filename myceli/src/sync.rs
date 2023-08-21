use anyhow::{bail, Result};
use cid::multihash::MultihashDigest;
use cid::Cid;
use ipfs_unixfs::parse_links;
use local_storage::block::StoredBlock;
use local_storage::storage::Storage;
use log::{error, warn};
use messages::cid_list::CompactList;
use messages::{cid_list, Message, SyncMessage};
use parity_scale_codec::Encode;
use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    iter::IntoIterator,
};

type ByMeta = BTreeMap<cid_list::Meta, ToSend>;

#[derive(Default)]
pub(crate) struct Syncer {
    pull: ByMeta,
    push: ByMeta,
    mtu: usize,
    ready: VecDeque<Message>,
    named_pushes: Vec<(Cid, String)>,
}

#[derive(Clone, Copy)]
enum Side {
    Push,
    Pull,
}

impl Syncer {
    pub fn new<I: IntoIterator<Item = (Cid, String)>, J: IntoIterator<Item = Cid>>(
        mtu: usize,
        known_knowns: I,
        known_unknowns: J,
    ) -> Result<Self> {
        let mut result = Self {
            pull: ByMeta::default(),
            push: ByMeta::default(),
            mtu,
            ready: VecDeque::default(),
            named_pushes: Vec::default(),
        };
        for (cid, name) in known_knowns {
            result.will_push(&cid)?;
            result.named_pushes.push((cid, name));
        }
        for cid in known_unknowns {
            result.will_pull(&cid)?;
        }
        Ok(result)
    }
    pub fn push_dag(
        &mut self,
        filename: String,
        root: Cid,
        mut other_blocks: Vec<Cid>,
    ) -> Result<Message> {
        self.will_push(&root)?;
        for cid in &other_blocks {
            self.will_push(cid)?;
        }
        let mut list = CompactList::try_from(&root)?;
        other_blocks.push(root);
        let size = self.mtu - messages::PUSH_OVERHEAD - filename.encoded_size();
        other_blocks.retain(|cid| !list.include(cid, size));
        let result = Message::push(list, filename);
        self.ready.push_front(result.clone());
        let other_msgs = self.push_now(other_blocks)?;
        for msg in other_msgs {
            self.ready.push_back(msg);
        }
        Ok(result)
    }
    pub fn push_now(&mut self, cids: Vec<Cid>) -> anyhow::Result<Vec<Message>> {
        for cid in &cids {
            self.will_push(cid)?;
        }
        let size = self.mtu - messages::PUSH_OVERHEAD;
        let lists = self.sending_now(cids, size, Side::Push)?;
        let result = lists
            .into_iter()
            .map(|l| Message::push(l, "".to_owned()))
            .collect();
        Ok(result)
    }
    pub fn pull_now(&mut self, cids: Vec<Cid>) -> anyhow::Result<Vec<Message>> {
        for cid in &cids {
            self.will_pull(cid)?;
        }
        let lists = self.sending_now(cids, self.mtu, Side::Pull)?;
        let result = lists.into_iter().map(Message::pull).collect();
        Ok(result)
    }
    pub fn will_pull(&mut self, cid: &Cid) -> anyhow::Result<()> {
        Self::add(&mut self.pull, cid)
    }
    pub fn will_push(&mut self, cid: &Cid) -> anyhow::Result<()> {
        Self::add(&mut self.push, cid)
    }
    pub fn stop_pulling(&mut self, cid: &Cid) {
        Self::stop(&mut self.pull, cid);
    }
    pub fn stop_pushing(&mut self, cid: &Cid) {
        Self::stop(&mut self.push, cid);
    }
    pub fn pop_pending_msg(&mut self) -> Option<Message> {
        self.ready.pop_front()
    }
    pub fn build_msg(&mut self) -> Result<()> {
        if let Some((cid, name)) = self.named_pushes.pop() {
            self.push_dag(name, cid, Vec::new())?;
        }
        let pulls = self.pull_now(Vec::default())?;
        let pushs = self.pull_now(Vec::default())?;
        self.ready.extend(pulls.into_iter());
        self.ready.extend(pushs.into_iter());
        Ok(())
    }

    pub fn handle(&mut self, msg: SyncMessage, store: &mut Storage) -> Result<Option<Message>> {
        match msg {
            SyncMessage::Push(pm) => {
                if !pm.check() {
                    bail!("Push message corrupted.");
                }
                self.handle_push(&pm.cids, store)
            }
            SyncMessage::Ack(l) => {
                for cid in &l {
                    self.stop_pushing(&cid);
                }
                Ok(None)
            }
            SyncMessage::Pull(l) => {
                let mut result = None;
                for cid in &l {
                    if let Ok(block) = store.get_block_by_cid(&cid.to_string()) {
                        let m = Message::block(block.data);
                        if result.is_none() {
                            result = Some(m);
                        } else {
                            self.ready.push_back(m);
                        }
                    }
                }
                Ok(result)
            }
            SyncMessage::Block(v) => {
                if let Err(e) = self.handle_block(v, store) {
                    error!("Error while attempting to handle new block: {e:?}");
                }
                Ok(None)
            }
        }
    }

    fn handle_push(
        &mut self,
        cids: &cid_list::CompactList,
        store: &Storage,
    ) -> Result<Option<Message>> {
        let mut ack_resp = cid_list::CompactList::default();
        let mut pull_resp = cid_list::CompactList::default();
        for cid in cids {
            self.stop_pushing(&cid);
            if store.has_cid(&cid) {
                ack_resp.include(&cid, self.mtu);
            } else {
                pull_resp.include(&cid, self.mtu);
                store.ack_cid(&cid);
                // self.algos.insert(cid.hash().code());
                if let Err(e) = self.will_pull(&cid) {
                    error!("Unable to start pulling {cid}: {e}");
                }
            }
        }
        let mut ack_msg = None;
        if !ack_resp.is_empty() {
            if let Some(m) = self.push.get_mut(&ack_resp.shared_traits()) {
                //This isn't a real push, so don't mark those overflow CIDs as having been actually been pushed.
                m.fill_cids(&mut ack_resp, self.mtu, false);
            }
            ack_msg = Some(Message::Sync(SyncMessage::Ack(ack_resp)));
        }
        let resp = if pull_resp.is_empty() {
            ack_msg
        } else {
            if let Some(m) = self.pull.get_mut(&pull_resp.shared_traits()) {
                m.fill_cids(&mut pull_resp, self.mtu, true);
            }
            if let Some(am) = ack_msg {
                self.ready.push_back(am);
            }
            Some(Message::Sync(SyncMessage::Pull(pull_resp)))
        };
        Ok(resp)
    }
    fn handle_block(&mut self, bytes: Vec<u8>, store: &mut Storage) -> Result<()> {
        let mut mhs = HashMap::new();
        let mut hit_cid: Option<Cid> = None;
        for m in self.pull.values() {
            for cid in m.hi.iter().chain(m.lo.iter()) {
                let ch = cid.hash();
                if let Some(bh) = mhs.get(&ch.code()) {
                    if bh == ch {
                        hit_cid = Some(*cid);
                        break;
                    }
                } else if let Ok(c) = cid::multihash::Code::try_from(ch.code()) {
                    let bh = c.digest(&bytes);
                    mhs.insert(ch.code(), bh);
                    if bh == *ch {
                        hit_cid = Some(*cid);
                        break;
                    }
                } else {
                    error!(
                        "Existing CID uses hash algo {} which is not supported.",
                        ch.code()
                    );
                }
            }
        }
        if let Some(cid) = hit_cid {
            let links: Vec<String> = parse_links(&cid, &bytes)?
                .iter()
                .map(|c| c.to_string())
                .collect();
            store.import_block(&StoredBlock {
                cid: cid.to_string(),
                filename: None,
                data: bytes,
                links,
            })?;
            self.stop_pulling(&cid);
        } else {
            warn!("Received a block with no matching CID that was waiting for it.");
        }
        Ok(())
    }

    fn sending_now(
        &mut self,
        mut cids: Vec<Cid>,
        size: usize,
        side: Side,
    ) -> anyhow::Result<Vec<cid_list::CompactList>> {
        let mut result = Vec::new();
        while !cids.is_empty() {
            let cid = cids.pop().unwrap();
            let mut list = cid_list::CompactList::try_from(&cid)?;
            cids.retain(|cid| !list.include(cid, size));
            self.fill(&mut list, size, side, true)?;
            result.push(list);
        }
        Ok(result)
    }
    fn fill(
        &mut self,
        list: &mut CompactList,
        size: usize,
        side: Side,
        mutating: bool,
    ) -> Result<()> {
        let meta = if let Some(cid) = list.into_iter().next() {
            cid.try_into()?
        } else {
            bail!("Requested to fill an empty CID list");
        };
        let state_map = match side {
            Side::Push => &mut self.push,
            Side::Pull => &mut self.pull,
        };
        if let Some(state) = state_map.get_mut(&meta) {
            state.fill_cids(list, size, mutating);
        }
        Ok(())
    }
    fn add(side: &mut ByMeta, cid: &Cid) -> anyhow::Result<()> {
        let s = side.entry(cid.try_into()?).or_default();
        s.hi.push_front(*cid);
        s.lo.push_back(*cid);
        Ok(())
    }
    fn stop(side: &mut ByMeta, cid: &Cid) -> bool {
        if let Ok(Some(side)) = cid_list::Meta::try_from(cid).map(|m| side.get_mut(&m)) {
            for q in [&mut side.hi, &mut side.lo] {
                if let Some(index) = q.iter().position(|c| c == cid) {
                    q.remove(index);
                    return true;
                }
            }
        }
        false
    }
}

#[derive(Default)]
struct ToSend {
    hi: VecDeque<Cid>,
    lo: VecDeque<Cid>,
}

impl ToSend {
    fn fill_cids(&mut self, list: &mut cid_list::CompactList, size: usize, move_on: bool) {
        for (q, and_remove) in [(&mut self.hi, true), (&mut self.lo, false)] {
            while !q.is_empty() && list.include(q.iter().next().unwrap(), size) {
                if !move_on {
                    continue;
                }
                if let Some(x) = q.pop_front() {
                    if !and_remove {
                        q.push_back(x);
                    }
                }
            }
        }
    }
}
