use std::{fs, mem};
use std::ffi::OsString;
use std::io::{Read, BufReader};
use std::collections::HashMap;

use chrono::NaiveDate;
use lazy_static::lazy_static;
use regex::Regex;

use crate::{MediaFolders, MediaMap, MediaItem, MediaItems};

pub fn scan_dir(path: &str) -> Option<Vec<(OsString, fs::Metadata)>> {
    fs
    ::read_dir(path)
    .map(|dir| {
        Some(
            dir
            .filter_map(|item|
                item
                .map(|entry|
                    entry
                    .metadata()
                    .map(|entry_meta|
                        Some((entry.file_name(), entry_meta))
                    )
                    .unwrap_or(None)
                )
                .unwrap_or(None)
            )
            .collect::<Vec<_>>(),
        )
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
    scan_dir(data_path)
    .map(|files| {
        files
        .iter()
        .for_each(|(name, meta)| {
            let name = name.to_str().unwrap();

            if !meta.is_dir() || !is_media_folder(name) {
                return;
            }

            NaiveDate
            ::parse_from_str(name, "%Y-%m-%d")
            .map(|parsed_date|
                 media_folders
                 .push(
                     (
                         format!("{}{}", data_path, name),
                         parsed_date,
                     ),
                 ),
            );
        });
    });

    media_folders
    .sort_by(|a, b|
        b.1.cmp(&a.1)
    );
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

    media_folders
        .iter()
        .for_each(|(folder_path, _)| {
            scan_dir(&folder_path)
            .map(|subfolders| {
                let subfolder_items: Vec<String> =
                    subfolders
                    .iter()
                    .filter_map(|(name, meta)| {
                        let name = name.to_str().unwrap();

                        if !is_image_file(name) {
                            return None;
                        }

                        let file_path = format!("{}/{}", &folder_path, name);

                        let file = fs::File::open(
                            &file_path,
                        );

                        if file.is_err() {
                            return None;
                        }

                        let mut file = file.unwrap();
                        let mut start_buf = vec!(0u8; 16);
                        file.read(&mut start_buf);

                        if let Err(_) = image::guess_format(&start_buf) {
                            // unknown image format
                            return None;
                        }

                        media_items.insert(
                            file_path.clone(),
                            ( meta.clone(), None ),
                        );

                        Some(file_path.clone())
                    })
                    .collect();

                if subfolder_items.len() != 0 {
                    media_map
                        .insert(
                            folder_path.clone(),
                            subfolder_items,
                        );
                }
            });
        });
}

pub fn exif_enrich_media_items(
    media_items: &mut MediaItems,
) {
    media_items
    .iter_mut()
    .for_each(|(mut image_path, mut item)| {
        std::fs::File
        ::open(image_path)
        .map(|file| {
            exif::Reader
            ::new()
            .read_from_container(
                &mut BufReader::new(&file),
            )
            .map(|exif|
                // insert exif into image meta
                mem::replace(
                    &mut item.1,
                    Some(
                        exif.fields()
                            .cloned()
                            .collect::<Vec<_>>(),
                    ),
                ),
            );
        });
    });
}

pub fn reorder_media_folders(
    mut media_folders: &mut MediaFolders,
    media_items: &mut MediaItems,
    media_map: &mut MediaMap,
) {
    media_map
        .iter()
        .for_each(|(folder_path, paths)|
            paths
                .iter()
                .for_each(|path| {
                    media_items
                        .get(path)
                        .map(|item|
                            &item.1
                        )
                        .map(|item_exif|
                            println!("{:?}", item_exif)
                        );
                })
        );
}
