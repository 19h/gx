use std::{fs, cmp};
use image::GenericImageView;

pub fn resize_if_needed(
    cache_path: String,
    image_path: String,
    thubm_width: u32,
    thubm_height: u32,
) -> Option<String> {
    let cleaned_name =
        image_path
            .clone()
            .replace("/", "_")
            .replace(".", "-");

    let cache_file_path = format!(
        "{}/{}_w{}_h{}.jpg",
        &cache_path,
        cleaned_name,
        thubm_width,
        thubm_height,
    );

    if fs::metadata(&cache_file_path).is_ok() {
        return Some(cache_file_path.clone());
    }

    fs::create_dir_all(
        &cache_path,
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
            ::create(&cache_file_path)
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
        .map(|_| Some(cache_file_path.clone()))
        .unwrap_or(None)
}
