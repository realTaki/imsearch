use std::thread;
use std::time::Duration;

use super::database::{ImageColumnFamily, MetaData};
use crate::config::ConfDir;
use crate::db::utils::init_column_family;
use anyhow::Result;
use rocksdb::{IteratorMode, Options, DB};

/// check whether the database needs update
pub fn check_db_update(path: &ConfDir) -> Result<()> {
    let version_file = path.version();

    // v1, v2 => v3
    if !version_file.exists() {
        println!("START UPGRADING (2 -> 3) AFTER 10 SECS!!!");
        thread::sleep(Duration::from_secs(10));
        update_from_2_to_3(path)?;
    }

    Ok(())
}

fn update_from_2_to_3(path: &ConfDir) -> Result<()> {
    let opts = Options::default();
    let image_db = DB::open_for_read_only(&opts, path.path().join("image.db"), true)?;
    let features_db = DB::open_for_read_only(&opts, path.path().join("features.db"), true)?;

    let new_db = DB::open_default(path.database())?;

    init_column_family(&new_db)?;

    let new_feature = new_db
        .cf_handle(ImageColumnFamily::NewFeature.as_ref())
        .unwrap();
    let index_image = new_db
        .cf_handle(ImageColumnFamily::IdToImage.as_ref())
        .unwrap();
    let image_list = new_db
        .cf_handle(ImageColumnFamily::ImageList.as_ref())
        .unwrap();
    let meta_data = new_db
        .cf_handle(ImageColumnFamily::MetaData.as_ref())
        .unwrap();

    let mut total_features = 0u64;
    for (idx, data) in features_db.iterator(IteratorMode::Start).enumerate() {
        print!("\r{}", idx);
        let idx = idx.to_le_bytes();
        let feature = data.0;
        let image_id = data.1;
        let image_path = image_db.get(image_id)?.unwrap();

        new_db.put_cf(&new_feature, idx, feature)?;
        new_db.put_cf(&index_image, idx, &image_path)?;
        new_db.put_cf(&image_list, image_path, [])?;

        total_features += 1;
    }
    let total_images = image_db.iterator(IteratorMode::Start).count() as u64;

    new_db.put_cf(
        &meta_data,
        MetaData::TotalFeatures,
        total_features.to_le_bytes(),
    )?;
    new_db.put_cf(
        &meta_data,
        MetaData::TotalImages,
        total_images.to_le_bytes(),
    )?;

    std::fs::write(path.version(), "3")?;

    Ok(())
}