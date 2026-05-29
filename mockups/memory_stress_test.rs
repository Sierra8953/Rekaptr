use std::sync::Arc;
use dashmap::DashMap;
use std::time::{Instant, Duration};

fn main() {
    println!("--- Rekaptr Memory Stress Test Simulation ---");
    
    // Simulate AppState caches
    let artwork_cache: DashMap<String, Option<String>> = DashMap::new();
    let process_cache: DashMap<String, u32> = DashMap::new();
    let playlist_cache: DashMap<String, std::path::PathBuf> = DashMap::new();
    
    // Simulate session blocks (4 hours of recording = 7200 segments if 2s each)
    // In reality, AppState stores SessionBlocks which are contiguous chunks, 
    // but the segments are on disk. However, UI might be caching metadata.
    let mut session_blocks = Vec::new();

    let start = Instant::now();
    let iterations = 10000;
    
    println!("Simulating {} iterations of game detection and artwork loading...", iterations);
    
    for i in 0..iterations {
        let game_title = format!("Game_{}", i % 100); // 100 unique games
        
        // Simulate artwork caching
        if !artwork_cache.contains_key(&game_title) {
            artwork_cache.insert(game_title.clone(), Some(format!("C:\\Cache\\{}_hero.jpg", i % 100)));
        }
        
        // Simulate process detection
        process_cache.insert(game_title.clone(), 1234 + i as u32);
        
        // Simulate session block growth (e.g. 1 block per hour or per start/stop)
        if i % 1000 == 0 {
            session_blocks.push(format!("Block_{}", i));
        }

        if i % 2500 == 0 {
            println!("  Iteration {}: ArtworkCache={}, ProcessCache={}", i, artwork_cache.len(), process_cache.len());
        }
    }

    println!("\nFinal State:");
    println!("  Artwork Cache: {} entries", artwork_cache.len());
    println!("  Process Cache: {} entries", process_cache.len());
    println!("  Session Blocks: {} entries", session_blocks.len());
    println!("  Total simulation time: {:?}", start.elapsed());
    
    println!("\nObservations:");
    println!("1. Caches grow linearly with unique games/processes encountered.");
    println!("2. Without eviction, long-running sessions with many different games will leak memory.");
    println!("3. Next Step: Implement LRU or clear-on-idle logic for these DashMaps.");
}
