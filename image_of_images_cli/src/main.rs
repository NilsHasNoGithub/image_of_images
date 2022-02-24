use std::{path::PathBuf, thread};

use image_of_images::{find_free_filepath, MakeImgOfImsOpts, ProgressReceiver, progress_channel};
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

fn start_print_progress_thread(progress_receiver: ProgressReceiver) {
    thread::spawn(move || {
        let term = console::Term::stdout();
        let _ = term.write_line("");
        while let Ok((part, total, desc)) = progress_receiver.recv() {
            let _ = term.clear_last_lines(1);
            let _ = term.write_line(&format!("{desc} ({part}/{total})"));
        }
    });
}

fn main() -> anyhow::Result<()> {
    let _ = dotenv::dotenv();
    env_logger::init();

    let opt = Opt::from_args();

    std::fs::create_dir_all(&opt.output_dir)?;

    let output_file = find_free_filepath(opt.output_dir, "result", ".png");

    let (progress_sender, progress_receiver) = progress_channel();

    start_print_progress_thread(progress_receiver);

    image_of_images::make_img_of_images(
        opt.target_img,
        opt.input_dir,
        output_file,
        MakeImgOfImsOpts {
            target_width: opt.target_width,
            num_horizontal_imgs: opt.num_horizontal_imgs,
            num_vertical_imgs: opt.num_vertical_imgs,
            max_imgs: opt.max_imgs,
            no_pop: opt.no_pop,
            progress_sender: Some(progress_sender),
            ..Default::default()
        },
    )?;

    Ok(())
}
