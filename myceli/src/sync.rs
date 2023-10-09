use anyhow::{bail, Result};
use cid::multihash::MultihashDigest;
use cid::{Cid, Version};
use ipfs_unixfs::{codecs::Codec, parse_links};
use libipld::{prelude::Codec as _, Ipld, IpldCodec};
use local_storage::block::StoredBlock;
use local_storage::storage::Storage;
use log::{debug, error, info, warn};
use messages::cid_list::CompactList;
use messages::{cid_list, Message, SyncMessage, PUSH_OVERHEAD};
use parity_scale_codec::Encode;
use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    iter,
    iter::IntoIterator,
};

type ByMeta = BTreeMap<cid_list::Meta, ToSend>;

#[derive(Default)]
pub(crate) struct Syncer {
    pull: ByMeta,
    push: ByMeta,
    mtu: usize,
    ready: VecDeque<Message>,
    pending_names: Vec<(Cid, String)>,
    names: HashMap<Cid, String>,
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
            pending_names: Vec::default(),
            names: HashMap::default(),
        };
        for (cid, name) in known_knowns {
            result.will_push(&cid)?;
            if !name.is_empty() {
                result.names.insert(cid, name.clone());
                result.pending_names.push((cid, name));
            }
        }
        for cid in known_unknowns {
            result.will_pull(&cid)?;
        }
        Ok(result)
    }
    pub fn push_dag(&mut self, root: &StoredBlock, later: bool) -> Result<Option<Message>> {
        debug!("push_dag({root:?},later={later}");
        let root_cid = Cid::try_from(root.cid.as_str())?;
        self.will_push(&root_cid)?;
        let mut linked_cids = Vec::default();
        for link in &root.links {
            let link_cid = Cid::try_from(link.as_str())?;
            self.will_push(&link_cid)?;
            linked_cids.push(link_cid)
        }
        let mut root_push = None;
        if let Some(name) = &root.filename {
            self.pending_names.push((root_cid, name.clone()));
            self.names.insert(root_cid, name.clone());
            let mut list = CompactList::try_from(&root_cid)?;
            let size = self.mtu - messages::PUSH_OVERHEAD - root.filename.encoded_size();
            linked_cids.retain(|cid| !list.include(cid, size));
            let root_msg = Message::push(list, name.clone())?;
            if later {
                self.ready.push_front(root_msg);
            } else {
                root_push = Some(root_msg)
            }
        } else {
            linked_cids.push(root_cid);
        }
        let other_msgs = self.push_now(linked_cids)?;
        for msg in other_msgs {
            self.ready.push_front(msg);
        }
        Ok(root_push)
    }
    pub fn push_now(&mut self, cids: Vec<Cid>) -> anyhow::Result<Vec<Message>> {
        for cid in &cids {
            self.will_push(cid)?;
        }
        let size = self.mtu - messages::PUSH_OVERHEAD;
        let lists = self.sending_now(cids, size, Side::Push)?;
        let result = lists
            .into_iter()
            .flat_map(|l| Message::push(l, "".to_owned()).ok())
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
    pub fn pop_pending_msg(&mut self, store: &Storage) -> Option<Message> {
        match self.ready.pop_front() {
            Some(Message::Sync(SyncMessage::Pull(l))) => {
                let mut m = CompactList::default();
                for c in &l {
                    if store.has_cid(&c) {
                        debug!("Refusing to pull {c:?} which we already have.");
                    } else {
                        m.include(&c, usize::MAX);
                    }
                }
                Some(Message::Sync(SyncMessage::Pull(m)))
            }
            o => o,
        }
    }
    pub fn build_msg(&mut self, store: &mut Storage) -> Result<()> {
        if let Some((cid, name)) = self.pending_names.pop() {
            let mut list = cid_list::CompactList::try_from(&cid)?;
            self.fill(
                &mut list,
                self.mtu - PUSH_OVERHEAD - name.encoded_size(),
                Side::Push,
            )?;
            if let Ok(m) = Message::push(list, name) {
                info!("Build: Will push DAG {m:?}");
                self.ready.push_back(m);
            }
            return Ok(());
        }
        if let Some(c) = self
            .pull
            .iter_mut()
            .flat_map(|(_, s)| s.hi.pop_front())
            .next()
        {
            if !store.has_cid(&c) {
                let v = self.pull_now(vec![c])?;
                info!("Build: Will pull {v:?}");
                self.ready.extend(v.into_iter());
            }
        }
        if let Some(c) = self
            .push
            .iter_mut()
            .flat_map(|(_, s)| s.hi.pop_front())
            .next()
        {
            let v = self.push_now(vec![c])?;
            info!("Build: Will push {v:?}");
            self.ready.extend(v.into_iter());
        }
        if self.ready.is_empty() {
            let size = self.mtu - PUSH_OVERHEAD;
            for q in self.push.values_mut() {
                let mut list = cid_list::CompactList::default();
                if let Some(inc_cnt) = q.lo.iter().position(|c| !list.include(c, size)) {
                    info!(
                        "Lo-pri push: {list:?} from {} avail. Rotating {inc_cnt} from {:?}",
                        q.lo.len(),
                        &q.lo
                    );
                    q.lo.rotate_left(inc_cnt);
                    info!("... to {:?}", &q.lo);
                }
                if let Ok(m) = Message::push(list, String::default()) {
                    self.ready.push_back(m);
                }
            }
            let size = self.mtu;
            for q in self.pull.values_mut() {
                let mut list = cid_list::CompactList::default();
                if let Some(inc_cnt) = q.lo.iter().position(|c| !list.include(c, size)) {
                    q.lo.rotate_left(inc_cnt);
                }
                if !list.is_empty() {
                    info!("Lo-pri pull: {list:?} from {} avail.", q.lo.len());
                    self.ready.push_back(Message::pull(list));
                }
            }
        }
        Ok(())
    }

    pub fn handle(&mut self, msg: SyncMessage, store: &mut Storage) -> Result<Option<Message>> {
        debug!("Sync::handle({msg:?})");
        match msg {
            SyncMessage::Push(pm) => {
                if !pm.check() {
                    bail!("Push message corrupted.");
                }
                self.handle_push(&pm.first_cid_name, pm.cids.into_iter(), store)
            }
            SyncMessage::Ack(l) => {
                for cid in &l {
                    debug!("Remote ACKed {cid}");
                    self.stop_pushing(&cid);
                }
                Ok(None)
            }
            SyncMessage::Pull(l) => {
                let mut result = None;
                for cid in &l {
                    info!("Remote requested block for {cid}");
                    if let Ok(block) = store.get_block_by_cid(&cid.to_string()) {
                        let len = block.data.len();
                        let m = Message::block(block.data);
                        if m.encoded_size() > self.mtu {
                            warn!("Blocks too large to fit {} exist, example: {cid} is {} bytes leading to a message of {}B.", self.mtu, len, m.encoded_size());
                        }
                        if result.is_none() {
                            result = Some(m);
                        } else {
                            self.ready.push_front(m);
                        }
                    }
                }
                Ok(result)
            }
            SyncMessage::Block(v) => self.handle_block(v, store),
        }
    }

    fn handle_push<C: Iterator<Item = Cid> + Clone>(
        &mut self,
        name: &str,
        mut cids: C,
        store: &Storage,
    ) -> Result<Option<Message>> {
        let mut ack_resp = cid_list::CompactList::default();
        let mut pull_resp = cid_list::CompactList::default();
        for cid in cids.clone() {
            self.stop_pushing(&cid);
            if store.has_cid(&cid) {
                self.stop_pulling(&cid);
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
        let root = if let Some(cid) = cids.next() {
            cid.to_string()
        } else {
            bail!("No CIDs in a Push");
        };
        store.set_name(&root, name);
        debug!(
            "DAG pushed to me: {name}={root}. Will ack {} CIDs and pull {}",
            ack_resp.into_iter().count(),
            pull_resp.into_iter().count()
        );
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
                self.ready.push_front(am);
            }
            Some(Message::Sync(SyncMessage::Pull(pull_resp)))
        };
        Ok(resp)
    }
    fn handle_block(&mut self, bytes: Vec<u8>, store: &mut Storage) -> Result<Option<Message>> {
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
        let mut result = Ok(None);
        if let Some(cid) = hit_cid {
            let links = parse_links(&cid, &bytes)?;
            if !links.is_empty() {
                result = self.handle_push("", links.iter().chain(iter::once(&cid)).cloned(), store);
            }
            let links = links.iter().map(|c| c.to_string()).collect();
            let filename = self.names.get(&cid).cloned();
            debug!("Hit CID ({cid}) I was waiting on, importing it named {filename:?} with links {links:?}");
            store.import_block(&StoredBlock {
                cid: cid.to_string(),
                filename,
                data: bytes,
                links,
            })?;
            self.stop_pulling(&cid);
        } else {
            let hash = cid::multihash::Code::Sha2_256.digest(&bytes);
            let mut cids = Vec::default();
            if IpldCodec::DagPb
                .references::<Ipld, _>(bytes.as_slice(), &mut cids)
                .is_ok()
            {
                if !cids.is_empty() {
                    result = self.handle_push("", cids.iter().cloned(), store);
                }
                let cid = Cid::new(Version::V1, Codec::DagPb.into(), hash)?;
                let cid_s = cid.to_string();
                let links = cids.into_iter().map(|c| c.to_string()).collect();
                warn!("Received a block with no matching CID that was waiting for it. {} bytes in block. Importing it as a DAG-PB block, CID={cid_s}, links={links:?}. remaining dangling CIDs: {:?}", bytes.len(), self.pull);
                store.import_block(&StoredBlock {
                    cid: cid_s,
                    filename: None,
                    data: bytes,
                    links,
                })?;
            } else {
                let cid = Cid::new(Version::V1, Codec::Raw.into(), hash)?;
                let cid_s = cid.to_string();
                warn!("Received a block with no matching CID that was waiting for it. {} bytes in block. Importing it as a RAW block, CID={cid_s}. remaining dangling CIDs: {:?}", bytes.len(), self.pull);
                store.import_block(&StoredBlock {
                    cid: cid_s,
                    filename: None,
                    data: bytes,
                    links: Vec::new(),
                })?;
            }
        }
        result
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
            self.fill(&mut list, size, side)?;
            result.push(list);
        }
        Ok(result)
    }
    fn fill(&mut self, list: &mut CompactList, size: usize, side: Side) -> Result<()> {
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
            state.fill_cids(list, size, true);
        }
        Ok(())
    }
    fn add(side: &mut ByMeta, cid: &Cid) -> anyhow::Result<()> {
        let s = side.entry(cid.try_into()?).or_default();
        s.hi.push_back(*cid);
        s.lo.push_back(*cid);
        Ok(())
    }
    fn stop(side: &mut ByMeta, cid: &Cid) -> bool {
        if let Ok(Some(side)) = cid_list::Meta::try_from(cid).map(|m| side.get_mut(&m)) {
            if let Some(index) = side.hi.iter().position(|c| c == cid) {
                side.hi.remove(index);
                return true;
            }
            let ack_count = side.acked.entry(*cid).or_default();
            if let Some(a) = ack_count.checked_add(1) {
                *ack_count = a;
                true
            } else if let Some(index) = side.lo.iter().position(|c| c == cid) {
                side.lo.remove(index);
                true
            } else {
                side.acked.remove(cid).is_some()
            }
        } else {
            false
        }
    }
}

#[derive(Default, Debug)]
struct ToSend {
    hi: VecDeque<Cid>,
    lo: VecDeque<Cid>,
    acked: HashMap<Cid, u8>,
}

impl ToSend {
    fn fill_cids(&mut self, list: &mut cid_list::CompactList, size: usize, mutate: bool) {
        for q in [&mut self.hi, &mut self.lo] {
            if let Some(idx) = q.iter().position(|c| !list.include(c, size)) {
                if mutate {
                    q.rotate_left(idx);
                }
            }
        }
    }
}
