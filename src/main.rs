mod shader;
mod custom_image;

use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};
use eframe::egui;
use eframe::egui::{menu, IconData, TopBottomPanel, Ui};
use image::{DynamicImage, GenericImage, ImageBuffer};
use log::{info, warn};
use nalgebra::{point, vector};
use threadpool::ThreadPool;
use crate::shader::PixelPos;

const NBR_OF_THREADS: usize = 20;
const NBR_OF_THREADS_MAX: usize = 50;
const NBR_OF_ITERATIONS: u32 = 32;

fn main() -> eframe::Result {
    //Set up logging for the project
    std::env::set_var("RUST_LOG", "info");
    env_logger::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([600.0, 400.0])
            .with_icon(IconData::default()),
        ..Default::default()
    };

    eframe::run_native(
        "Physical Ray-Tracer with eframe support",
        options,
        Box::new(|cc| {
            //image support
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(App::new()))
        })
    )
}

struct App {
    ui_values: UIFields,
    image_float: Option<custom_image::CustomImage>,
    image_actual: Option<DynamicImage>,
    image_eframe_texture: Option<egui::TextureHandle>,
    thread_pool: ThreadPool,
}
impl App {
    fn new() -> Self {
        Self {
            ui_values: UIFields::default(),
            image_float: None,
            image_actual: None,
            image_eframe_texture: None,
            thread_pool: ThreadPool::new(NBR_OF_THREADS),
        }
    }

    /// Shortcut function to display the width text field including label horizontally.
    fn display_width_text_edit_field(&mut self, ui: &mut Ui) {
        ui.vertical_centered(|ui| { 
            ui.horizontal_top(|ui| {
                ui.label("Width:");
                let mut width_string = self.ui_values.width.to_string();
                ui.text_edit_singleline(&mut width_string);
                if width_string.parse::<u32>().is_ok() {
                    let num = width_string.parse::<u32>().unwrap();
                    if num != 0 {
                        self.ui_values.width = num;
                    } else {
                        self.ui_values.width = 1;
                    }
                } else if width_string.is_empty() {
                    self.ui_values.width = 1;
                }
            });
        });
    }
    
