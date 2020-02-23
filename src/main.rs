use actix_web::{get, web, App, HttpServer, Responder, HttpRequest, HttpResponse};
use std::sync::{Mutex, Arc};

use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::{fs, mem, io, cmp};
use std::ffi::OsString;

use chrono::NaiveDate;
use image;
use regex::Regex;
use std::io::{Read, BufReader, Error};
use exif;
use askama::Template;

mod fsutil;
use fsutil::discover_media_folders;
use crate::fsutil::exif_enrich_media_items;
use image::{DynamicImage, GenericImageView};
use actix_files::NamedFile;

type MediaFolders = Vec<(String, NaiveDate)>;
type MediaItem = (fs::Metadata, Option<Vec<exif::Field>>);
type MediaItems = HashMap<String, MediaItem>;
type MediaMap = HashMap<String, Vec<String>>;

#[derive(Template)]
#[template(path = "index.html")]
struct RootTemplate<'a> {
    folders: &'a Vec<(String, String)>,
}

#[derive(Template)]
#[template(path = "list.html")]
struct FolderTemplate<'a> {
    folder: &'a Vec<(String, String)>,
}

pub struct GxData {
    data_path: String,
    cache_path: String,

    media_folders: MediaFolders,
    media_items: MediaItems,
    media_map: MediaMap,
}

impl GxData {
    fn new(
        data_path: impl Into<String>,
        cache_path: impl Into<String>
    ) -> GxData {
        let mut gxd = GxData {
            data_path: data_path.into(),
            cache_path: cache_path.into(),

            media_folders: Vec::new(),
            media_items: HashMap::new(),
            media_map: HashMap::new(),
        };

        gxd.init();

        gxd
    }

    fn init(&mut self) {
        discover_media_folders(
            &self.data_path,
            &mut self.media_folders,
            &mut self.media_items,
            &mut self.media_map,
        );

        exif_enrich_media_items(
            &mut self.media_items,
        );
    }

    fn build_list_data(
        &self
    ) -> Vec<(String, String)> {
        self.media_folders
            .iter()
            .cloned()
            .filter_map(|(folder, _)|
                self.media_map
                    .get(&folder)
                    .map(|item|
                        item.first()
                    )
                    .unwrap_or(None)
                    .map(|item|
                        (folder, item.clone()),
                    )
            )
            .collect()
    }

    fn build_folder_data(
        &self,
        folder: &str,
    ) -> Option<Vec<(String, String)>> {
        self.media_map
            .get(folder)
            .map(|items|
                items
                    .iter()
                    .map(|item|
                        (
                            item.clone(),
                            item.clone(), // TODO fx
                        ),
                    )
                    .collect::<Vec<_>>()
            )
    }

    fn resize_if_needed(
        &self,
        image_path: String,
        thubm_width: u32,
        thubm_height: u32,
    ) -> Option<String> {
        let cleaned_name =
            image_path
                .clone()
                .replace("/", "_")
                .replace(".", "-");

        if self.media_items.get(&image_path).is_none() {
            return None;
        }

        let cache_path = format!(
            "{}/{}_w{}_h{}.jpg",
            self.cache_path,
            cleaned_name,
            thubm_width,
            thubm_height,
        );

        if fs::metadata(&cache_path).is_ok() {
            return Some(cache_path.clone());
        }

        fs::create_dir_all(
            &self.cache_path,
        );

        image::open(image_path)
            .map(|mut image| {
                let width = image.width();
                let height = image.height();

                let min_dim = cmp::min(width, height);
                let max_dim = cmp::max(width, height);

                image
                    .crop(
                        (max_dim - min_dim) / 2,
                        0,
                        min_dim,
                        min_dim,
                    )
                    .thumbnail(
                        thubm_width,
                        thubm_height,
                    )
            })
            .map(|image| {
                fs::File
                ::create(&cache_path)
                    .map(|mut output|
                        Some(
                            image.write_to(
                                &mut output,
                                image::ImageFormat::Jpeg,
                            ),
                        )
                    )
                    .unwrap_or(None)
            })
            .map(|_| Some(cache_path.clone()))
            .unwrap_or(None)
    }
}

async fn index(
    info: web::Path<()>,
    db: web::Data<Arc<Mutex<GxData>>>,
    req: HttpRequest,
) -> impl Responder {
    let db = &mut *db.lock().unwrap();

    HttpResponse::Ok()
        .content_type("text/html")
        .body(
            (RootTemplate {
                folders: &db.build_list_data(),
            }).render().unwrap(),
        )
}

async fn thumb(
    db: web::Data<Arc<Mutex<GxData>>>,
    req: HttpRequest,
) -> actix_web::Result<NamedFile> {
    let db = &mut *db.lock().unwrap();

    let image =
        req.match_info()
            .query("filename");

    let thumb =
        db.resize_if_needed(
            image.into(),
            200,
            200,
        );

    if thumb.is_none() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "").into());
    }

    let path: PathBuf = thumb.unwrap().parse().unwrap();

    Ok(NamedFile::open(path)?)
}

async fn listing(
    db: web::Data<Arc<Mutex<GxData>>>,
    req: HttpRequest,
) -> impl Responder {
    let db = &mut *db.lock().unwrap();

    let folder_name =
        req.match_info()
            .query("filename");

    match db.build_folder_data(folder_name) {
        Some(data) =>
            HttpResponse::Ok()
                .content_type("text/html")
                .body(
                    (FolderTemplate {
                        folder: &data,
                    }).render().unwrap(),
                ),
        _ => HttpResponse::NotFound().await.unwrap(),
    }
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    let db = Arc::new(
        Mutex::new(
            GxData::new(
                "images/",
                "cache/",
            ),
        ),
    );

    HttpServer::new(move || {
        App::new()
            .data(db.clone())
            .service(
                actix_files
                ::Files
                ::new("/static", "static/")
                    .use_etag(true)
                    .use_last_modified(true),
            )
            .service(
                actix_files
                ::Files
                ::new("/images", "images/")
                    .use_etag(true)
                    .use_last_modified(true),
            )
            .service(
                web
                ::resource("/thumb/{filename:.*}")
                    .to(thumb)
            )
            .service(
                web
                ::resource("/f/{filename:.*}")
                    .to(listing)
            )
            .service(
                web
                ::resource("/")
                    .to(index)
            )
    })
        .workers(4)
        .bind("127.0.0.1:8080")?
        .run()
        .await
}
