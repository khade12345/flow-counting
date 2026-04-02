use clap::Parser;
use cluster_event::{Event, cluster_hits, load_hdf5};
use cluster_event::{write_hdf5_event, write_hdf5_clust};

use cluster_event::tpx::{load_tpx3};

#[derive(Parser, Debug)]
#[command(author, version, about = "Evaluates Clusters from Electron Microscopy", long_about = None)]
struct Args {
    /// input .hdf5 (fields x,y,toa,tot) or .tpx3 File
    #[arg(
        short,
        long,
        default_value_t = ("./example_measurement.hdf5").to_string()
    )]
    input: String,

    /// Maximum Pixel distance
    #[arg(short = 'p', long, default_value_t = 5)]
    eps_pixel: u16,

    /// Maximum Time distance [s]
    #[arg(short = 't', long, default_value_t = 50e-9)]
    eps_time: f64,

    /// Length of the Buffer
    #[arg(short, long, default_value_t = 0)]
    cutoff: usize,
    
    /// Output HDF5 File for events/hits
    #[arg(short = 'e', long)]
    output_hits: Option<String>,

    /// Output HDF5 File for clusters
    #[arg(short = 'o', long, default_value_t = ("clusters.hdf5").to_string())]
    output_clusters: String,

    /// Number of threads for parallel processing
    #[arg(short = 'n', long, default_value_t = 1)]
    n_threads: usize,

    /// Min tot for electron hits. tot's below this will be discarded when reading .tpx3
    #[arg(short = 'm', long, default_value_t = 5)]
    min_tot: u16,
}


fn main() -> std::io::Result<()> {
    let args = Args::parse();

    let mut save_events = false; 
    let mut save_path = "output_hits";
    if let Some(path) = &args.output_hits {
        save_events = true; save_path = path;
    }

    println!("File: {}", args.input);
    println!("reading file...");
    
    let hits: Vec<Event> = if args.input.ends_with(".tpx3") {
        println!("detected TPX3 format");
        load_tpx3(&args.input, args.n_threads, args.min_tot).unwrap()
    } else if args.input.ends_with(".hdf5") || args.input.ends_with(".h5") {
        println!("detected HDF5 format");
        load_hdf5(&args.input).unwrap()
    } else {
        panic!("Unsupported file format. Expected .tpx3 or .hdf5/.h5");
    };
    println!("finished reading file");

    if save_events {
        println!("writing hits in: {}", &args.output_hits.clone().unwrap());
        write_hdf5_event(&save_path, &hits).unwrap();
    }

    let clusters = cluster_hits(&hits, args.eps_pixel, args.eps_time, args.cutoff, args.n_threads, ).unwrap();

    println!("writing h5 file...");
    write_hdf5_clust(&args.output_clusters, &clusters);
    println!("done!");
    Ok(())
}