    /// Shortcut function to display the height text field including label horizontally. 
    fn display_height_text_edit_field(&mut self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            ui.horizontal_top(|ui| {
                ui.label("Height:");
                let mut height_string = self.ui_values.height.to_string();
                ui.text_edit_singleline(&mut height_string);
                if height_string.parse::<u32>().is_ok() {
                    let num = height_string.parse::<u32>().unwrap();
                    if num != 0 {
                        self.ui_values.height = num;
                    } else {
                        self.ui_values.height = 1;
                    }
                } else if height_string.is_empty() {
                    self.ui_values.height = 1;
                }
            });
        });
    }
    
    /// Shortcut function to display the text field managing the number of frames including label
    /// horizontally. 
    fn display_nbr_of_iterations_edit_field(&mut self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            ui.horizontal_top(|ui| {
                ui.label("Number of frames:");
                let mut nbr_of_iterations_string = self.ui_values.nbr_of_iterations.to_string();
                ui.text_edit_singleline(&mut nbr_of_iterations_string);
                if nbr_of_iterations_string.parse::<u32>().is_ok() {
                    let num = nbr_of_iterations_string.parse::<u32>().unwrap();
                    if num != 0 {
                        self.ui_values.nbr_of_iterations = num;
                    } else {
                        self.ui_values.nbr_of_iterations = NBR_OF_ITERATIONS;
                    }
                } else if nbr_of_iterations_string.is_empty() {
                    self.ui_values.nbr_of_iterations = NBR_OF_ITERATIONS;
                }
            });
        });
    }
    
    /// Shortcut function to display the text field managing the number of threads including label
    /// horizontally. 
    fn display_nbr_of_threads_edit_field(&mut self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            ui.horizontal_top(|ui| {
                ui.label("Number of parallel threads:");
                ui.add(egui::Slider::new(&mut self.ui_values.nbr_of_threads, 1..=NBR_OF_THREADS_MAX));
            });
        });
    }
    
    /// Shortcut function that generates and displays the time taken to render the image. 
    fn display_frame_generation_time(&mut self, ui: &mut Ui) {
        let s = match self.ui_values.frame_gen_time {
            Some(s) => format!("{:?}", s),
            None => "".to_string(),
        };
        ui.label(format!("Time to generate frame: {s}"));
    }
    
    /// Generates the image in which the render result will be stored as soon as the CustomImage is 
    /// no longer necessary. This image has to be generated beforehand. 
    fn generate_image_actual(&mut self, ctx: &egui::Context) {
        let width = self.ui_values.width;
        let height = self.ui_values.height;
        
        let data = (0..(width * height * 3)).map(|_| 0u8).collect();
        let img = DynamicImage::ImageRgb8(ImageBuffer::from_vec(width, height, data).unwrap());
        
        self.image_actual = Some(img.clone());
        
        let rgb_img = img.to_rgba8();
        let size = [rgb_img.width() as usize, rgb_img.height() as usize];
        let pixels = rgb_img.as_raw();
        let color_image = 
            egui::ColorImage::from_rgba_unmultiplied(size, pixels);
        
        self.image_eframe_texture = Some(
            ctx.load_texture("dynamic_image", color_image, egui::TextureOptions::default())
        );
    }
    
    /// Generates a new blank CustomImage //TODO insert actual link
    /// and moves it to the main app. 
    fn generate_image_float(&mut self) {
        let width = self.ui_values.width;
        let height = self.ui_values.height;
        
        let img = custom_image::CustomImage::new(width, height);
        self.image_float = Some(img);
    }
    
    /// A single frame render process. Takes the uniforms and mixes the image into the CustomImage TODO actual reference
    /// at the appropriate level. 
    fn apply_shader2(&mut self, ctx: &egui::Context, uniforms: Arc<shader::RaytracingUniforms>) {
        let img = self.image_float.as_mut().unwrap();
        let width = img.get_width();
        let height = img.get_height();
        
        let (channel_sender, channel_receiver) = mpsc::channel::<(u32, Vec<f32>)>();
        
        for y in 0..height {
            let sender = channel_sender.clone();
            let uniforms = uniforms.clone();
            
            self.thread_pool.execute(move || {
                let mut row = Vec::<f32>::with_capacity((width * 4) as usize);
                
                for x in 0..width {
                    let (r, g, b) = 
                        shader::ray_generation_shader(
                            PixelPos{x, y}, 
                            shader::Dimensions {width, height}, 
                            &uniforms);
                    
                    row.push(r);
                    row.push(g);
                    row.push(b);
                }
                
                sender.send((y, row)).unwrap();
            })
        }
        
        let mut done_rows = 0;
        while done_rows < height { 
            let (y, row) = channel_receiver.recv().expect("Channel unexpectedly closed! The render process was not yet done.");
            let mut iter = row.into_iter();
            let mut x = 0;
            while let (Some(r), Some(g), Some(b)) = 
                (iter.next(), iter.next(), iter.next()) {
                let ratio = 1.0 / (uniforms.frame_id + 1) as f32;
                img.blend_pixel(x, y as usize, &custom_image::Pixel { r, g, b, a: 1.0 }, ratio).unwrap();
                x += 1;
            }
            done_rows += 1;
        }
        
        self.thread_pool.join();    //theoretically does nothing, it's here just in case
        
        //self.renew_texture_handle(ctx);
    }
    
    /// Starts the render process. Determines the uniforms and then renders the set amount of 
    /// frames. 
    fn render(&mut self, ctx: &egui::Context) {
        if self.image_actual.is_none() {
            warn!("Tried to apply shader, however no image is loaded!");
            return;
        }

        let mut uniforms = shader::RaytracingUniforms{
            aabbs: Arc::new(vec![
                shader::Aabb::new_box(&point![-1.5, 0.0, 1.0], 0.25, 3.0, 3.0),
                shader::Aabb::new_sphere(&point![0.0, 0.0, 1.0], 1.0),
                shader::Aabb::new_sphere(&point![1.0, 0.0, 1.0], 1.0),

                shader::Aabb::new_box(&point![0.0, -1.0, 0.0], 50.0, 0.1, 50.0),
            ]),
            lights: Arc::new(vec![
                shader::Light::new(point![0.0, 2.0, -1.0], 10.0),
                shader::Light::new(point![0.0, 1_000.0, 0.0], 1_000_000.0),
            ]),
            camera: shader::Camera::new(point![0.0, 0.0, 0.0], vector![0.0, 0.0, 1.0], 60.0),
            frame_id: 0,
        };
        
        self.thread_pool.set_num_threads(self.ui_values.nbr_of_threads);

        let now = Instant::now();
        
        for frame_number in 0..self.ui_values.nbr_of_iterations {
            uniforms.frame_id = frame_number;
            let uniforms_ref = Arc::new(uniforms.clone());
            self.apply_shader2(ctx, uniforms_ref.clone());
            self.ui_values.frame_gen_time = Some(now.elapsed());
            info!("Completed Frame #{}", frame_number);
        }
        
        self.image_actual = Some(self.image_float.clone().unwrap().into());
        self.renew_texture_handle(ctx);

        //self.ui_values.frame_gen_time = Some(now.elapsed());
    }
    
    fn renew_texture_handle(&mut self, ctx: &egui::Context) {
        if self.image_actual.is_none() {
            self.image_eframe_texture = None;
            return;
        }
        
        let img = self.image_actual.clone().unwrap();

        let rgb_img = img.to_rgba8();
        let size = [rgb_img.width() as usize, rgb_img.height() as usize];
        let pixels = rgb_img.as_raw();
        let color_image =
            egui::ColorImage::from_rgba_unmultiplied(size, pixels);

        self.image_eframe_texture = Some(
            ctx.load_texture("dynamic_image", color_image, egui::TextureOptions::default())
        );
    }
}

