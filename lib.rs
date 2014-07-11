#![feature(default_type_params)]

extern crate debug;

use std::kinds::Send;
use std::fmt::Show;
use std::comm::{
    SyncSender,
    Receiver,
    Empty,
    Disconnected
};

use std::task::spawn;
use std::io::timer;

#[deriving(Send)]
enum SystemMessage<I> {
    AddSubscriber(Sender<I>),
    KillYourself,
}

trait NodeState<I: Send, O: Send + Copy> {
    fn act(&mut self, msg: I) -> O;
}

struct Node<I, O, S> {
    inbox_read: Receiver<I>,
    // is None once there is a reference to it.
    inbox_enter: Sender<I>,

    state: S,
    listeners: Vec<Sender<O>>,

    system_read: Receiver<SystemMessage<O>>,
    // is None once there is a reference to it.
    system_enter: SyncSender<SystemMessage<O>>
}

struct NodeRef<I, O> {
    inbox_enter_ref: Sender<I>,
    system_enter_ref: SyncSender<SystemMessage<O>>
}

struct FnNodeState<I, O> {
    f: fn(I) -> O
}

fn actor_from_fn<A: Send + Copy, B: Send + Copy>(f: fn(A)-> B) -> Node<A, B, FnNodeState<A, B>>{
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

fn link_from_to<A, B, C: Send>(a: &NodeRef<A, C>, b: &NodeRef<C, B>) {
    a.system_enter_ref.send(AddSubscriber(b.inbox_enter_ref.clone()));
}

fn link_to_channel<I, O: Send>(a: &NodeRef<I, O>, ch: Sender<O>) {
    a.system_enter_ref.send(AddSubscriber(ch));
}

impl <I: Send + Copy, O: Send + Copy, S: 'static + Send + NodeState<I, O>> Node<I, O, S> {
    fn get_ref(&mut self) -> NodeRef<I,O> {
        NodeRef {
            inbox_enter_ref: self.inbox_enter.clone(),
            system_enter_ref: self.system_enter.clone()
        }
    }

    fn spawn(mut self) -> NodeRef<I, O> {
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
    fn send(&self, message: I) {
        self.inbox_enter_ref.send(message);
    }
    fn send_kill(&self) {
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

//
// Example
//


fn print<A: Show>(a: A) {
    println!("{}", a);
}

fn double(x: uint) -> uint { x * 2 }
fn square(x: uint) -> uint { x * x }

fn main() {
    let (s, r) = channel();
    {
        let double_actor = actor_from_fn(double).spawn();
        let square_actor = actor_from_fn(square).spawn();
        let print_actor = actor_from_fn(print).spawn();

        link_from_to(&double_actor, &square_actor);
        link_from_to(&square_actor, &print_actor);
        link_to_channel(&square_actor, s.clone());

        double_actor.send(1);
        double_actor.send(2);
        double_actor.send(3);
        double_actor.send(4);
        double_actor.send(5);
        double_actor.send(6);

        spawn(proc() {
            timer::sleep(500);
            double_actor.send_kill();
            square_actor.send_kill();
            print_actor.send_kill();
        });
    }

    let xs: Vec<uint> = r.iter().take(5).collect();
    println!("{}", xs);
}
