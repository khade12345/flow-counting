use memmap2::Mmap;
// use rayon::prelude::*;
use crate::{Event};
use std::thread::{scope};
use rayon::prelude::*;


pub struct Chunk { 
    start: usize, //excluding header
    payload_size: u16,
    chip_idx: u8,
}

/*
 * Loads Events from a TPX3 file.
 */
pub fn load_tpx3(path: &str, n_threads: usize, tot_threshold: u16) -> Result<Vec<Event>, Box<dyn std::error::Error>> {
    let file = std::fs::File::open(path)?;
    let mmap = unsafe { memmap2::Mmap::map(&file)? };

    let tpx_chunks = find_chunks(&mmap).unwrap();
    println!("Found {} TPX3 chunks", tpx_chunks.len());

    println!("Translating and sorting chunks.");    

    let out_vec = scope(|s| {
        let mmap_ref = &mmap;
        let mut out_vec: Vec<Event> = Vec::<Event>::with_capacity(n_threads);
        let mut threads = Vec::with_capacity(n_threads);

        let n_chunks = tpx_chunks.len();
        let chunk_segment: usize = n_chunks.div_ceil(n_threads);
        for chunks_list in tpx_chunks.chunks(chunk_segment) { 
            threads.push(s.spawn( move || {
                    println!("read + sort tpx thread started!");
                process_chunks(chunks_list, mmap_ref, &tot_threshold).unwrap()
            }));
        }
        for thread in threads{
            out_vec.extend(thread.join().unwrap());
            println!("read + sort tpx thread finished!");
        }
        println!("sorting!");
        // for chunk_vec in out_vec.chunks_mut(sort_window) {
        //     chunk_vec.par_sort_unstable_by(|a, b| a.time.cmp(&b.time));
        // }
        out_vec.par_sort_unstable_by(|a, b| a.time.cmp(&b.time));
        println!("sorted!");
        out_vec
    });

    Ok(out_vec)

}


/*
 * Corrects raw pixel coordinates based on the Chip ID and detector geometry.
 * cross_offset accounts for the physical gap between chips.
 */
 fn apply_chip_correction(raw_x: u16, raw_y: u16, chip_id: u8, cross_offset: u16) -> (u16, u16) {
    let offset = 256 + 2 * cross_offset;

    match chip_id {
        // Chip 0: Flipped Y, shifted right and up
        0 => (raw_x + offset, 255 - raw_y + offset),
        // Chip 1: Flipped X, shifted right
        1 => (255 - raw_x + offset, raw_y),
        // Chip 2: Flipped X
        2 => (255 - raw_x, raw_y),
        // Chip 3: Flipped Y, shifted up
        3 => (raw_x, 255 - raw_y + offset),
        // Chip 4: Flipped Y, shifted further right (offset2) and up
        4 => (raw_x + offset*2, 255 - raw_y + offset),
        // Chip 5: Rotated (X/Y swap), shifted furthest right (offset3) and up
        5 => (255 - raw_y + offset*3, 255 - raw_x + offset),
        // Chip 6: Flipped X, shifted further right
        6 => (255 - raw_x + offset*2, raw_y),
        // Chip 7: Rotated (X/Y swap), shifted furthest right (offset3)
        7 => (255 - raw_y + offset*3, 255 - raw_x),
        // Default case if chip_id is unexpected
        _ => (raw_x, raw_y),
    }
}

/*
 * Parse an 0xb type TPX3 pixel data packet
 */
fn parse_pixel(pkg: u64, rollover: u64, chip_idx: u8, cross_offset: u16) -> Event {
    // Unpack Raw Bitfields
    let pix_addr = (pkg >> 44) & 0xFFFF; 
    let dcol = (pix_addr >> 9) & 0b0111_1111; 
    let spix = (pix_addr >> 3) & 0b0011_1111;
    let pix = pix_addr & 0b0111;

    let raw_x = (dcol * 2 + (pix / 4)) as u16;
    let raw_y = (spix * 4 + (pix % 4)) as u16;

    let tot = (pkg >> 20) & 0x3FF; 
    let toa = (pkg >> 30) & 0x3FFF;
    let ftoa = (pkg >> 16) & 0xF;  
    let spidr_time = pkg & 0xFFFF; 

    let (x, y) = apply_chip_correction(raw_x, raw_y, chip_idx, cross_offset);

    // Time Calculation. Detector scans 10^12 times/ second -> 
    let time: i64 = ((rollover << 34) | (spidr_time << 18) | (toa << 4)) as i64 - ftoa as i64;

    Event {
        x,
        y,
        time,
        intens: tot as u16,
    }
}

