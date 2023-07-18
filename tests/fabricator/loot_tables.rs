use dialga::{factory::ComponentFactory, EntityFabricator};
use palkia::prelude::*;

use eyre::bail;
use rand::{rngs::StdRng, SeedableRng};
use serde::Deserialize;
use wicker::WeightedPicker;

use std::{collections::HashMap, sync::Mutex};

macro_rules! impl_component {
    (@ $ty:ty) => {
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
    ($($ty:ty),* $(,)?) => {
        $(
            impl_component!{@ $ty}
        )*
    };
}

struct Inventoried {
    items: Vec<Entity>,
}

#[derive(Deserialize)]
struct HasName(String);

impl_component!(Inventoried, HasName);

/// Fake component for loading an inventory either literally or from a loot table
struct InventoryFactory;

impl ComponentFactory<Context> for InventoryFactory {
    fn assemble<'a, 'w>(
        &self,
        mut builder: EntityBuilder<'a, 'w>,
        node: &kdl::KdlNode,
        ctx: &Context,
    ) -> eyre::Result<EntityBuilder<'a, 'w>> {
        #[derive(Deserialize)]
        #[serde(tag = "type")]
        enum Raw {
            Literal {
                items: Vec<String>,
            },
            LootTable {
                #[serde(rename = "table-name")]
                table_name: String,
            },
        }

        let raw: Raw = knurdy::deserialize_node(node)?;

        let items = match raw {
            Raw::Literal { items } => items
                .iter()
                .map(|bp_name| {
                    // god
                    let builder2 = builder.spawn_again();
                    ctx.fabber.instantiate(bp_name, builder2, ctx)
                })
                .collect::<Result<Vec<_>, _>>()?,
            Raw::LootTable { table_name } => {
                let table_picker =
                    match ctx.loot_tables.get(table_name.as_str()) {
                        Some(it) => it,
                        None => bail!("no loot table named {:?}", &table_name),
                    };

                let builder2 = builder.spawn_again();
                let bp_name = {
                    let mut rng = ctx.rng.lock().unwrap();
                    table_picker.get(&mut *rng)
                };
                let e = ctx.fabber.instantiate(&bp_name, builder2, ctx)?;
                vec![e]
            }
        };

        if let Some(inv_here) = builder.get_component_mut::<Inventoried>() {
            inv_here.items.extend(items);
        } else {
            builder.insert(Inventoried { items });
        }

        Ok(builder)
    }
}

struct Context {
    loot_tables: HashMap<String, WeightedPicker<String>>,
    fabber: EntityFabricator<Context>,
    rng: Mutex<StdRng>,
}

const BLUEPRINT_SRC: &str = r#"
stone-sword {
    has-name "Stone Sword"
}
iron-sword {
    has-name "Iron Sword"
}
diamond-sword {
    has-name "Diamond Sword!"
}
stone-hammer {
    has-name "Stone HAMMAR"
}
copper { has-name "Copper Piece"; }
gold { has-name "Gold Piece"; }
jewel { has-name "Fancy Expensive Jewel"; }

player {
    inventory type="Literal" { 
        // This is slightly awkward to write unfortunately
        items "copper" "copper"
    }
    inventory type="LootTable" table-name="common-weapons"
    inventory type="LootTable" table-name="common-loot"
}

treasure-chest {
    has-name "Treasure chest! wowza"
    inventory type="Literal" { items "copper" "gold" "gold"; }
    inventory type="LootTable" table-name="rare-loot"
    inventory type="LootTable" table-name="common-loot"
}
"#;

mod loot {
    use std::collections::HashMap;

    use kdl::KdlDocument;
    use serde::Deserialize;
    use wicker::WeightedPicker;

    const LOOT_TABLES: &str = r#"
common-weapons {
    stone-sword  weight=10
    iron-sword   weight=3
    stone-hammer weight=5
}

rare-weapons {
    iron-sword    weight=8
    diamond-sword weight=3
}

common-loot {
    // In real life each coin wouldn't be its own object, there would be itemstacks...
    // and there would be some way to weight the amount per loot pool, not just a constant...
    copper weight=10
    gold   weight=1
    jewel  weight=1
}

rare-loot {
    gold   weight=5
    jewel  weight=1
}
"#;

    #[derive(Deserialize)]
    struct LootTableEntry {
        weight: f64,
    }

    pub fn load() -> HashMap<String, WeightedPicker<String>> {
        let doc: KdlDocument = LOOT_TABLES.parse().unwrap();

        let mut tables = HashMap::new();

        for kid in doc.nodes() {
            let table: HashMap<String, LootTableEntry> =
                knurdy::deserialize_node(kid).unwrap();
            let picker = WeightedPicker::new(
                table
                    .into_iter()
                    .map(|(name, entry)| (name, entry.weight))
                    .collect(),
            );
            tables.insert(kid.name().to_string(), picker);
        }

        tables
    }
}

#[test]
fn main() {
    const RAND_SEED: u64 = 0x76043972beadcafe;

    let mut world = World::new();
    world.register_component::<HasName>();
    world.register_component::<Inventoried>();

    let mut fabber = EntityFabricator::new();
    fabber.register_serde::<HasName>("has-name");
    fabber.register("inventory", InventoryFactory);
    fabber
        .load_str(BLUEPRINT_SRC, "example.kdl")
        .unwrap_or_else(|e| panic!("{:?}", miette::Report::new(e)));

    let mut ctx = Context {
        loot_tables: loot::load(),
        fabber,
        rng: Mutex::new(StdRng::seed_from_u64(RAND_SEED)),
    };

    let player = ctx
        .fabber
        .instantiate("player", world.spawn(), &ctx)
        .unwrap();
    let treasure_chest = ctx
        .fabber
        .instantiate("treasure-chest", world.spawn(), &ctx)
        .unwrap();

    // now i don't know what the rng is gonna give, but I don't care! PBT baby
    *ctx.rng.get_mut().unwrap() = StdRng::seed_from_u64(RAND_SEED);
    let player2 = ctx
        .fabber
        .instantiate("player", world.spawn(), &ctx)
        .unwrap();
    let treasure_chest2 = ctx
        .fabber
        .instantiate("treasure-chest", world.spawn(), &ctx)
        .unwrap();
    {
        let player_inv = world.query::<&Inventoried>(player).unwrap();
        let player_inv2 = world.query::<&Inventoried>(player2).unwrap();
        assert_eq!(player_inv.items.len(), 4);
        for (e1, e2) in player_inv.items.iter().zip(player_inv2.items.iter()) {
            let name1 = world.query::<&HasName>(*e1).unwrap();
            let name2 = world.query::<&HasName>(*e2).unwrap();
            assert_eq!(&name1.0, &name2.0);
        }
    }
    {
        let treasure_inv = world.query::<&Inventoried>(treasure_chest).unwrap();
        let treasure_inv2 =
            world.query::<&Inventoried>(treasure_chest2).unwrap();
        assert_eq!(treasure_inv.items.len(), 5);
        for (e1, e2) in
            treasure_inv.items.iter().zip(treasure_inv2.items.iter())
        {
            let name1 = world.query::<&HasName>(*e1).unwrap();
            let name2 = world.query::<&HasName>(*e2).unwrap();
            assert_eq!(&name1.0, &name2.0);
        }
    }
    // Dummy check
    {
        let treasure_name = world.query::<&HasName>(treasure_chest).unwrap();
        let treasure_name2 = world.query::<&HasName>(treasure_chest2).unwrap();
        assert_eq!(&treasure_name.0, &treasure_name2.0);
    }
}
