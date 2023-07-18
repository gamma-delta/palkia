use std::collections::HashMap;

use dialga::EntityFabricator;
use palkia::prelude::*;
use serde::Deserialize;

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

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct TrackedPosition;

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct Positioned {
  x: i32,
  y: i32,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct Named(String);

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct PhysicBody {
  mass: u32,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct HasHP {
  start_hp: u32,
  #[serde(default)]
  resistances: HashMap<String, i32>,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct FactionAffiliations {
  member_of: String,
  liked_by: Vec<String>,
  disliked_by: Vec<String>,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct Legendary;

impl_component!(
  TrackedPosition,
  Positioned,
  Named,
  PhysicBody,
  HasHP,
  FactionAffiliations,
  Legendary
);

fn setup_world() -> World {
  let mut world = World::new();
  world.register_component::<TrackedPosition>();
  world.register_component::<Positioned>();
  world.register_component::<Named>();
  world.register_component::<PhysicBody>();
  world.register_component::<HasHP>();
  world.register_component::<FactionAffiliations>();
  world.register_component::<Legendary>();
  world
}

fn setup_fab() -> EntityFabricator<()> {
  let mut fab = EntityFabricator::new();
  fab.register_serde::<TrackedPosition>("tracked-position");
  fab.register_serde::<Named>("name");
  fab.register_serde::<PhysicBody>("physic-body");
  fab.register_serde::<HasHP>("has-hp");
  fab.register_serde::<FactionAffiliations>("factions");
  fab.register_serde::<Legendary>("legendary");
  fab
}

fn setup_both() -> (World, EntityFabricator<()>) {
  (setup_world(), setup_fab())
}

#[test]
fn test() {
  let (mut world, mut fab) = setup_both();

  let bp_src = r#"
    // Two example splicees -- in real life these will be more complicated
    mob {
        tracked-position
    }

    legend {
        legendary
    }

    grass {
        physic-body mass=10
        has-hp start-hp=10
    }

    cat {
        (splice)mob
        physic-body mass=50
        has-hp { 
            start-hp 10
            resistances falling=100 ice=-20
        }
        factions {
            member-of "cats"
            liked-by "humans" "elves"
            disliked-by "dogs" "dwarves"
        }
    }

    housecat {
        (splice)cat
        name "Macy"
    }

    puma {
        (splice)cat
        physic-body mass=150
        (splice)legend
    }
    "#;
  fab
    .load_str(bp_src, "example.kdl")
    .unwrap_or_else(|e| panic!("{:?}", miette::Report::new(e)));

  let grass = fab.instantiate("grass", world.spawn(), &()).unwrap();
  {
    let (pb, hp) = world.query::<(&PhysicBody, &HasHP)>(grass).unwrap();
    assert_eq!(*pb, PhysicBody { mass: 10 },);
    assert_eq!(
      *hp,
      HasHP {
        start_hp: 10,
        resistances: HashMap::new(),
      },
    );
  }

  let housecat = fab.instantiate("housecat", world.spawn(), &()).unwrap();
  {
    let (name, pb, hp, fa, _tp) = world
      .query::<(
        &Named,
        &PhysicBody,
        &HasHP,
        &FactionAffiliations,
        &TrackedPosition,
      )>(housecat)
      .unwrap();

    assert_eq!(*name, Named("Macy".to_owned()));
    assert_eq!(*pb, PhysicBody { mass: 50 });
    assert_eq!(
      *hp,
      HasHP {
        start_hp: 10,
        resistances: [("falling".to_string(), 100), ("ice".to_string(), -20)]
          .into_iter()
          .collect(),
      }
    );

    assert_eq!(
      *fa,
      FactionAffiliations {
        member_of: "cats".to_string(),
        liked_by: vec!["humans".to_owned(), "elves".to_owned()],
        disliked_by: vec!["dogs".to_owned(), "dwarves".to_owned()],
      }
    );
  }

  let puma = fab.instantiate("puma", world.spawn(), &()).unwrap();
  {
    let (pb, hp, fa, _tp, _leg) = world
      .query::<(
        &PhysicBody,
        &HasHP,
        &FactionAffiliations,
        &TrackedPosition,
        &Legendary,
      )>(puma)
      .unwrap();

    assert_eq!(*pb, PhysicBody { mass: 150 });
    assert_eq!(
      *hp,
      HasHP {
        start_hp: 10,
        resistances: [("falling".to_string(), 100), ("ice".to_string(), -20)]
          .into_iter()
          .collect(),
      }
    );
    assert_eq!(
      *fa,
      FactionAffiliations {
        member_of: "cats".to_string(),
        liked_by: vec!["humans".to_owned(), "elves".to_owned()],
        disliked_by: vec!["dogs".to_owned(), "dwarves".to_owned()],
      }
    );
  }
}

#[test]
#[should_panic(
  expected = r#"BlueprintLookupError(BlueprintNotFound("unknown"))"#
)]
fn error_unknown_blueprint() {
  let (mut world, mut fab) = setup_both();

  let bp_src = r#"
    oh-no {
        tracked-position
        erroring-comp
    }
    "#;
  fab
    .load_str(bp_src, "example.kdl")
    .unwrap_or_else(|e| panic!("{:?}", miette::Report::new(e)));

  fab.instantiate("unknown", world.spawn(), &()).unwrap();
}

#[test]
#[should_panic(expected = r#"NoAssembler("erroring-comp")"#)]
fn error_unknown_component() {
  let (mut world, mut fab) = setup_both();

  let bp_src = r#"
    oh-no {
        tracked-position
        erroring-comp
    }
    "#;
  fab
    .load_str(bp_src, "example.kdl")
    .unwrap_or_else(|e| panic!("{:?}", miette::Report::new(e)));

  fab.instantiate("oh-no", world.spawn(), &()).unwrap();
}

#[test]
#[should_panic(expected = r#"BlueprintLookupError(InheritanceLoop(["#)]
fn error_loop() {
  let bp_src = r#"
    alpha {
        physic-body mass=10
        (splice)beta
    }
    beta {
        has-hp start-hp=10
        (splice)gamma
    }
    gamma {
        legendary
        (splice)delta
    }
    delta {
        tracks-position
        (splice)alpha
    }
    entrypoint {
        (splice)alpha
    }
    "#;

  let (mut world, mut fab) = setup_both();
  fab
    .load_str(bp_src, "example.kdl")
    .unwrap_or_else(|e| panic!("{:?}", miette::Report::new(e)));

  fab.instantiate("entrypoint", world.spawn(), &()).unwrap();
}

#[test]
#[should_panic(
  expected = r#"BlueprintLookupError(InheriteeNotFound("foobar", "oh-no"))"#
)]
fn error_splice_fail() {
  let bp_src = r#"
    foobar {
        (splice)oh-no
    }
    "#;

  let (mut world, mut fab) = setup_both();
  fab
    .load_str(bp_src, "example.kdl")
    .unwrap_or_else(|e| panic!("{:?}", miette::Report::new(e)));

  fab.instantiate("foobar", world.spawn(), &()).unwrap();
}
