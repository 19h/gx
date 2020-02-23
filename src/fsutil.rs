use std::{fs, mem};
use std::ffi::OsString;
use std::io::{Read, BufReader};
use std::collections::HashMap;

use chrono::NaiveDate;
use lazy_static::lazy_static;
use regex::Regex;

use crate::{MediaFolders, MediaMap, MediaItem, MediaItems};

pub fn scan_dir(path: &str) -> Option<Vec<(OsString, fs::Metadata)>> {
    fs::read_dir(path)
        .map(|dir| {
            let entries = dir.filter_map(|item|
                item.map(|entry|
                    entry
                        .metadata()
                        .map(|entry_meta|
                            Some((entry.file_name(), entry_meta))
                        )
                        .unwrap_or(
                            None,
                        )
                )
                    .unwrap_or(
                        None,
                    )
            )
                .collect::<Vec<_>>();

            Some(entries)
        })
        .unwrap_or(None)
}

pub fn is_media_folder(name: &str) -> bool {
    lazy_static! {
        static ref matcher: Regex = Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap();
    }

    matcher.is_match(name)
}

pub fn is_image_file(name: &str) -> bool {
    lazy_static! {
        static ref matcher: Regex = Regex::new(r"\.(?:jpe?g|png|gif|bmp)$").unwrap();
    }

    matcher.is_match(&name.to_lowercase())
}

fn find_media_folders(
    data_path: &String,
    media_folders: &mut MediaFolders,
) {
    if let Some(files) = scan_dir(data_path) {
        for (name, meta) in files.iter() {
            let name = name.to_str().unwrap();

            if !meta.is_dir() || !is_media_folder(name) {
                continue;
            }

            let parsed_date =
                NaiveDate::parse_from_str(name, "%Y-%m-%d");

            if parsed_date.is_err() {
                continue;
            }

            media_folders
                .push(
                    (
                        format!("{}{}", data_path, name),
                        parsed_date.unwrap(),
                    ),
                );
        }

        media_folders
            .sort_by(|a, b|
                b.1.cmp(&a.1)
            );
    }
}

pub fn discover_media_folders(
    data_path: &String,
    mut media_folders: &mut MediaFolders,
    media_items: &mut MediaItems,
    media_map: &mut MediaMap,
) {
    find_media_folders(
        data_path,
        &mut media_folders,
    );

    for (folder_path, _) in media_folders.iter() {
        if let Some(subfolders) = scan_dir(&folder_path) {
            let mut subfolder_items: Vec<String> = Vec::new();

            for (name, meta) in subfolders {
                let name = name.to_str().unwrap();

                if !is_image_file(name) {
                    continue;
                }

                let file_path = format!("{}/{}", &folder_path, name);

                let file = fs::File::open(
                    &file_path,
                );

                if file.is_err() {
                    continue;
                }

                let mut file = file.unwrap();
                let mut start_buf = vec!(0u8; 16);
                file.read(&mut start_buf);

                if let Err(_) = image::guess_format(&start_buf) {
                    // unknown image format
                    continue;
                }

                subfolder_items.push(
                    file_path
                        .clone()
                        .into(),
                );

                media_items.insert(
                    file_path.into(),
                    (
                        meta.clone(),
                        None,
                    ),
                );
            };

            if subfolder_items.len() != 0 {
                media_map
                    .insert(
                        folder_path.clone(),
                        subfolder_items,
                    );
            }
        }
    }
}

pub fn exif_enrich_media_items(
    media_items: &mut MediaItems,
) {
    for (image_path, item) in media_items.iter_mut() {
        let file = std::fs::File::open(
            image_path,
        ).unwrap();

        let exif = exif::Reader::new()
            .read_from_container(
                &mut BufReader::new(&file),
            )
            .unwrap();

        // insert exif into image meta
        mem::replace(
            &mut item.1,
            Some(
                exif.fields()
                    .cloned()
                    .collect::<Vec<_>>(),
            ),
        );
    }
}
