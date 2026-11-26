//! Release-time binary patch generator and applier (bidiff).
//!
//! Build with the feature, e.g.
//! `cargo build --release --bin patchgen --features patchgen`
//!
//! Usage:
//!   patchgen diff  <old.exe> <new.exe> <patch>
//!   patchgen apply <old.exe> <patch>   <out.exe>
//!
//! `apply` verifies the base and result hashes carried by the patch.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 4 || (args[0] != "diff" && args[0] != "apply") {
        eprintln!("usage: patchgen <diff|apply> <old> <new-or-patch> <out>");
        std::process::exit(2);
    }

    #[cfg(feature = "patchgen")]
    {
        let old = std::fs::read(&args[1]).expect("read old file");
        match args[0].as_str() {
            "diff" => {
                let new = std::fs::read(&args[2]).expect("read new file");
                let params = bidiff::DiffParams::with_threads(32, Some(1 << 20), None).expect("diff params");
                let out = std::fs::File::create(&args[3]).expect("create patch");
                let mut out = std::io::BufWriter::new(out);
                bidiff::simple_diff_chunked_with_params(&old, &new, &mut out, &params, 3).expect("diff");
                use std::io::Write;
                out.flush().expect("flush patch");
                println!("wrote {}", args[3]);
            }
            "apply" => {
                let patch_data = std::fs::read(&args[2]).expect("read patch");
                let patch = bidiff::patch::read_patch(&patch_data).expect("parse patch");
                let base = patch.verify_base(&old).expect("patch is for another base");
                let mut out = std::fs::File::create(&args[3]).expect("create output");
                base.apply(&mut out).expect("apply patch");
                println!("wrote {}", args[3]);
            }
            _ => unreachable!(),
        }
    }

    #[cfg(not(feature = "patchgen"))]
    {
        eprintln!("patchgen was built without --features patchgen");
        std::process::exit(2);
    }
}
