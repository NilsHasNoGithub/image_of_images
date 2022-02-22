use std::{thread, time::Duration, path::{PathBuf, Path}};

use crossbeam::channel::{Receiver, Sender};
use eframe::{epi::{App, Storage}, NativeOptions};
use egui::{Response, TextBuffer};
use image_of_images::{
    make_img_of_images, progress_channel, ProgressReceiver, ProgressSender, IMAGE_EXTENSIONS,
};

#[derive(Debug, Clone, Copy)]
enum FileDialogType {
    TargetImgPath,
    InputFolderPath,
    OutputFolderPath,
}

enum NumInputType {
    NumHorizontalImgs,
    NumVerticalImgs,
    TargetImgWidth,
}

impl FileDialogType {
    fn make_result_event(self, result: String) -> Event {
        match self {
            FileDialogType::TargetImgPath => Event::SetTargetImgPath(result),
            FileDialogType::InputFolderPath => Event::SetInputFolderPath(result),
            FileDialogType::OutputFolderPath => Event::SetOutputFolderPath(result),
        }
    }
}

#[derive(Debug)]
enum Event {
    SetTargetImgPath(String),
    SetInputFolderPath(String),
    SetOutputFolderPath(String),
    SetProgressText(Option<String>),
    ProcessFinished,
}

#[derive(Debug, Clone)]
struct ImgOfImgsGui {
    target_img_path: String,
    input_folder_path: String,
    output_folder_path: String,
    num_horizontal_imgs: String,
    num_vertical_imgs: String,
    target_img_width: String,
    processing: bool,
    event_receiver: Receiver<Event>,
    event_sender: Sender<Event>,
    progress_text: Option<String>,
    progress_receiver: ProgressReceiver,
    progress_sender: ProgressSender,
}

impl ImgOfImgsGui {
    fn show_select_dialog(&self, dialog_type: FileDialogType) {
        let result_sender = self.event_sender.clone();

        thread::spawn(move || {
            let opt_path = match dialog_type {
                FileDialogType::TargetImgPath => native_dialog::FileDialog::new()
                    .add_filter("Image", &IMAGE_EXTENSIONS)
                    .set_location(".")
                    .show_open_single_file(),
                FileDialogType::InputFolderPath | FileDialogType::OutputFolderPath => {
                    native_dialog::FileDialog::new()
                        .set_location(".")
                        .show_open_single_dir()
                }
            };

            match opt_path {
                Ok(Some(path)) => {
                    if let Some(s) = path.to_str() {
                        result_sender
                            .send(dialog_type.make_result_event(s.into()))
                            .unwrap();
                    }
                }
                Err(e) => {
                    log::warn!("Error using file dialog: {:?}", e)
                }
                _ => (),
            }
        });
    }

    fn add_path_input(&mut self, ui: &mut egui::Ui, dialog_type: FileDialogType) {
        ui.label(match dialog_type {
            FileDialogType::TargetImgPath => "Target image",
            FileDialogType::InputFolderPath => "Input folder",
            FileDialogType::OutputFolderPath => "Output folder",
        });

        ui.horizontal(|ui| {
            text_input_long(
                ui,
                match dialog_type {
                    FileDialogType::TargetImgPath => &mut self.target_img_path,
                    FileDialogType::InputFolderPath => &mut self.input_folder_path,
                    FileDialogType::OutputFolderPath => &mut self.output_folder_path,
                },
            );

            if ui.button("Select").clicked() {
                self.show_select_dialog(dialog_type)
            }
        });

        ui.end_row();
    }

    fn handle_events(&mut self) {
        while let Ok((part, total, desc)) = self.progress_receiver.try_recv() {
            self.progress_text = Some(format!("Processing: {} ({}/{})", desc, part, total));
        }

        while let Ok(event) = self.event_receiver.try_recv() {
            match event {
                Event::SetTargetImgPath(s) => self.target_img_path = s,
                Event::SetInputFolderPath(s) => self.input_folder_path = s,
                Event::SetOutputFolderPath(s) => self.output_folder_path = s,
                Event::SetProgressText(s) => self.progress_text = s,
                Event::ProcessFinished => self.processing = false,
            }
        }
    }

    fn output_file(&self) -> PathBuf {
        Path::new(&self.output_folder_path).join("result.png")
    }

    fn start_make_img_of_imgs(&self) -> anyhow::Result<()> {
        let progress_sender = self.progress_sender.clone();
        let event_sender = self.event_sender.clone();

        let target_img_path = self.target_img_path.clone();
        let input_folder_path = self.input_folder_path.clone();
        let output_folder_path = self.output_folder_path.clone();

        let num_horizontal_imgs = self.num_horizontal_imgs.parse()?;
        let num_vertical_imgs = self.num_vertical_imgs.parse()?;
        let target_width = self.target_img_width.parse()?;

        let out_file = self.output_file();

        thread::spawn(move || {
            let result = make_img_of_images(
                target_img_path,
                input_folder_path,
                &output_folder_path,
                image_of_images::MakeImgOfImsOpts {
                    progress_sender: Some(progress_sender),
                    num_horizontal_imgs,
                    num_vertical_imgs,
                    target_width,
                    ..Default::default()
                },
            );

            match result {
                Ok(_) => event_sender.send(Event::SetProgressText(Some(format!(
                    "Finished creating image, which can be found here '{}'",
                    out_file.to_str().unwrap_or_default(),
                )))),
                Err(e) => event_sender.send(Event::SetProgressText(Some(format!(
                    "Failed creating image of images: {}",
                    e
                )))),
            }
            .unwrap();

            event_sender.send(Event::ProcessFinished).unwrap();
        });

        Ok(())
    }

