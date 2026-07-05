//! The room registry (spec §6.2): `RoomCode → room actor`, the Create/Join routing, the
//! per-IP connection/Join-attempt limiter (the room-code brute-force guard), and the
//! deregistration a GC'd room calls on exit. A shared `Arc<Registry>` behind brief
//! `std::sync::Mutex`es — the locks guard only routing metadata (never game state, which
//! each room actor solely owns), and no `.await` is ever held across one.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use async_channel::Sender;

use dooduel_core::game::Config;

use crate::room::{RoomMsg, room_task};
use crate::util::{self, TokenBucket};

/// The per-IP limiter (spec §6.2). A generous burst so a legitimate flurry of
/// reconnects passes; a modest sustained refill throttles room-code brute-force (each
/// guess is a fresh connection = one token).
const IP_BURST: f64 = 20.0;
const IP_REFILL_PER_SEC: f64 = 4.0;

/// The live-room routing table + the per-IP limiter.
pub struct Registry {
    rooms: Mutex<HashMap<String, Sender<RoomMsg>>>,
    ip: Mutex<HashMap<IpAddr, TokenBucket>>,
    start: Instant,
}

impl Registry {
    /// A fresh, empty registry.
    pub fn new() -> Arc<Self> {
        Arc::new(Registry {
            rooms: Mutex::new(HashMap::new()),
            ip: Mutex::new(HashMap::new()),
            start: Instant::now(),
        })
    }

    /// Consume one per-IP token (spec §6.2). `false` ⇒ the attempt is rate-limited.
    pub fn check_ip(&self, ip: IpAddr) -> bool {
        let now = self.start.elapsed();
        let mut map = self.ip.lock().expect("ip limiter mutex");
        map.entry(ip)
            .or_insert_with(|| TokenBucket::new(IP_BURST, IP_REFILL_PER_SEC))
            .try_take(now)
    }

    /// Mint a room with a fresh unique `[A-Z0-9]` code, spawn its actor on the global
    /// executor, and return the code + its intake sender (spec §6.2). The collision
    /// check holds the rooms lock so two concurrent creates can't claim one code.
    pub fn create_room(self: &Arc<Self>, config: Config) -> (String, Sender<RoomMsg>) {
        let (tx, rx) = async_channel::unbounded::<RoomMsg>();
        let code = {
            let mut rooms = self.rooms.lock().expect("rooms mutex");
            let code = loop {
                let candidate = util::random_room_code();
                if !rooms.contains_key(&candidate) {
                    break candidate;
                }
            };
            rooms.insert(code.clone(), tx.clone());
            code
        };
        crate::EX
            .spawn(room_task(code.clone(), rx, Arc::clone(self), config))
            .detach();
        (code, tx)
    }

    /// Look up a live room's intake sender (spec §6.2). Unknown ⇒ `None` ⇒ the wire
    /// layer answers `Error{RoomNotFound}` (a typo never founds an empty room).
    pub fn lookup(&self, code: &str) -> Option<Sender<RoomMsg>> {
        self.rooms.lock().expect("rooms mutex").get(code).cloned()
    }

    /// Deregister a GC'd room (its actor calls this on exit, spec §6.2).
    pub fn remove_room(&self, code: &str) {
        self.rooms.lock().expect("rooms mutex").remove(code);
    }

    #[cfg(test)]
    fn room_count(&self) -> usize {
        self.rooms.lock().expect("rooms mutex").len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn create_mints_unique_codes_lookup_finds_them_remove_deregisters() {
        let reg = Registry::new();
        let (code_a, _tx_a) = reg.create_room(Config::default());
        let (code_b, _tx_b) = reg.create_room(Config::default());
        assert_eq!(code_a.len(), 6);
        assert_ne!(code_a, code_b, "each room gets a distinct code");
        assert_eq!(reg.room_count(), 2);

        assert!(
            reg.lookup(&code_a).is_some(),
            "a live room is found by code"
        );
        assert!(
            reg.lookup("ZZZZZZ").is_none(),
            "an unknown code is not found (⇒ RoomNotFound)"
        );

        reg.remove_room(&code_a);
        assert!(reg.lookup(&code_a).is_none(), "a GC'd room is deregistered");
        assert_eq!(reg.room_count(), 1);
    }

    #[test]
    fn per_ip_limiter_allows_a_burst_then_throttles_that_ip_only() {
        let reg = Registry::new();
        let ip: IpAddr = Ipv4Addr::new(203, 0, 113, 7).into();
        // The burst (IP_BURST) of rapid attempts passes; the next is throttled (a tight
        // loop leaves no time to refill at IP_REFILL_PER_SEC).
        let mut allowed = 0;
        for _ in 0..(IP_BURST as u32 + 5) {
            if reg.check_ip(ip) {
                allowed += 1;
            }
        }
        assert_eq!(
            allowed, IP_BURST as u32,
            "exactly the burst passed before throttling kicked in"
        );
        // A DIFFERENT IP has its own bucket — unaffected by the first IP's flood.
        let other: IpAddr = Ipv4Addr::new(198, 51, 100, 9).into();
        assert!(
            reg.check_ip(other),
            "a distinct IP is limited independently"
        );
    }
}
