//! Atlas device-buffer ownership-transfer contract.
//!
//! This test uses the real Hephaestus WGPU device/stream buffer path. It skips
//! only when no WGPU adapter can be acquired; it does not substitute a mock
//! buffer or fake stream.

use core::sync::atomic::AtomicUsize;
use std::thread;

use hephaestus_core::{CommandStream, KernelDevice};
use hephaestus_wgpu::{ComputeDevice, DeviceBuffer, WgpuBuffer, WgpuDevice};
use melinoe::sync::{sync_region_scope, SyncRegionToken};
use melinoe::{AcqRel, BrandedAtomic, MelinoeCell};

struct DeviceRegion<'brand> {
    source: MelinoeCell<'brand, WgpuBuffer<u32>>,
    destination: MelinoeCell<'brand, WgpuBuffer<u32>>,
    fence: BrandedAtomic<'brand, AtomicUsize>,
}

fn device_or_skip() -> Option<WgpuDevice> {
    match WgpuDevice::try_default("melinoe-atlas-device-contract") {
        Ok(device) => Some(device),
        Err(error) => {
            eprintln!("skipping Atlas device contract: {error}");
            None
        }
    }
}

fn submit_copy_with_owned_region<'brand>(
    device: &WgpuDevice,
    mut token: SyncRegionToken<'brand>,
    region: &DeviceRegion<'brand>,
) -> hephaestus_wgpu::Result<SyncRegionToken<'brand>> {
    {
        let source = region.source.borrow(&token);
        let destination = region.destination.borrow(&token);

        let mut stream = device.stream()?;
        stream.copy(&*source, &*destination)?;
        stream.submit()?;
        device.synchronize()?;
    }

    region.fence.store_exclusive(1, &mut token);
    Ok(token)
}

#[test]
fn sync_region_token_owns_real_device_stream_copy() {
    let Some(device) = device_or_skip() else {
        return;
    };

    sync_region_scope(|token| {
        let host = [3_u32, 5, 8, 13, 21, 34, 55, 89];
        let region = DeviceRegion {
            source: MelinoeCell::new(device.upload(&host).unwrap()),
            destination: MelinoeCell::new(device.alloc_zeroed::<u32>(host.len()).unwrap()),
            fence: BrandedAtomic::new(0),
        };

        let token = submit_copy_with_owned_region(&device, token, &region).unwrap();
        let read = token.share();

        thread::scope(|scope| {
            let handles = (0..2)
                .map(|_| {
                    let device = device.clone();
                    let region = &region;
                    scope.spawn(move || {
                        let destination = region.destination.borrow(read);
                        let mut out = [0_u32; 8];
                        device.download(&*destination, &mut out).unwrap();
                        region.fence.fetch_add_with(1, read, AcqRel);
                        out
                    })
                })
                .collect::<Vec<_>>();

            for handle in handles {
                assert_eq!(handle.join().unwrap(), host);
            }
        });

        let mut token = token;
        assert_eq!(region.fence.load_exclusive(&mut token), 3);
        assert_eq!(region.destination.borrow(&token).len(), host.len());
    });
}
