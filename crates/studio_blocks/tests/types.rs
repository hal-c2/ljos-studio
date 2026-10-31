//! `ljostypes.json` says what `register` registers: Studio's build script
//! compiles the shell against it, running none of this crate's code.
//! `LJOS_TYPES=write cargo test -p studio_blocks --test types` writes it.

#[test]
fn the_type_dump_file_is_current() {
    ljos::types::check_file("ljostypes.json".as_ref(), studio_blocks::register).unwrap();
}
