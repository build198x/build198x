# format-commodore-amiga-adf

Read, write, and verify Commodore Amiga **ADF** disk images in Rust — the
880 KB floppy images an Amiga boots from. Dependency-free (`core`/`std` only),
deterministic, and panic-free on malformed input.

It handles both floppy filesystems (**OFS** and **FFS**), arbitrary file and
directory trees of any size, bootable and plain data disks, and the read side
too — `list`, `read`, and a full checksum `verify`. Rust's existing `adflib`
leaves disk *writing* unimplemented; this crate writes byte-for-byte
reproducible disks and reads them back.

## Write a bootable disk

```rust
use format_commodore_amiga_adf::{master, FileSystem, Volume};

// The common case: one executable that boots straight into the program.
let adf: Vec<u8> = master(&hunk_exe, "mygame", "MyGame")?;
std::fs::write("mygame.adf", &adf)?;

// Or an arbitrary tree, bootable or a data disk:
let mut vol = Volume::new("MyDisk", FileSystem::Ofs);
vol.add_file("c/hello", &command_bytes)?;      // creates the `c` directory
vol.add_file("s/startup-sequence", b"c/hello\n")?;
vol.set_bootable(true);
let adf = vol.build()?;                          // 901,120 bytes, deterministic
```

## Read one back

```rust
use format_commodore_amiga_adf::Disk;

let disk = Disk::open(&adf)?;
assert_eq!(disk.label(), "MyDisk");
for entry in disk.list("c")? {
    println!("{} ({} bytes)", entry.name, entry.size);
}
let bytes = disk.read("c/hello")?;
disk.verify()?;   // every checksum: boot, root, bitmap, headers, data
```

## Notes

- **OFS vs FFS.** OFS (`DOS\0`) is the default and boots on any Kickstart,
  including a bare A500/KS1.3. FFS (`DOS\1`) is denser and faster but boots only
  on **Kickstart 2.0+** — the 1.3 ROM's floppy filesystem is OFS-only. Choose
  with `Volume::new(label, FileSystem::Ffs)` or `master_fs`.
- **Deterministic.** The same inputs always produce identical bytes — dates are
  zeroed and block allocation is fixed — so committed `.adf` files are
  reproducible.
- **Panic-free reads.** `Disk` range-checks every block pointer and bounds every
  chain, so a corrupt image returns `Error::Corrupt`, never a panic.
- **Scope.** Double-density 880 KB floppies, OFS and FFS. The
  International/Dir-Cache filesystem variants, hard-disk (RDB) layouts, and
  multi-disk sets are not yet covered.

## Part of the 198x family

This crate powers the Amiga disk-mastering step of [Build198x], the build-tools
pipeline for the [198x] retro-computing project. The standalone `build198x-adf`
binary wraps it as a command-line tool.

## Licence

GPL-2.0-or-later. The 198x emulator/tooling family is copyleft throughout — it
composes freely with the GPL retro-computing ecosystem most Amiga tooling lives
in. If you build on this crate, your work inherits those terms.

[198x]: https://github.com/build198x
[Build198x]: https://github.com/build198x/build198x