/*
 * Loops through TPX3 headers and locates data chunks
 */
fn find_chunks(mmap: &Mmap) -> Result<Vec<Chunk>, Box<dyn std::error::Error>>{
    let n_bytes = mmap.len();
    let mut chunks = Vec::with_capacity(4*(mmap.len()/8)/1000); // numbers of 64 bit hits per chunk is around 1000
    let mut cursor = 0;

    while cursor + 8 <= n_bytes {
        let header_pkg = u64::from_le_bytes(mmap[cursor..cursor+8].try_into()?);

        // Check for "TPX3" (0x33585054)
        if header_pkg & 0xFFFFFFFF == 0x33585054 {
            let payload_size = (header_pkg >> 48) as u16;
            let chip_idx = (header_pkg>>32 & 0b1111_1111) as u8;
            
            // Store where the data header and how long the data is in bytes and the chip no.
            chunks.push(Chunk {
                start: cursor + 8,
                payload_size,
                chip_idx,
            });
            cursor += 8 + payload_size as usize;

        } else {
            // DEBUG: If it fails, print the hex of where we are 
            // and the surrounding 16 bytes to see if "TPX3" is nearby
            let start = cursor.saturating_sub(16);
            let end = (cursor + 16).min(n_bytes);
            println!("Failed at cursor {}. Hex at cursor: {:016x}", cursor, header_pkg);
            println!("Surrounding data: {:02x?}", &mmap[start..end]);
            return Err("Invalid TPX3 header signature at header position".into());
        }
    }
    Ok(chunks)
}

/*
* Loops through data chunks and converts them to hits
*/
fn process_chunks(chunks: &[Chunk], mmap: &Mmap, &tot_threshold: &u16) -> Result<Vec<Event>, Box<dyn std::error::Error>> {

    // iterate through each chunk
    let mut rollover_count: u32 = 0;
    let mut approaching_rollover = false;
    let mut leaving_rollover = true;

    let mut out_vec: Vec<Event> = Vec::<Event>::with_capacity(mmap.len()/8);
    for chunk in chunks.iter() {
        let payload_start: usize = chunk.start;
        let payload_size = chunk.payload_size as usize;
        let payload_end = payload_start + payload_size;
        let chip_idx = chunk.chip_idx;

        
        let mut cursor = payload_start+8;
        let mut chunk_vec: Vec<Event> = Vec::with_capacity(10000);

        while cursor + 8 <= payload_end{
            let pkg = u64::from_le_bytes(mmap[cursor..cursor+8].try_into()?);

            if pkg>>60 == 0xb {
                let spidr_time = pkg & 0xFFFF;
                let mut current_rollover = rollover_count;
            
                // 1. Handle High-Time Packets (End of cycle)
                if spidr_time > 58982 { // 0.9 * 65536
                    if leaving_rollover {
                        // This is an out-of-order packet from the previous cycle
                        current_rollover = rollover_count.saturating_sub(1);
                    } else if !approaching_rollover {
                        approaching_rollover = true;
                    }
                }
            
                // 2. Trigger the Rollover (Transition from High to Low)
                if spidr_time < 655 && approaching_rollover { // 0.01 * 65536
                    approaching_rollover = false;
                    leaving_rollover = true;
                    rollover_count += 1;
                    current_rollover = rollover_count;
                }
            
                // 3. Clear the Transition State
                if leaving_rollover && spidr_time > 6553 { // 0.1 * 65536
                    approaching_rollover = false;
                    leaving_rollover = false;
                }
                let event: Event = parse_pixel(pkg, current_rollover as u64, chip_idx, 2);
                // typically ignore tot values less than 5 (afterpulses)
                if event.intens>=tot_threshold {
                    chunk_vec.push(event);
                }
              
            }
            else {
                //TODO add TDC signal saver
            }

            cursor += 8;
        }
        //chunk_vec.sort_unstable_by(|a, b| a.time.cmp(&b.time));
        // println!("filtered chunk length: {}", chunk_vec.len());
        out_vec.extend(chunk_vec);
    }

    Ok(out_vec)
}
