use std::path::PathBuf;

use image_of_images::{MakeImgOfImsOpts, find_free_filepath};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Opt {
    #[structopt(long)]
    input_dir: PathBuf,
    #[structopt(long)]
    target_img: PathBuf,
    #[structopt(long)]
    output_dir: PathBuf,
    #[structopt(long, default_value = "1000")]
    target_width: u32,
    #[structopt(long, default_value = "40")]
    num_horizontal_imgs: u32,
    #[structopt(long, default_value = "40")]
    num_vertical_imgs: u32,
    #[structopt(long)]
    max_imgs: Option<usize>,
    #[structopt(long)]
    no_pop: bool,
}

fn main() -> anyhow::Result<()> {
    let _ = dotenv::dotenv();
    env_logger::init();

    let opt = Opt::from_args();

    std::fs::create_dir_all(&opt.output_dir)?;

    let output_file = find_free_filepath(opt.output_dir, "result", ".png");

    image_of_images::make_img_of_images(opt.target_img, opt.input_dir, output_file, MakeImgOfImsOpts{
        target_width: opt.target_width,
        num_horizontal_imgs: opt.num_horizontal_imgs,
        num_vertical_imgs: opt.num_vertical_imgs,
        max_imgs: opt.max_imgs,
        no_pop: opt.no_pop,
        ..Default::default()
    })?;
    
    Ok(())
}
