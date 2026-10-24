//! The real-time player: its blocks against the golden `transport`
//! area, media read ahead of the render thread, the defects it fixes, its
//! recordings, its control calls under concurrency (RT-06), and the prototype's engine tests on
//! miniaudio's null device (rows 4–7).
//!
//! The binary installs `AllocDisabler`: the render steps that run inside `assert_no_alloc` abort
//! on an allocation or a deallocation (RT-02, RT-09).

use assert_no_alloc::AllocDisabler;

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

mod golden;
mod media;
mod null_device;
mod record;
mod support;
mod transport;
