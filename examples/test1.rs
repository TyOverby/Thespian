extern crate thespian;

use std::fmt::Show;
use std::io::timer;
use std::task::spawn;

use thespian::actor_from_fn;
use thespian::link_from_to;
use thespian::link_to_channel;

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
