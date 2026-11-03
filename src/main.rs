use std::env;
use std::path::PathBuf;
use std::process;

use rustkv::Bitcask;

fn print_usage() {
    eprintln!("Usage: rustkv [--dir <PATH>] <COMMAND> [ARGS...]");
    eprintln!();
    eprintln!("Commands:");
    eprintln!("  set <key> <value>   Store or update a key-value entry");
    eprintln!("  get <key>           Retrieve the value for a key");
    eprintln!("  rm <key>            Delete a key via tombstone");
    eprintln!("  keys                List all live keys in sorted order");
    eprintln!("  compact             Compact segments and generate .hint index");
    eprintln!("  stats               Display live keys, segments, and uncompacted waste");
}

fn main() {
    let mut args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        print_usage();
        process::exit(1);
    }

    let mut data_dir = PathBuf::from("data");
    if args.len() >= 2 && args[0] == "--dir" {
        data_dir = PathBuf::from(&args[1]);
        args.drain(0..2);
    }

    if args.is_empty() {
        print_usage();
        process::exit(1);
    }

    let mut db = match Bitcask::open(&data_dir) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("error opening storage dir {:?}: {}", data_dir, e);
            process::exit(1);
        }
    };

    match args[0].as_str() {
        "set" => {
            if args.len() != 3 {
                eprintln!("Usage: rustkv set <key> <value>");
                process::exit(1);
            }
            if let Err(e) = db.put(&args[1], args[2].as_bytes().to_vec()) {
                eprintln!("error: {}", e);
                process::exit(1);
            }
            println!("OK");
        }
        "get" => {
            if args.len() != 2 {
                eprintln!("Usage: rustkv get <key>");
                process::exit(1);
            }
            match db.get(&args[1]) {
                Ok(Some(bytes)) => println!("{}", String::from_utf8_lossy(&bytes)),
                Ok(None) => {
                    eprintln!("(nil)");
                    process::exit(2);
                }
                Err(e) => {
                    eprintln!("error: {}", e);
                    process::exit(1);
                }
            }
        }
        "rm" => {
            if args.len() != 2 {
                eprintln!("Usage: rustkv rm <key>");
                process::exit(1);
            }
            match db.delete(&args[1]) {
                Ok(true) => println!("DELETED"),
                Ok(false) => println!("NOT_FOUND"),
                Err(e) => {
                    eprintln!("error: {}", e);
                    process::exit(1);
                }
            }
        }
        "keys" => {
            for k in db.list_keys() {
                println!("{}", k);
            }
        }
        "compact" => match db.compact() {
            Ok(seg_id) => println!("Compacted into segment {:06}.data", seg_id),
            Err(e) => {
                eprintln!("compaction error: {}", e);
                process::exit(1);
            }
        },
        "stats" => {
            let st = db.stats();
            println!("Live Keys         : {}", st.live_keys);
            println!("Active Segment ID : {:06}", st.active_file_id);
            println!("Total Segments    : {}", st.total_segments);
            println!("Uncompacted Bytes : {}", st.uncompacted_bytes);
        }
        _ => {
            print_usage();
            process::exit(1);
        }
    }
}
