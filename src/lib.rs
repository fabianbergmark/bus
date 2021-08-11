mod sync;

use crate::sync::Arc;
use crate::sync::Mutex;
use crate::sync::MutexGuard;
use crate::sync::Weak;
use crate::sync::lazy_static;
use petgraph::algo::tarjan_scc;
use petgraph::stable_graph::{NodeIndex, StableGraph};
use petgraph::visit::EdgeRef;
use petgraph::visit::IntoEdgeReferences;
use petgraph::visit::IntoNodeReferences;
use petgraph::Undirected;
use std::collections::HashMap;

type StateRef<P> = SharedRef<State<P>>;
type StateWeakRef<P> = WeakRef<State<P>>;
type SharedRef<R> = Arc<Mutex<R>>;
type WeakRef<R> = Weak<Mutex<R>>;
type Graph<P> = StableGraph<StateWeakRef<P>, (), Undirected>;

lazy_static! {
    static ref LOCK: Mutex<()> = Mutex::new(());
}

pub struct Bus<P: Default> {
    s: &'static str,
    state: StateRef<P>,
}

impl<P: Default> Bus<P> {
    pub fn new(s: &'static str) -> Self {
        let bus = Bus {
            s,
            state: Arc::new(Mutex::new(State::default())),
        };
        {
            let state = &mut bus.state.lock().unwrap();
            let ix;
            {
                let inner = &mut state.inner.lock().unwrap();
                ix = inner.map.add_node(Arc::downgrade(&bus.state));
            }
            state.ix = ix;
        }
        bus
    }
}

impl<P: Default> Drop for Bus<P> {
    fn drop(&mut self) {
        println!("drop");
        /*loop {
            //let _lock = LOCK.lock();
            let self_inner_ref;
            let self_ix;
            {
                let self_state = self.state.lock().unwrap();
                self_ix = self_state.ix;
                self_inner_ref = self_state.inner.clone();
            }
            let mut self_inner = self_inner_ref.lock().unwrap();

            if self_inner.contains(&self.state) {
                self_inner.map.remove_node(self_ix);
                Self::split_components(&mut self_inner.map);
                return;
            }
        }*/
    }
}

impl<P: Default> Bus<P> {
    pub fn connect(&mut self, bus: &mut Bus<P>) {
        println!("connect {}", self.s);
        //let lock_ = LOCK.lock();
        let self_inner_ref;
        let other_inner_ref;
        // Avoid holding two locks at the same time.
        {
            self_inner_ref = self.state.lock().unwrap().inner.clone();
        }
        {
            other_inner_ref = bus.state.lock().unwrap().inner.clone();
        }
        if !Arc::ptr_eq(&self_inner_ref, &other_inner_ref) {
            println!("before lock");
            let (mut self_inner, mut other_inner) = Self::lock(&self_inner_ref, &other_inner_ref, self.s);
            println!("after lock");

            // Verify that the bus state hasn't changed now that we have a lock.
            if !(self_inner.contains(&self.state) && other_inner.contains(&bus.state))
            {
                drop(self_inner);
                drop(other_inner);
                self.connect(bus);
                return;
            }

            Self::join_components(&mut self_inner.map, &mut other_inner.map, &self_inner_ref);

            let self_ix = self.state.lock().unwrap().ix;
            let other_ix = bus.state.lock().unwrap().ix;
            self_inner.map.add_edge(self_ix, other_ix, ());
        }
        println!("connect done {}", self.s);
    }

    fn lock<'a, 'b>(
        self_inner_ref: &'a SharedRef<Inner<P>>,
        other_inner_ref: &'b SharedRef<Inner<P>>,
        s: &'static str,
    ) -> (MutexGuard<'a, Inner<P>>, MutexGuard<'b, Inner<P>>) {
        // Lock the mutexes by order of the Arc ptr to prevent deadlocks.

        println!("lock {:?} {:?} {}", Arc::as_ptr(self_inner_ref), Arc::as_ptr(other_inner_ref), s);
        {
            Self::try_lock(self_inner_ref, s);
            Self::try_lock(other_inner_ref, s);
        }
        if Arc::as_ptr(self_inner_ref) > Arc::as_ptr(other_inner_ref) {
            let self_inner = self_inner_ref.lock().unwrap();
            println!("locked 1 {}", s);
            let other_inner = other_inner_ref.lock().unwrap();
            println!("locked 2 {}", s);
            (self_inner, other_inner)
        } else {
            let other_inner = other_inner_ref.lock().unwrap();
            println!("locked 3 {}", s);
    
            let self_inner = self_inner_ref.lock().unwrap();
            println!("locked 4 {}", s);
            (self_inner, other_inner)
        }
    }

    fn try_lock(l: &SharedRef<Inner<P>>, s: &'static str) {
        match l.try_lock() {
            Err(e) => {println!("{:?} {}", e, s)}
            _ => {println!("ok {}", s);}
        }
    }

