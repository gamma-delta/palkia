# Palkia

An Entity-Component-Message architecture crate.

## What's an Entity-Component-Message architecture?

Here's an excert from [the blog post I wrote on it](https://www.petra-k.at/blog/2022-07-22_fff4/):

> Like in ECS, under ECM you have entities, which are lists of components. But, instead of linking behavior of different components with systems, 
> you do it by passing messages.
>
> When you implement Component for a struct, you implement a method that registers that struct with different message types. 
> Then, from a message handler, you can fire messages to other entities.
>
> When an entity gets a message, it runs through its components in order, and if that component type registers a handler for that message type,
> it runs the handler and passes the updated value to the next component, and so on … and then finally returns the modified message to the caller.
> 
> And there’s a method on World to pass a message to all entities, as your entrypoint.

Check out the [tests](https://github.com/gamma-delta/palkia/tree/main/tests) or [examples](https://github.com/gamma-delta/palkia/blob/main/examples/game.rs) for more, I guess.

## Why is it called Palkia?

I've been naming the helper crates for Foxfire after Pokemon, just because there's a lot of them and I don't want to spend tons of time coming up with names.
I picked Palkia specifically because the crate provides a method of organizing data, and Palkia controls space.

---
