#![crate_id="thespian"]

#![feature(default_type_params)]

use std::kinds::Send;
use std::comm::{
    SyncSender,
    Receiver,
    Empty,
    Disconnected
};

use std::task::spawn;

#[deriving(Send)]
pub enum SystemMessage<I> {
    AddSubscriber(Sender<I>),
    KillYourself,
}

pub trait NodeState<I: Send, O: Send + Copy> {
    fn act(&mut self, msg: I) -> O;
}

pub struct Node<I, O, S> {
    inbox_read: Receiver<I>,
    // is None once there is a reference to it.
    inbox_enter: Sender<I>,

    state: S,
    listeners: Vec<Sender<O>>,

    system_read: Receiver<SystemMessage<O>>,
    // is None once there is a reference to it.
    system_enter: SyncSender<SystemMessage<O>>
}

pub struct NodeRef<I, O> {
    inbox_enter_ref: Sender<I>,
    system_enter_ref: SyncSender<SystemMessage<O>>
}

pub struct FnNodeState<I, O> {
    f: fn(I) -> O
}

pub fn actor_from_fn<A: Send + Copy, B: Send + Copy>(f: fn(A)-> B) -> Node<A, B, FnNodeState<A, B>>{
    let (inbox_enter, inbox_read) = channel();
    let (listeners_enter, listeners_read) = sync_channel(1);
    Node {
        inbox_read: inbox_read,
        inbox_enter: inbox_enter,
        state: FnNodeState {f: f},
        listeners: Vec::new(),
        system_read: listeners_read,
        system_enter: listeners_enter
    }
}

pub fn link_from_to<A, B, C: Send>(a: &NodeRef<A, C>, b: &NodeRef<C, B>) {
    a.system_enter_ref.send(AddSubscriber(b.inbox_enter_ref.clone()));
}

pub fn link_to_channel<I, O: Send>(a: &NodeRef<I, O>, ch: Sender<O>) {
    a.system_enter_ref.send(AddSubscriber(ch));
}

impl <I: Send + Copy, O: Send + Copy, S: 'static + Send + NodeState<I, O>> Node<I, O, S> {
    pub fn get_ref(&mut self) -> NodeRef<I,O> {
        NodeRef {
            inbox_enter_ref: self.inbox_enter.clone(),
            system_enter_ref: self.system_enter.clone()
        }
    }

    pub fn spawn(mut self) -> NodeRef<I, O> {
        let ret = self.get_ref();
        spawn(proc() {
            let mut node = self;
            loop {
                match node.system_read.try_recv() {
                    Ok(AddSubscriber(l)) => {
                        node.listeners.push(l);
                    }
                    Ok(KillYourself) => {
                        break;
                    }
                    Err(Empty) => { }
                    Err(Disconnected) => { }
                };

                let mesg = match node.inbox_read.try_recv() {
                    Ok(m) => {
                        m
                    }
                    Err(Empty) => {
                        continue;
                    }
                    Err(Disconnected) => {
                        break;
                    }
                };

                let ans = node.state.act(mesg);

                // Try to send messages to all of our listeners, but
                // Kill everything that can no longer be sent to.
                node.listeners.retain(|listener| {
                    match listener.send_opt(ans) {
                        Ok(_) => {
                            true
                        }
                        Err(_) => {
                            false
                        }
                    }
                });
            }
        });
        ret
    }
}

impl <I: Send, O: Send> NodeRef<I, O> {
    pub fn send(&self, message: I) {
        self.inbox_enter_ref.send(message);
    }
    pub fn send_kill(&self) {
        self.system_enter_ref.send(KillYourself);
    }
}

impl <I: Send, O: Send> Clone for NodeRef<I, O> {
    fn clone(&self) -> NodeRef<I, O> {
        NodeRef {
            inbox_enter_ref: self.inbox_enter_ref.clone(),
            system_enter_ref: self.system_enter_ref.clone()
        }
    }
}

impl <I: Send, O: Send> NodeState<I, O> for FnNodeState<I, O> {
    fn act(&mut self, msg: I) -> O {
        (self.f)(msg)
    }
}