    fn add_number_input(&mut self, ui: &mut egui::Ui, num_type: NumInputType) {
        ui.label(match num_type {
            NumInputType::NumHorizontalImgs => "Amount of horizontal images",
            NumInputType::NumVerticalImgs => "Amount of vertical images",
            NumInputType::TargetImgWidth => "Target image width",
        });

        let field = match num_type {
            NumInputType::NumHorizontalImgs => &mut self.num_horizontal_imgs,
            NumInputType::NumVerticalImgs => &mut self.num_vertical_imgs,
            NumInputType::TargetImgWidth => &mut self.target_img_width,
        };

        if ui.text_edit_singleline(field).changed() {
            (*field) = field.chars().filter(|c| c.is_numeric()).collect();

            if let Err(_) = field.parse::<u32>() {
                field.drain(..);
            }
        }

        ui.end_row();
    }
}

impl Default for ImgOfImgsGui {
    fn default() -> Self {
        let (event_sender, event_receiver) = crossbeam::channel::unbounded();
        let (progress_sender, progress_receiver) = progress_channel();

        Self {
            target_img_path: Default::default(),
            input_folder_path: Default::default(),
            output_folder_path: "results".into(),
            processing: false,
            event_receiver,
            event_sender,
            progress_text: Default::default(),
            progress_receiver,
            progress_sender,
            num_horizontal_imgs: 40.to_string(),
            num_vertical_imgs: 40.to_string(),
            target_img_width: 1000.to_string(),
        }
    }
}

fn text_input_long<S: TextBuffer>(ui: &mut egui::Ui, input_field_txt: &mut S) -> Response {
    ui.add(egui::TextEdit::singleline(input_field_txt).desired_width(300f32))
}

fn request_update_every(ctx: eframe::epi::Frame, interval: Duration) {
    thread::spawn(move || loop {
        thread::sleep(interval);
        ctx.request_repaint();
    });
}

impl App for ImgOfImgsGui {
    fn update(&mut self, ctx: &egui::CtxRef, frame: &eframe::epi::Frame) {
        // handle events
        self.handle_events();


        request_update_every(frame.clone(), Duration::from_millis(100));

        egui::CentralPanel::default().show(&ctx, |ui| {
            egui::Grid::new("Config")
                // .min_col_width(200f32)
                .max_col_width(500f32)
                .show(ui, |ui| {
                    self.add_path_input(ui, FileDialogType::TargetImgPath);
                    self.add_path_input(ui, FileDialogType::InputFolderPath);
                    self.add_path_input(ui, FileDialogType::OutputFolderPath);
                    self.add_number_input(ui, NumInputType::NumHorizontalImgs);
                    self.add_number_input(ui, NumInputType::NumVerticalImgs);
                    self.add_number_input(ui, NumInputType::TargetImgWidth);
                });

            if !self.processing {
                if ui.button("Create image of images").clicked() && !self.processing {
                    self.processing = true;
                    if let Err(e) = self.start_make_img_of_imgs() {
                        self.progress_text = Some(format!("Error in input fields: {}", e))
                    }
                }
            }

            if let Some(txt) = &self.progress_text {
                ui.label(txt);
            }

        });
    }

    fn name(&self) -> &str {
        "Image of Images"
    }

    fn setup(&mut self, _ctx: &egui::CtxRef, _frame: &eframe::epi::Frame, storage: Option<&dyn eframe::epi::Storage>) {
        if let Some(storage) = storage {
            let defaults = Self::default();

            self.target_img_path = storage.get_string("target_img").unwrap_or(defaults.target_img_path);
            self.input_folder_path = storage.get_string("input_folder").unwrap_or(defaults.input_folder_path);
            self.output_folder_path = storage.get_string("output_folder").unwrap_or(defaults.output_folder_path);
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::epi::Storage) {
        storage.set_string("target_img", self.target_img_path.clone());
        storage.set_string("input_folder", self.input_folder_path.clone());
        storage.set_string("output_folder", self.output_folder_path.clone());
    }

    fn auto_save_interval(&self) -> std::time::Duration {
        Duration::from_secs(1)
    }

}

fn main() {
    let _ = dotenv::dotenv();
    env_logger::init();
    let app = ImgOfImgsGui::default();

    eframe::run_native(Box::new(app), NativeOptions::default());
}
