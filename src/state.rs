use crate::async_engine::{AsyncEngine, SpawnedFuture};
use crate::backend::{BackendEvent, OutputId, OutputIds, SeatId, SeatIds};
use crate::client::{Client, Clients};
use crate::event_loop::EventLoop;
use crate::format::Format;
use crate::globals::{AddGlobal, Globals};
use crate::ifs::wl_output::WlOutputGlobal;
use crate::ifs::wl_seat::WlSeatGlobal;
use crate::ifs::wl_surface::NoneSurfaceExt;
use crate::tree::{DisplayNode, NodeIds};
use crate::utils::asyncevent::AsyncEvent;
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::linkedlist::LinkedList;
use crate::utils::numcell::NumCell;
use crate::utils::queue::AsyncQueue;
use crate::Wheel;
use ahash::AHashMap;
use std::cell::RefCell;
use std::rc::Rc;

pub struct State {
    pub eng: Rc<AsyncEngine>,
    pub el: Rc<EventLoop>,
    pub wheel: Rc<Wheel>,
    pub clients: Clients,
    pub next_name: NumCell<u32>,
    pub globals: Globals,
    pub formats: AHashMap<u32, &'static Format>,
    pub output_ids: OutputIds,
    pub seat_ids: SeatIds,
    pub node_ids: NodeIds,
    pub root: Rc<DisplayNode>,
    pub backend_events: AsyncQueue<BackendEvent>,
    pub output_handlers: RefCell<AHashMap<OutputId, SpawnedFuture<()>>>,
    pub seats: RefCell<AHashMap<SeatId, SeatData>>,
    pub outputs: CopyHashMap<OutputId, Rc<WlOutputGlobal>>,
    pub seat_queue: LinkedList<Rc<WlSeatGlobal>>,
    pub slow_clients: AsyncQueue<Rc<Client>>,
    pub none_surface_ext: Rc<NoneSurfaceExt>,
}

pub struct SeatData {
    pub handler: SpawnedFuture<()>,
    pub tree_changed: Rc<AsyncEvent>,
}

impl State {
    pub fn add_global<T>(&self, global: &Rc<T>)
    where
        Globals: AddGlobal<T>,
    {
        self.globals.add_global(self, global)
    }

    pub fn tree_changed(&self) {
        let seats = self.seats.borrow();
        for seat in seats.values() {
            seat.tree_changed.trigger();
        }
    }
}