    fn join_components(target: &mut Graph<P>, other: &mut Graph<P>, ptr: &SharedRef<Inner<P>>) {
        let mut ix_map = HashMap::new();
        for (ix, weak_state) in other.node_references() {
            if let Some(strong_state) = weak_state.upgrade() {
                let state = &mut strong_state.lock().unwrap();
                let new_ix = target.add_node(weak_state.clone());
                state.inner = ptr.clone();
                state.ix = new_ix;
                ix_map.insert(ix, new_ix);
            }
        }
        for edge in other.edge_references() {
            if let Some(source_ix) = ix_map.get(&edge.source()) {
                if let Some(target_ix) = ix_map.get(&edge.target()) {
                    target.add_edge(*source_ix, *target_ix, ());
                }
            }
        }
        other.clear();
    }

    pub fn disconnect(&mut self, bus: &mut Bus<P>) {
        println!("disconnect {}", self.s);
        //let lock_ = LOCK.lock();
        let self_inner_ref;
        let self_ix;
        let other_inner_ref;
        let other_ix;
        // Avoid holding two locks at the same time.
        {
            let self_state = self.state.lock().unwrap();
            self_ix = self_state.ix;
            self_inner_ref = self_state.inner.clone();
        }
        {
            let other_state = bus.state.lock().unwrap();
            other_ix = other_state.ix;
            other_inner_ref = other_state.inner.clone();
        }
        if Arc::ptr_eq(&self_inner_ref, &other_inner_ref) {
            println!("disconnect lock inner {} {:?}", self.s, Arc::as_ptr(&self_inner_ref));
            let mut self_inner = self_inner_ref.lock().unwrap();
            println!("disconnect locked inner {} {:?}", self.s, Arc::as_ptr(&self_inner_ref));

            // Verify that the bus state hasn't changed now that we have a lock.
            if !(self_inner.contains(&self.state) && self_inner.contains(&bus.state))
            {
                drop(self_inner);
                self.disconnect(bus);
                return;
            }
            let edge = self_inner.map.find_edge(self_ix, other_ix).unwrap();
            self_inner.map.remove_edge(edge);
            Self::split_components(&mut self_inner.map);
        }
        println!("disconnect done {}", self.s);
    }

    fn split_components(map: &mut Graph<P>) {
        let components = tarjan_scc(map as &Graph<P>);
        if components.len() > 1 {
            for component in components {
                let inner_ref = Arc::new(Mutex::new(Inner::default()));
                let mut inner = inner_ref.lock().unwrap();
                let mut ix_map = HashMap::new();
                for node in &component {
                    if let Some(strong_state) = map[*node].upgrade() {
                        let state = &mut strong_state.lock().unwrap();
                        let old_ix = state.ix;
                        assert_eq!(old_ix, *node);
                        let new_ix = inner.map.add_node(map[*node].clone());
                        ix_map.insert(old_ix, new_ix);
                        state.ix = new_ix;
                        state.inner = inner_ref.clone();
                    }
                }
                for node in component {
                    if let Some(node_ix) = ix_map.get(&node) {
                        for neighbor in map.neighbors(node) {
                            if let Some(neighbor_ix) = ix_map.get(&neighbor) {
                                if inner.map.find_edge(*node_ix, *neighbor_ix).is_none() {
                                    inner.map.add_edge(ix_map[&node], ix_map[&neighbor], ());
                                }
                            }
                        }
                    }
                }
            }
        }
        map.clear();
    }

    pub fn get_data(&self) -> Arc<P> {
        //let _lock = LOCK.lock();
        let inner_ref;
        {
            let state = self.state.lock().unwrap();
            inner_ref = state.inner.clone();
        }

        let inner = inner_ref.lock().unwrap();
        if !inner.contains(&self.state) {
            drop(inner);
            return self.get_data();
        }
        inner.p.clone()
    }
}

struct State<P> {
    ix: NodeIndex,
    inner: SharedRef<Inner<P>>,
}


impl<P> Drop for State<P> {
    fn drop(&mut self) {
        println!("drop state");
    }
}

impl<P: Default> Default for State<P> {
    fn default() -> Self {
        State {
            ix: NodeIndex::new(0),
            inner: Arc::new(Mutex::new(Inner::default())),
        }
    }
}

struct Inner<P> {
    map: Graph<P>,
    p: Arc<P>,
}

impl<P> Drop for Inner<P> {
    fn drop(&mut self) {
        println!("drop inner");
        self.map.clear();
    }
}

impl<P: Default> Default for Inner<P> {
    fn default() -> Self {
        Inner {
            map: Default::default(),
            p: Arc::new(P::default()),
        }
    }
}

impl <P> Inner<P> {
    fn contains(&self, state: &StateRef<P>) -> bool {
        for (_, weak_state) in self.map.node_references() {
            if let Some(strong_state) = weak_state.upgrade() {
                if Arc::ptr_eq(&strong_state, state) {
                    return true;
                }
            }
        }
        false
    }
}
