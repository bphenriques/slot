# H700 panel support

Slot defaults to 120 Hz for these exact Base OS model/panel pairs, with
720×480 resolution, totals 820×536, and a 26 MHz DT pixel clock:

| Base OS target | Vendor panel driver | Evidence |
| --- | --- | --- |
| `rgsp` | `rg34xxsp_v1` | Full 60/120 Hz resource and game-speed matrix on the connected device |
| `rg34xxsp` | `rg34xxsp_v1` | Live 120.14 Hz LCD interrupt measurement; ROCKNIX has an explicit 120 Hz mode |
| `rg34xx` | `fog_fj035fhd05_v1` | Vendor DT identity/timings and ROCKNIX's explicit 120 Hz mode; no non-SP device connected for a live test |

The common vendor driver name alone is insufficient: other models use it for
640×480 panels. Both the exact model/panel pair and the timing fields must match.
Other panel revisions remain unchanged. Register writes use the same tested
divider-before-PLL enable sequence and PLL-before-divider restore sequence,
with readback verification and a 60 Hz rollback attempt on failure. Nothing
changes the physical mode during emulation. Slot applies it at startup and
reapplies it before restoring the backlight on wake.

The connected RG34XX SP was running NextUI. Its temporary register change leaves
NextUI installed, and reboot restores the original hardware mode. Slot's default
and wake behavior apply when running Slot; no NextUI startup scripts were changed.

## ROCKNIX evidence

Source revision: `c17558d2abadebec634100264523aaddc98b3b96`.
ROCKNIX uses the generic MIPI DPI/SPI driver's firmware timing tables, exposed
as DRM display modes. Its [driver and binary format](https://github.com/ROCKNIX/distribution/blob/c17558d2abadebec634100264523aaddc98b3b96/projects/ROCKNIX/devices/H700/patches/linux/0110-v2_20250226_kikuchan98_drm_panel_add_generic_mipi_panel_driver.patch)
describe a 64-byte header/config followed by 32-byte big-endian timing records.
The decoded [complete panel table](results/rocknix-panel-modes.json) records:

| ROCKNIX panel firmware | Advertised modes |
| --- | --- |
| `rg34xx-panel` | 60 / 120 Hz |
| `rg34xx-sp-panel` | 60 / 120 Hz |
| `rg34xx-sp-v2-panel` | 60 Hz |
| `rg35xx-plus-panel` | 60 / 120 Hz |
| `rg35xx-plus-rev6-panel` | 60 Hz |
| `rg35xx-sp-v2-panel` | about 59.52 Hz |
| `rg40xx-panel` | 60 / 120 Hz |
| `rg40xx-v2-panel` | 60 Hz |
| `rgcubexx-panel` | 60 / 120 Hz |
| `rg28xx-panel` | 60 Hz |

These are firmware mode tables, not a blanket guarantee about every hardware
revision in those model families. The extra families are not enabled by this
Slot change. In particular, absence of a 120 Hz entry is not proof that a panel
cannot be overclocked; it provides no affirmative evidence for enabling it here.

The [RG34XX firmware](https://github.com/ROCKNIX/distribution/blob/c17558d2abadebec634100264523aaddc98b3b96/projects/ROCKNIX/packages/linux-firmware/kernel-firmware/extra-firmware/panels/anbernic,rg34xx-panel.panel)
and [original RG34XX SP firmware](https://github.com/ROCKNIX/distribution/blob/c17558d2abadebec634100264523aaddc98b3b96/projects/ROCKNIX/packages/linux-firmware/kernel-firmware/extra-firmware/panels/anbernic,rg34xx-sp-panel.panel)
use 26/52 MHz pixel clocks with 813×533 totals, giving about 60/120 Hz.
Slot retains the vendor kernel's 820×536 timings; its tested register values
produce about 52.8 MHz and 120.13 Hz. The method is the same clock increase,
while the kernel-specific timings differ.

The non-SP vendor identity and stock timings are documented in the upstream
developer's [RG34XX device-tree dump](https://gist.github.com/macromorgan/3e12146a63b53c5b61e4e3c7d4547a0e).
This is source evidence for that exact combination, not a live test of a
non-SP RG34XX in this session.

To reproduce the decoding, download the `.panel` files from the pinned
[firmware directory](https://github.com/ROCKNIX/distribution/tree/c17558d2abadebec634100264523aaddc98b3b96/projects/ROCKNIX/packages/linux-firmware/kernel-firmware/extra-firmware/panels)
and run:

```sh
python3 -B tools/refresh/rocknix_modes.py /tmp/rocknix-panels \
  --revision c17558d2abadebec634100264523aaddc98b3b96
```
