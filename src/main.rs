use clap::Parser;
use cluster_event::{Event, clust_analysis, clust_analysis_cutoff, clust_analysis_cutoff_highest_toa, load_hdf5, load_hdf5_parallel};
use cluster_event::{write_hdf5_event, write_hdf5_clust};

use cluster_event::tpx::{load_tpx3};
use std::fs::File;
use std::io::Write;
use std::time::Instant;
use std::thread::{scope};
#[derive(Parser, Debug)]
#[command(author, version, about = "Evaluates Clusters from Electron Microscopy", long_about = None)]
struct Args {
    /// HDF5 File
    #[arg(
        short,
        long,
        default_value_t = ("./example_measurement.hdf5").to_string()
    )]
    file: String,

    /// Run benchmark
    #[arg(short, long, default_value_t = false)]
    bench: bool,

    /// Maximum Pixel distance
    #[arg(short = 'p', long, default_value_t = 5)]
    eps_pixel: u16,

    /// Maximum Time distance [s]
    #[arg(short = 't', long, default_value_t = 50e-9)]
    eps_time: f64,

    /// Length of the Buffer
    #[arg(short, long, default_value_t = 0)]
    cutoff: usize,

    /// Output events/hits
    #[arg(short, long, default_value_t = false)]
    save_events: bool,
    
    /// Output HDF5 File for events/hits
    #[arg(short = 'e', long)]
    output_event: Option<String>,

    /// Output HDF5 File for clusters
    #[arg(short = 'o', long, default_value_t = ("clusters.hdf5").to_string())]
    output: String,

    /// Number of threads for parallel processing
    #[arg(short = 'n', long, default_value_t = 1)]
    n_threads: usize,

    /// Min tot for electron hits. tot's below this will be discarded when reading .tpx3
    #[arg(short = 'm', long, default_value_t = 5)]
    min_tot: u16,

    /// Sort window size for TPX3 data (longer is better)
    #[arg(short = 's', long, default_value_t = 10_000)]
    sort_window: usize,
}



fn main() -> std::io::Result<()> {
    let args = Args::parse();

    let mut save_events = false; 
    let mut save_path = "output_hits";
    if let Some(path) = &args.output_event {
        save_events = true; save_path = path;
    }

    println!("File: {}", args.file);
    if args.bench {
        calc_sum(&args.file, args.eps_pixel, args.eps_time, args.cutoff)?;
        return Ok(());
    }

    if args.n_threads == 1 {
        println!("reading h5...");
        let hits: Vec<Event> = load_hdf5(&args.file).unwrap();
        let n_hits: usize = hits.len();
        println!("nhits = {}", n_hits);
        println!("finished reading h5");
        if args.cutoff == 0 {
            let clusters = clust_analysis(&hits, args.eps_pixel, args.eps_time);
            println!("Found {} clusters", clusters.len());
            write_hdf5_clust(&args.output, &clusters);
        } else {
            let clusters = clust_analysis_cutoff_highest_toa(&hits, args.eps_pixel, args.eps_time, args.cutoff);
            println!("Found {} clusters", clusters.len());
            println!("writing h5 file...");
            write_hdf5_clust(&args.output, &clusters);
            println!("done!")
        }
    }

    // for multi thread processing:
    else {
        println!("reading TPX3...");
        let hits: Vec<Event> = load_tpx3(&args.file, args.n_threads, args.min_tot, args.sort_window).unwrap();
        println!("finished reading tpx");


        if save_events {
            println!("writing hits...");
            write_hdf5_event(&save_path, &hits).unwrap();
        }

        let n_hits: usize = hits.len();
        println!("nhits = {}", n_hits);
        let n_threads: usize = args.n_threads;
        let hit_section_len: usize = n_hits.div_ceil(n_threads);
        scope(|s| {
            let mut threads= Vec::with_capacity(n_threads);
            let mut clusters = Vec::with_capacity(n_hits/2);
            for hits_section in hits.chunks(hit_section_len) {
                // threads start running here:
                threads.push(s.spawn(|| {
                    println!("clustering hits thread started");
                    clust_analysis_cutoff_highest_toa(hits_section, args.eps_pixel, args.eps_time, args.cutoff)
                }));
            }
            for thread in threads{
                // wait for each thread to finish before appending its result to clusterd hits:
                clusters.extend(thread.join().unwrap());
                println!("clustering hits thread finished");
            }
            println!("clusters length {}",clusters.len());
            println!("writing h5 file...");
            write_hdf5_clust(&args.output, &clusters);
            println!("done!")
        });

 
        


    } 
    return Ok(());
}

fn calc_sum(path: &str, eps_pixel: u16, eps_time: f64, cutoff: usize) -> std::io::Result<()> {
    println!("Run time measurment");
    let hits: Vec<Event> = load_hdf5(path).unwrap();
    let num_average = 5;
    let mut file: File = File::create("speed.csv")?;
    write!(file, "num_hits freq[MHz] time[microsec]\n")?;
    for len in 1..=100 {
        let length = len * hits.len() / 100;
        let sliced_hits = &hits[1..length];

        let duration = if cutoff > 0 {
            println!("Calculating with {} as cutoff", cutoff);
            let start = Instant::now();
            for _ in 0..num_average {
                let _dat = clust_analysis_cutoff(sliced_hits, eps_pixel, eps_time, cutoff);
            }
            start.elapsed() / num_average
        } else {
            let start = Instant::now();
            for _ in 0..num_average {
                let _dat = clust_analysis(sliced_hits, eps_pixel, eps_time);
            }
            start.elapsed() / num_average
        };
        let freq = (sliced_hits.len() as f64) / (duration.as_micros() as f64);
        write!(file, "{} {:?} {:?}\n", length, freq, duration.as_micros())?;
    }
    return Ok(());
}
