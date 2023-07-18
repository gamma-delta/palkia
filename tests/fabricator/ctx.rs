use dialga::{factory::ComponentFactory, EntityFabricator};
use kdl::KdlNode;
use palkia::prelude::*;

use serde::Deserialize;

use std::sync::atomic::{AtomicU32, Ordering};

macro_rules! impl_component {
    ($ty:ty) => {
        impl Component for $ty {
            fn register_handlers(
                builder: HandlerBuilder<Self>,
            ) -> HandlerBuilder<Self>
            where
                Self: Sized,
            {
                builder
            }
        }
    };
}

struct Context {
    counter: AtomicU32,
}

//

struct SingleInc {
    foo: u32,
}
impl_component!(SingleInc);

struct DoubleInc {
    bar: String,
}
impl_component!(DoubleInc);

struct SingleIncAssembler;
impl ComponentFactory<Context> for SingleIncAssembler {
    fn assemble<'a, 'w>(
        &self,
        mut builder: EntityBuilder<'a, 'w>,
        node: &KdlNode,
        ctx: &Context,
    ) -> eyre::Result<EntityBuilder<'a, 'w>> {
        // Cool pattern: make a sentinel struct you deser with Serde and then frobnicate
        #[derive(Deserialize)]
        struct Raw {
            foo: u32,
            increment: u32,
        }

        let raw: Raw = knurdy::deserialize_node(node)?;

        ctx.counter.fetch_add(raw.increment, Ordering::SeqCst);

        let inc = SingleInc { foo: raw.foo };
        builder.insert(inc);
        Ok(builder)
    }
}

struct DoubleIncAssembler;
impl ComponentFactory<Context> for DoubleIncAssembler {
    fn assemble<'a, 'w>(
        &self,
        mut builder: EntityBuilder<'a, 'w>,
        node: &KdlNode,
        ctx: &Context,
    ) -> eyre::Result<EntityBuilder<'a, 'w>> {
        #[derive(Deserialize)]
        struct Raw {
            bar: String,
            increment: u32,
        }

        let raw: Raw = knurdy::deserialize_node(node)?;

        ctx.counter.fetch_add(raw.increment * 2, Ordering::SeqCst);

        let inc = DoubleInc { bar: raw.bar };
        builder.insert(inc);
        Ok(builder)
    }
}

fn setup_world() -> World {
    let mut world = World::new();

    world.register_component::<SingleInc>();
    world.register_component::<DoubleInc>();

    world
}

fn setup_fab() -> EntityFabricator<Context> {
    let mut fab = EntityFabricator::new();

    fab.register("single", SingleIncAssembler);
    fab.register("double", DoubleIncAssembler);

    fab
}

fn setup_both() -> (World, EntityFabricator<Context>) {
    (setup_world(), setup_fab())
}

#[test]
fn test() {
    let bp_src = r#"
alpha {
    single increment=1 foo=42
}
beta {
    double increment=3 bar="Hello, world!"
}
gamma {
    single increment=5 foo=69
    double increment=7 bar="beep boop"
}
    "#;

    let context = Context {
        counter: AtomicU32::new(0),
    };

    let (mut world, mut fab) = setup_both();
    fab.load_str(bp_src, "example.kdl")
        .unwrap_or_else(|e| panic!("{:?}", miette::Report::new(e)));

    assert_eq!(context.counter.load(Ordering::SeqCst), 0);

    let alpha = fab.instantiate("alpha", world.spawn(), &context).unwrap();
    assert_eq!(context.counter.load(Ordering::SeqCst), 1);

    let beta = fab.instantiate("beta", world.spawn(), &context).unwrap();
    assert_eq!(context.counter.load(Ordering::SeqCst), 1 + 2 * 3);

    let gamma = fab.instantiate("gamma", world.spawn(), &context).unwrap();
    assert_eq!(
        context.counter.load(Ordering::SeqCst),
        1 + 2 * 3 + (5 + 2 * 7)
    );

    // just to be sure
    let alpha_foo = world.query::<&SingleInc>(alpha).unwrap().foo;
    assert_eq!(alpha_foo, 42);
    let beta_bar = &world.query::<&DoubleInc>(beta).unwrap().bar;
    assert_eq!(beta_bar.as_str(), "Hello, world!");
    let gamma_foo = world.query::<&SingleInc>(gamma).unwrap().foo;
    let gamma_bar = &world.query::<&DoubleInc>(gamma).unwrap().bar;
    assert_eq!(gamma_foo, 69);
    assert_eq!(gamma_bar.as_str(), "beep boop");
}
