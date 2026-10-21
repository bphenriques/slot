use slot_retro::{AchievementMemory, ButtonMask, LibretroCore, RetroCore};
use std::path::Path;

#[test]
fn real_cores_expose_game_boy_color_and_advance_in_ra_address_order() {
    // An original tiny program writes different markers to the two banks, then spins.
    // This catches both reversed banks and gpSP's nonzero IWRAM descriptor offset.
    let code: [u32; 7] = [
        0xe3a00403, 0xe3a01012, 0xe5c01000, 0xe3a00402, 0xe3a01034, 0xe5c01000, 0xeafffffe,
    ];
    let mut rom = vec![0u8; 0x8000];
    rom[..4].copy_from_slice(&0xea00002eu32.to_le_bytes());
    rom[0xa0..0xac].copy_from_slice(b"SLOT RA TEST");
    rom[0xac..0xb0].copy_from_slice(b"SLTE");
    rom[0xb0..0xb2].copy_from_slice(b"00");
    rom[0xb2] = 0x96;
    let sum = rom[0xa0..0xbd].iter().fold(0u8, |a, b| a.wrapping_add(*b));
    rom[0xbd] = 0u8.wrapping_sub(sum).wrapping_sub(0x19);
    for (i, instruction) in code.iter().enumerate() {
        rom[0xc0 + i * 4..0xc4 + i * 4].copy_from_slice(&instruction.to_le_bytes());
    }
    let rom_path = Path::new(env!("CARGO_TARGET_TMPDIR")).join("ra-memory.gba");
    std::fs::write(&rom_path, rom).unwrap();
    for name in ["mgba", "gpsp"] {
        let dylib = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../vendor/{name}_libretro.{}",
            std::env::consts::DLL_EXTENSION
        ));
        if !dylib.exists() {
            eprintln!("skipping {name}: host core unavailable");
            continue;
        }
        let mut core = LibretroCore::open(&dylib).unwrap();
        core.load(&rom_path).unwrap();
        for _ in 0..30 {
            core.run_frame(ButtonMask::default());
            core.take_audio();
        }
        let mut ram = vec![0; 0x58000];
        let valid = core.achievement_memory(&mut ram);
        assert_eq!(valid.regions()[0].start, 0, "{name}");
        assert_eq!(valid.regions()[0].len, 0x8000, "{name}");
        assert_eq!(valid.regions()[1].start, 0x8000, "{name}");
        assert_eq!(valid.regions()[1].len, 0x40000, "{name}");
        assert_eq!(ram[0], 0x12, "{name}: IWRAM/descriptor offset");
        assert_eq!(ram[0x8000], 0x34, "{name}: EWRAM/address order");
        assert_eq!(
            core.achievement_memory(&mut [0; 10]),
            AchievementMemory::default()
        );
    }

    let dylib = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../vendor/mgba_libretro.{}",
        std::env::consts::DLL_EXTENSION
    ));
    if !dylib.exists() {
        return;
    }
    for (ext, model) in [("gb", "Game Boy"), ("gbc", "Game Boy Color")] {
        let mut rom = vec![0; 0x8000];
        rom[0x100..0x104].copy_from_slice(&[0x00, 0xc3, 0x50, 0x01]);
        rom[0x104..0x108].copy_from_slice(&[0xce, 0xed, 0x66, 0x66]);
        rom[0x143] = if ext == "gbc" { 0x80 } else { 0 };
        rom[0x147] = 0x03; // MBC1 + RAM + battery
        rom[0x149] = 0x03; // Four cartridge RAM banks
        rom[0x14d] = rom[0x134..0x14d]
            .iter()
            .fold(0u8, |sum, byte| sum.wrapping_sub(*byte).wrapping_sub(1));
        // Write fixed WRAM, bank 1, HRAM, and two cartridge banks. Select GBC WRAM
        // bank 2 after writing bank 1, so the snapshot must preserve both banks.
        let mut code = vec![
            0x3e, 0x12, 0xea, 0x00, 0xc0, 0x3e, 0x34, 0xea, 0x00, 0xd0, 0x3e, 0x56, 0xe0, 0x80,
            0x3e, 0x0a, 0xea, 0x00, 0x00, 0x3e, 0x78, 0xea, 0x00, 0xa0, 0x3e, 0x01, 0xea, 0x00,
            0x60, 0x3e, 0x01, 0xea, 0x00, 0x40, 0x3e, 0x9a, 0xea, 0x00, 0xa0,
        ];
        if ext == "gbc" {
            code.extend_from_slice(&[
                0x3e, 0x02, 0xe0, 0x70, 0x3e, 0xbc, 0xea, 0x00, 0xd0, 0x3e, 0x07, 0xe0, 0x70, 0x3e,
                0xde, 0xea, 0x00, 0xd0,
            ]);
        }
        code.extend_from_slice(&[0x18, 0xfe]);
        rom[0x150..0x150 + code.len()].copy_from_slice(&code);
        let path = rom_path.with_extension(ext);
        std::fs::write(&path, rom).unwrap();
        let mut core = LibretroCore::open(&dylib).unwrap();
        core.set_option("mgba_gb_model", model);
        core.load(&path).unwrap();
        for _ in 0..30 {
            core.run_frame(ButtonMask::default());
            core.take_audio();
        }
        let mut ram = vec![0; 0x58000];
        let valid = core.achievement_memory(&mut ram);
        for (address, value) in [
            (0xc000, 0x12),
            (0xd000, 0x34),
            (0xe000, 0x12),
            (0xff80, 0x56),
            (0xa000, 0x78),
            (0x16000, 0x9a),
        ] {
            assert_eq!(ram[address], value, "{ext}: RA address {address:#x}");
        }
        let readable = |address| {
            valid
                .regions()
                .iter()
                .any(|r| (r.start..r.start + r.len).contains(&address))
        };
        assert!(!readable(0xfea0), "unusable RAM is not observed zero RAM");
        assert!(!readable(0x1c000), "absent cartridge banks remain invalid");
        assert_eq!(readable(0x10000), ext == "gbc");
        if ext == "gbc" {
            assert_eq!(ram[0x10000], 0xbc, "GBC WRAM bank 2");
            assert_eq!(ram[0x15000], 0xde, "GBC WRAM bank 7");
        }
    }
}