/// This struct simply holds all values that will be mutated via the UI. It serves to differentiate 
/// the main app from the clutter that are these additional fields. As soon as the rendering 
/// process begins, these values are snapshot for the entire duration of this process. 
struct UIFields {
    width: u32,
    height: u32,
    frame_gen_time: Option<Duration>,
    nbr_of_iterations: u32,
    nbr_of_threads: usize,
    tab: UiTab,
}
impl Default for UIFields {
    fn default() -> Self {
        Self {
            width: 600,
            height: 400,
            frame_gen_time: None,
            nbr_of_iterations: NBR_OF_ITERATIONS,
            nbr_of_threads: NBR_OF_THREADS,
            tab: UiTab::Settings,
        }
    }
}

/// This enum differentiates which tab is currently displayed in the apps main content window. 
enum UiTab {
    Settings,   //pre render settings such as width, height or number of frames
    Objects,    //3D models and lights defined in the scene
    Display,    //the screen ultimately displaying the result 
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.add_enabled(self.image_actual.is_some(), 
                                      egui::Button::new("Save Image"))
                        .clicked() {
                        
                        let dialog = rfd::FileDialog::new()
                            .set_file_name("image.png")
                            .save_file();
                        if let Some(path) = dialog {
                            let clone = self.image_actual.clone().unwrap();
                            match clone.save(path) {
                                Ok(_) => (),
                                Err(e) => {warn!("Error saving image: {:?}", e);},
                            }
                        }
                    }
                });
                ui.menu_button("Edit", |ui| {
                    if ui.button("Generate new blank Image").clicked() {
                        self.generate_image_actual(ctx);
                        self.generate_image_float();
                    }
                    if ui.button("Start Rendering").clicked() {
                        self.render(ctx);
                    }
                    if ui.button("Reset Settings to default").clicked() {
                        self.ui_values = UIFields::default();
                    }
                });
            });
        });
        
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.horizontal_top(|ui| {
                    if ui.button("Settings").clicked() {
                        self.ui_values.tab = UiTab::Settings;
                    }
                    if ui.button("Objects").clicked() {
                        self.ui_values.tab = UiTab::Objects;
                    }
                    if ui.button("Display").clicked() {
                        self.ui_values.tab = UiTab::Display;
                    }
                });
            });
            
            match self.ui_values.tab {
                UiTab::Settings => {
                    self.display_width_text_edit_field(ui);
                    self.display_height_text_edit_field(ui);
                    self.display_nbr_of_threads_edit_field(ui);
                    self.display_nbr_of_iterations_edit_field(ui);
                }
                UiTab::Objects => {
                    todo!() 
                }
                UiTab::Display => {
                    ui.horizontal_top(|ui| {
                        self.display_frame_generation_time(ui);
                    });

                    egui::Frame::NONE.fill(egui::Color32::GRAY).show(ui, |ui| {
                        if let Some(ref img) = self.image_eframe_texture {
                            egui::ScrollArea::both().show(ui, |ui| {
                                ui.add(
                                    egui::Image::from_texture(img).fit_to_original_size(1.0)
                                );
                            });
                        } else if ui.button("Start generating image").clicked() {
                            self.generate_image_actual(ctx);
                            self.generate_image_float();
                        }
                    });
                }
            }
        });
    }
}
