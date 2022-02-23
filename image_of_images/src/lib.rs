use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use crossbeam::channel::SendError;
use image::{
    buffer::ConvertBuffer, io::Reader as ImageReader, DynamicImage, GenericImageView, ImageBuffer,
    Rgb, Rgba,
};
use itertools::Itertools;
use rand::prelude::*;
use rayon::{
    iter::{
        IndexedParallelIterator, IntoParallelIterator, IntoParallelRefIterator, ParallelIterator,
    },
    slice::ParallelSliceMut,
};

type Image = ImageBuffer<Rgb<f32>, Vec<f32>>;
pub type ProgressSender = crossbeam::channel::Sender<(usize, usize, &'static str)>;
pub type ProgressReceiver = crossbeam::channel::Receiver<(usize, usize, &'static str)>;

pub const IMAGE_EXTENSIONS: [&str; 3] = ["png", "jpg", "jpeg"];

pub fn progress_channel() -> (ProgressSender, ProgressReceiver) {
    crossbeam::channel::unbounded()
}

pub fn find_free_filepath(dir: impl AsRef<Path>, base: &str, extension: &str) -> PathBuf {
    let dir = dir.as_ref();
    let mut result = dir.join(format!("{base}{extension}"));
    let mut i: usize = 1;

    while result.exists() {
        result = dir.join(format!("{base}({i}){extension}"));
        i += 1;
    }

    result
}

fn handle_progress_send_error(e: Result<(), SendError<(usize, usize, &'static str)>>) {
    if let Err(e) = e {
        log::warn!("Failed sending progress: {:?}", e);
    }
}

fn resize_img(img: Image, width: u32, height: u32) -> Image {
    let width = width as f32;
    let height = height as f32;
    let im_w = img.width() as f32;
    let im_h = img.height() as f32;

    let width_scale = width / im_w;
    let height_scale = height / im_h;

    let scale = width_scale.max(height_scale);

    let mut img = image::imageops::resize(
        &img,
        (im_w * scale).ceil() as u32,
        (im_h * scale).ceil() as u32,
        image::imageops::FilterType::Triangle,
    );

    let (im_w, im_h) = img.dimensions();

    let width = width as u32;
    let height = height as u32;
    let left = (im_w - width) / 2;
    let top = (im_h - height) / 2;

    // dbg!(im_w); dbg!(im_h); dbg!(width); dbg!(height); dbg!(left); dbg!(top);

    let r = image::imageops::crop(&mut img, left, top, width, height).to_image();
    assert_eq!(r.dimensions(), (width, height));
    r.convert()
}

fn load_imgs_from_dir(
    dir: impl AsRef<Path>,
    width: u32,
    height: u32,
    n_max_imgs: Option<usize>,
    progress_sender: &Option<ProgressSender>,
) -> anyhow::Result<Vec<Image>> {
    let mut result = Vec::new();
    let dir = dir.as_ref();

    let dir = dir
        .to_str()
        .ok_or(anyhow::anyhow!("Failed converting dir to str"))?
        .to_string();

    let mut all_imgs = Vec::new();

    for ext in IMAGE_EXTENSIONS {
        let glob_pattern = format!("{}/**/*.{}", &dir, ext);
        all_imgs.extend(glob::glob(&glob_pattern)?)
    }

    if let Some(n) = n_max_imgs {
        all_imgs.shuffle(&mut rand::thread_rng());
        all_imgs = all_imgs.into_iter().take(n).collect();
    }

    let n_imgs = all_imgs.len();

    for (i, entry) in all_imgs.into_iter().enumerate() {
        if let Some(s) = progress_sender {
            let e = s.send((i, n_imgs, "Loading images from disk"));
            handle_progress_send_error(e);
        }
        match entry {
            Ok(path) => {
                if let Some(img) = ImageReader::open(&path).ok().and_then(|r| r.decode().ok()) {
                    let img = resize_img(img.into_rgb32f(), width, height);
                    result.push(img);
                } else {
                    log::warn!("Failed loading image: {path:?}");
                }
            }
            Err(_) => (),
        }
    }

    Ok(result)
}

fn load_and_resize_target_img(
    target_img_path: impl AsRef<Path>,
    width: u32,
) -> anyhow::Result<Image> {
    let img = ImageReader::open(target_img_path)?.decode()?;

    let im_width = img.width() as f32;
    let im_height = img.height() as f32;
    let scale = width as f32 / im_width;

    let img = image::imageops::resize(
        &img,
        (im_width * scale) as u32,
        (im_height * scale) as u32,
        image::imageops::FilterType::Triangle,
    );

    Ok(img.convert())
}

fn squared_error(target_img: &Image, fill_img: &Image, x_start: u32, y_start: u32) -> f32 {
    // let mut sum = 0.0;

    let (w, h) = fill_img.dimensions();

    let patch = target_img.view(x_start, y_start, w, h);

    let sum = patch
        .pixels()
        .zip(fill_img.pixels())
        .map(|(p1, &p2)| {
            p1.2 .0
                .into_iter()
                .zip(p2.0.into_iter())
                .map(|(v1, v2)| (v1 - v2).powi(2))
                .sum::<f32>()
        })
        .sum::<f32>();

    sum / (w as f32 * h as f32)
}

fn insert_sub_img(target_img: &mut Image, fill_img: &Image, x_start: u32, y_start: u32) {
    let (w, h) = fill_img.dimensions();

    for i in 0..w {
        for j in 0..h {
            let x = x_start + i;
            let y = y_start + j;

            target_img.put_pixel(x, y, fill_img.get_pixel(i, j).clone())
        }
    }
}

fn insert_sub_imgs(
    target_img: &mut Image,
    sub_imgs: &[Vec<&Image>],
    progress_sender: &Option<ProgressSender>,
) {
    assert!(sub_imgs.len() > 0);
    assert!(sub_imgs[0].len() > 0);

    let mut x_from = 0;
    let mut y_from = 0;

    let (w, h) = sub_imgs[0][0].dimensions();
    let total = sub_imgs.iter().map(Vec::len).sum::<usize>();
    let mut cur = 0;

    for sub_img_row in sub_imgs {
        for &sub_img in sub_img_row {
            if let Some(s) = progress_sender {
                let e = s.send((cur, total, "Inserting images in target"));
                handle_progress_send_error(e);
                cur += 1;
            }
            assert!((w, h) == sub_img.dimensions());

            // dbg!(x_from);
            // dbg!(y_from);

            insert_sub_img(target_img, sub_img, x_from, y_from);

            x_from += w;
        }
        y_from += h;
        x_from = 0;
    }
}

fn empty_vec_2d<T>(rows: usize, cols: usize) -> Vec<Vec<Option<T>>> {
    (0..rows)
        .map(|_| (0..cols).map(|_| None).collect())
        .collect()
}

fn float_err_to_usize(f: f32, max_err: f32) -> usize {
    assert!(f >= 0.0);

    let f = f / max_err;
    let f = f * (usize::MAX as f32);

    return f as usize;
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ErrInfo {
    img_idx: usize,
    i_pos: u32,
    j_pos: u32,
    pos_err_min: usize,
    err: usize,
}

impl PartialOrd for ErrInfo {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        other
            .pos_err_min
            .partial_cmp(&self.pos_err_min)
            .and_then(|order| Some(order.then(self.err.partial_cmp(&other.err)?)))
    }
}

fn calc_errors(
    target_img: &Image,
    imgs: &[&Image],
    n_width: u32,
    n_height: u32,
    sub_img_width: u32,
    sub_img_height: u32,
    progress_sender: &Option<ProgressSender>,
) -> Vec<ErrInfo> {
    let mut result = Vec::new();

    let total = n_width * n_height;

    for i in (0..n_height) {
        for j in 0..n_width {
            let y_from = i * sub_img_height;
            let x_from = j * sub_img_width;

            if let Some(s) = progress_sender {
                let e = s.send((
                    (i * n_width + j) as usize,
                    total as usize,
                    "calculating errors",
                ));
                handle_progress_send_error(e)
            }

            // dbg!(n_height); dbg!(n_width); dbg!(x_from); dbg!(y_from); dbg!(sub_img_width); dbg!(sub_img_height);

            let errors: Vec<_> = imgs
                .par_iter()
                // .iter()
                .map(|im| squared_error(target_img, im, x_from, y_from))
                .enumerate()
                .map(|(idx, err)| (idx, i, j, err))
                .collect();

            let pos_err_min = errors
                .iter()
                .max_by(|&(_, _, _, err1), &(_, _, _, err2)| err1.partial_cmp(err2).unwrap())
                .unwrap()
                .3;

            result.extend(
                errors
                    .into_iter()
                    .map(|(img_idx, i_pos, j_pos, err)| (img_idx, i_pos, j_pos, pos_err_min, err)),
            );
        }
    }

    let max_err = result
        .iter()
        .map(|r| r.4)
        .max_by(|v1, v2| v1.partial_cmp(v2).unwrap())
        .unwrap();

    result
        .into_iter()
        .map(|(img_idx, i_pos, j_pos, pos_err_min, err)| ErrInfo {
            img_idx,
            i_pos,
            j_pos,
            pos_err_min: float_err_to_usize(pos_err_min, max_err),
            err: float_err_to_usize(err, max_err),
        })
        .collect()
}


fn fill_target_img(
    mut target_img: Image,
    imgs: &[&Image],
    sub_img_width: u32,
    sub_img_height: u32,
    pop_used_img: bool,
    progress_sender: &Option<ProgressSender>,
) -> anyhow::Result<Image> {
    let n_width = target_img.width() / sub_img_width;
    let n_height = target_img.height() / sub_img_height;

    if imgs.len() < (n_width * n_height) as usize {
        return Err(anyhow::anyhow!("Too few images in directory, try reducing the number of horizontal and/or vertical images"));
    }

    let pad_width = target_img.width() % sub_img_width;
    let pad_height = target_img.height() % sub_img_height;

    let cropped_width = target_img.width() - pad_width;
    let cropped_height = target_img.height() - pad_height;

    let target_img = image::imageops::crop(
        &mut target_img,
        pad_width / 2,
        pad_height / 2,
        cropped_width,
        cropped_height,
    )
    .to_image();

    let mut result_img = target_img.clone();
    let mut sub_imgs: Vec<Vec<_>> = empty_vec_2d(n_height as usize, n_width as usize);

    let mut errors = calc_errors(
        &target_img,
        imgs,
        n_width,
        n_height,
        sub_img_width,
        sub_img_height,
        &progress_sender,
    );

    // reverse sort
    errors.par_sort_by(|e1, e2| e2.err.partial_cmp(&e1.err).unwrap());
    let n_images = (n_width * n_height) as usize;
    let mut filled_imgs = 0;


    let mut black_list = HashSet::new();
    while let Some(ErrInfo {
        img_idx,
        i_pos,
        j_pos,
        ..
    }) = errors.pop()
    
    {
        let i = i_pos as usize;
        let j = j_pos as usize;

        if black_list.contains(&img_idx) || sub_imgs[i][j].is_some() {
            continue;
        }

        sub_imgs[i][j] = Some(imgs[img_idx]);
        black_list.insert(img_idx);
        filled_imgs += 1;

        if let Some(s) = progress_sender {
            let r = s.send((filled_imgs, n_images, "Selecting images for result"));
            handle_progress_send_error(r);
        }

        if filled_imgs >= n_images {
            break
        }

    }


    let sub_imgs: Vec<Vec<_>> = sub_imgs.into_iter().map(|v| v.into_iter().map(Option::unwrap).collect() ).collect();
    insert_sub_imgs(&mut result_img, &sub_imgs, progress_sender);

    Ok(result_img)
}

#[derive(Debug, Clone)]
pub struct MakeImgOfImsOpts {
    pub target_width: u32,
    pub num_horizontal_imgs: u32,
    pub num_vertical_imgs: u32,
    pub max_imgs: Option<usize>,
    pub no_pop: bool,
    pub progress_sender: Option<ProgressSender>,
}

impl Default for MakeImgOfImsOpts {
    fn default() -> Self {
        Self {
            target_width: 1000,
            num_horizontal_imgs: 40,
            num_vertical_imgs: 40,
            max_imgs: None,
            no_pop: false,
            progress_sender: None,
        }
    }
}

pub fn make_img_of_images(
    target_im_path: impl AsRef<Path>,
    input_dir: impl AsRef<Path>,
    output_file: impl AsRef<Path>,
    opts: MakeImgOfImsOpts,
) -> anyhow::Result<()> {
    let output_file = output_file.as_ref();
    let target_img = load_and_resize_target_img(target_im_path, opts.target_width)?;

    // let tgt_img_conf: ImageBuffer<Rgba<u16>, Vec<u16>> = target_img.convert();
    // tgt_img_conf.save(opt.output_dir.join("preprocessed_target_image.png"))?;

    let img_width = target_img.width() / opts.num_horizontal_imgs;
    let img_height = target_img.height() / opts.num_vertical_imgs;

    let imgs = load_imgs_from_dir(
        input_dir,
        img_width,
        img_height,
        opts.max_imgs,
        &opts.progress_sender,
    )?;

    let mut img_refs: Vec<&Image> = imgs.iter().collect();
    let result = fill_target_img(
        target_img,
        &mut img_refs,
        img_width,
        img_height,
        !opts.no_pop,
        &opts.progress_sender,
    )?;

    let result: ImageBuffer<Rgba<u16>, Vec<u16>> = result.convert();
    result.save(output_file)?;

    Ok(())
}
