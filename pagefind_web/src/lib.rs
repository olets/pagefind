#![allow(clippy::not_unsafe_ptr_arg_deref)]

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

use std::collections::HashMap;

use bit_set::BitSet;
use excerpt::calculate_excerpt;
use rust_stemmers::{Algorithm, Stemmer}; // TODO: too big
use wasm_bindgen::prelude::*;

mod excerpt;
mod index;
mod metadata;
mod util;

pub struct PageWord {
    page: u32,
    locs: Vec<u32>,
}

pub struct IndexChunk {
    from: String,
    to: String,
    hash: String,
}

pub struct SearchIndex {
    web_version: &'static str,
    generator_version: Option<String>,
    pages: Vec<String>,
    chunks: Vec<IndexChunk>,
    stops: Vec<String>,
    words: HashMap<String, Vec<PageWord>>,
}

#[cfg(debug_assertions)]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[cfg(debug_assertions)]
fn debug_log(s: &str) {
    log(&format!("From WASM: {}", s));
}

#[wasm_bindgen]
pub fn init_pagefind(metadata_bytes: &[u8]) -> *mut SearchIndex {
    #[cfg(debug_assertions)]
    debug_log("Initializing Pagefind");
    let mut search_index = SearchIndex {
        web_version: env!("CARGO_PKG_VERSION"),
        generator_version: None,
        pages: Vec::new(),
        chunks: Vec::new(),
        stops: Vec::new(),
        words: HashMap::new(),
    };

    match search_index.decode_metadata(metadata_bytes) {
        Ok(_) => Box::into_raw(Box::new(search_index)),
        Err(e) => {
            #[cfg(debug_assertions)]
            debug_log(&format!("{:#?}", e));
            std::ptr::null_mut::<SearchIndex>()
        }
    }
}

#[wasm_bindgen]
pub fn load_index_chunk(ptr: *mut SearchIndex, chunk_bytes: &[u8]) -> *mut SearchIndex {
    #[cfg(debug_assertions)]
    debug_log("Loading Index Chunk");
    let mut search_index = unsafe { Box::from_raw(ptr) };

    match search_index.decode_index_chunk(chunk_bytes) {
        Ok(_) => Box::into_raw(search_index),
        Err(e) => {
            #[cfg(debug_assertions)]
            debug_log(&format!("{:#?}", e));
            std::ptr::null_mut::<SearchIndex>()
        }
    }
}

#[wasm_bindgen]
pub fn request_indexes(ptr: *mut SearchIndex, query: &str) -> String {
    #[cfg(debug_assertions)]
    debug_log(&format! {"Finding the index chunks needed for {:?}", query});

    let search_index = unsafe { Box::from_raw(ptr) };
    let mut indexes = Vec::new();
    let terms = query.split(' ');

    for term in terms {
        let term_index = search_index
            .chunks
            .iter()
            .find(|chunk| term >= &chunk.from && term <= &chunk.to);
        if let Some(index) = term_index {
            indexes.push(index.hash.clone())
        }
    }

    let _ = Box::into_raw(search_index);
    indexes.join(" ")
}

#[wasm_bindgen]
pub fn search(ptr: *mut SearchIndex, query: &str) -> String {
    let search_index = unsafe { Box::from_raw(ptr) };

    if let Some(generator_version) = search_index.generator_version.as_ref() {
        if generator_version != search_index.web_version {
            let _ = Box::into_raw(search_index);
            return "ERROR: Version mismatch".into();
        }
    }

    let terms = query.split(' ');
    // TODO: i18n
    let en_stemmer = Stemmer::create(Algorithm::English);

    #[cfg(debug_assertions)]
    debug_log(&format! {"Searching {:?}", query});

    let mut maps = Vec::new();
    let mut words = Vec::new();
    for term in terms {
        let term = en_stemmer.stem(term).into_owned();
        if let Some(word_index) = search_index.words.get(&term) {
            words.extend(word_index);
            let mut set = BitSet::new();
            for page in word_index {
                set.insert(page.page as usize);
            }
            maps.push(set);
        }
    }

    let mut maps = maps.drain(..);
    let mut results = if let Some(map) = maps.next() {
        map
    } else {
        let _ = Box::into_raw(search_index);
        return "".into();
    };

    for map in maps {
        results.intersect_with(&map);
    }

    let mut pages: Vec<String> = vec![];

    for page in results.iter() {
        let locs: Vec<u32> = words
            .iter()
            .filter_map(|p| {
                if p.page as usize == page {
                    Some(p.locs.clone())
                } else {
                    None
                }
            })
            .flatten()
            .collect();
        pages.push(format!(
            "{}@{},{}@{}",
            &search_index.pages[page],
            calculate_excerpt(&locs, 30),
            30,
            locs.iter()
                .map(|l| l.to_string())
                .collect::<Vec<String>>()
                .join(",")
        ));
    }
    let o = pages.join(" ");
    let _ = Box::into_raw(search_index);

    #[cfg(debug_assertions)]
    debug_log(&format! {"{:?}", o});

    o
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}