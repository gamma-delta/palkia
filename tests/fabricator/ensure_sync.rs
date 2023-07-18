use std::marker::PhantomData;

use dialga::EntityFabricator;

struct MustSendSync<T: Send + Sync>(PhantomData<T>);

#[test]
fn must_compile() {
    let _ = MustSendSync::<EntityFabricator<()>>(PhantomData);
}
