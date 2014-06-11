#![feature(macro_rules)]
#![feature(phase)]

extern crate collections;
extern crate debug;
extern crate sync;

use std::comm::{channel, Sender, Receiver};
use std::task::spawn;
use std::any::{Any, AnyRefExt};
use collections::HashMap;

#[macro_escape]
mod match_any;

#[deriving(Send, Share)]
struct InternalMsg {
    contents: Box<Any:Share + Send>,
    sender: Option<Sender<InternalMsg>>
}

impl InternalMsg {
    fn new<T: Share + Send>(m: T, sender: Option<Sender<InternalMsg>>) -> InternalMsg {
        InternalMsg {
            contents: box m,
            sender: sender
        }
    }
}

enum SystemMsg {
    Undelivered(Box<Any: Share + Send>),
    Suicide
}

trait Actor: Clone + Send + Share {
    fn recieve(&mut self, msg: Box<Any: Share + Send>, sender: Option<Sender<InternalMsg>>);
    fn on_death() { }
}

#[deriving(Send)]
struct ActorWrapper {
    id: uint,
    sender: Sender<InternalMsg>,
    reciever: Receiver<InternalMsg>,
    handler: Box<Actor>
}

impl ActorWrapper {
    fn new<A: Actor + 'static>(a:A, id: uint)-> ActorWrapper {
        let (tx, rx) = channel();
        ActorWrapper {
            id: id,
            sender: tx,
            reciever: rx,
            handler: box a
        }
    }
}

trait ActorSystem {
    fn spawn<A: Actor>(&mut self, actor: A) -> uint;
    fn send_to<M: Send + Share>(&self, to_id: uint, msg: M, from: Option<Sender<InternalMsg>>);
}

#[deriving(Send, Share, Clone)]
struct LocalActorSystem {
    id_pool: uint,
    mapping: HashMap<uint, Sender<InternalMsg>>
}

impl LocalActorSystem {
    fn new() -> LocalActorSystem {
        LocalActorSystem {
            id_pool: 0,
            mapping: HashMap::new()
        }
    }
}

impl ActorSystem for LocalActorSystem {
    fn send_to<M: Send + Share>(&self, to_id: uint, msg: M, from: Option<Sender<InternalMsg>>) {
        match self.mapping.find(&to_id) {
            Some(ref sender) => { sender.send(InternalMsg::new(msg, from)); }
            None => {
                match from {
                    Some(sender) => sender.send(InternalMsg::new(Undelivered(box msg), None)),
                    None => {}
                }
            }
        };
    }

    fn spawn<A: Actor>(&mut self, actor: A) -> uint {
        let id = self.id_pool;
        self.id_pool += 1;

        let (tx, rx) = channel();
        spawn(proc() {
            let mut wrap = ActorWrapper::new(actor, id);
            tx.send(wrap.sender.clone());
            loop {
                let InternalMsg{contents, sender} = wrap.reciever.recv();
                match_any! { contents match
                    if SystemMsg {
                        &Suicide => { break; },
                        _ => { }
                    } else {
                        ()
                    }
                }
                wrap.handler.recieve(contents, sender);
            }
        });

        let sender = rx.recv();
        self.mapping.insert(id, sender.clone());
        id
    }
}

fn main(){
    #[deriving(Clone)]
    struct AddActor {
        state: uint
    }
    impl Actor for AddActor {
        fn recieve(&mut self, msg: Box<Any: Share + Send>, sender: Option<Sender<InternalMsg>>) {
            match_any! { msg match
                if uint {
                    &x => {self.state += x}
                } else {
                    ()
                }
            }
            match sender {
                Some(ref snd) => snd.send(InternalMsg::new(self.state, None)),
                None => {}
            }
        }
    }

    let mut las = LocalActorSystem::new();

    let id = las.spawn(AddActor{state: 0});

    let (tr, rx) = channel();
    las.send_to(id, 1u, Some(tr.clone()));
    las.send_to(id, Suicide, None);
    let resp = rx.recv();
    match_any! { resp.contents match
        if uint {
            &x => println!("{}",x)
        } else {()}
    }
}
