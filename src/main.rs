mod shader;
mod hsb;

use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};
use eframe::egui;
use eframe::egui::{menu, IconData, TopBottomPanel, Ui};
use image::{DynamicImage, GenericImage, ImageBuffer};
use log::warn;
use threadpool::ThreadPool;
use crate::shader::PixelPos;

const NBR_OF_THREADS: usize = 10;

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
    image_actual: Option<DynamicImage>,
    image_eframe_texture: Option<egui::TextureHandle>,
    thread_pool: ThreadPool,
}
impl App {
    fn new() -> Self {
        Self {
            ui_values: UIFields::default(),
            image_actual: None,
            image_eframe_texture: None,
            thread_pool: ThreadPool::new(NBR_OF_THREADS),
        }
    }

    fn display_width_text_edit_field(&mut self, ui: &mut Ui) {
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
    }
    
    fn display_height_text_edit_field(&mut self, ui: &mut Ui) {
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
    }
    
    fn display_frame_generation_time(&mut self, ui: &mut Ui) {
        let s = match self.ui_values.frame_gen_time {
            Some(s) => format!("{:?}", s),
            None => "".to_string(),
        };
        ui.label(format!("Time to generate frame: {s}"));
    }
    
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
    
    fn apply_shader(&mut self, ctx: &egui::Context) {
        if self.image_actual.is_none() {
            return;
        }
        let now = Instant::now();
        
        let img = self.image_actual.as_mut().unwrap();
        let width = img.width();
        let height = img.height();
        
        for y in 0..height {
            for x in 0..width {
                let (r, g, b) = shader::shader_spiral(PixelPos{x, y},
                                                      shader::Dimensions {width, height}, ());
                let (r, g, b) = ((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8);
                
                img.blend_pixel(x, y, image::Rgba::<u8>([r, g, b, 255]));
            }
        }
        
        self.ui_values.frame_gen_time = Some(now.elapsed());
        
        self.renew_texture_handle(ctx);
    }
    
    fn apply_shader2(&mut self, ctx: &egui::Context) {
        if self.image_actual.is_none() {
            warn!("Tried to apply shader, however no image is loaded!");
            return;
        }
        
        let uniforms = shader::RaytracingUniforms{aabbs: vec![
            shader::Aabb::test_instance1(),
            shader::Aabb::test_instance2(),
        ]};
        let uniforms_ref = Arc::new(uniforms);
        
        let now = Instant::now();
        
        let img = self.image_actual.as_mut().unwrap();
        let width = img.width();
        let height = img.height();
        
        let (channel_sender, channel_receiver) = mpsc::channel::<(u32, Vec<u8>)>();
        
        for y in 0..height {
            let sender = channel_sender.clone();
            let uniforms = uniforms_ref.clone();
            
            self.thread_pool.execute(move || {
                let mut row = Vec::<u8>::with_capacity((width * 3) as usize);
                
                for x in 0..width {
                    let (r, g, b) = shader::shader_raytracing(PixelPos{x, y},
                                                          shader::Dimensions {width, height}, &uniforms);
                    let (r, g, b) = ((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8);
                    
                    row.push(r);
                    row.push(g);
                    row.push(b);
                }
                
                sender.send((y, row)).unwrap();
            })
        }
        
        let mut done_rows = 0;
        while done_rows < height {  //TODO surely this shit can be optimised by copying the array into the image or smth
            let (y, row) = channel_receiver.recv().expect("Channel unexpectedly closed! The render process was not yet done.");
            let mut iter = row.into_iter();
            let mut x = 0;
            while let (Some(r), Some(g), Some(b)) = 
                (iter.next(), iter.next(), iter.next()) {
                img.blend_pixel(x, y, image::Rgba::<u8>([r, g, b, 255]));
                x += 1;
            }
            done_rows += 1;
        }
        
        self.thread_pool.join();    //theoretically does nothing, it's here just in case
        
        self.ui_values.frame_gen_time = Some(now.elapsed());
        
        self.renew_texture_handle(ctx);
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
struct UIFields {
    width: u32,
    height: u32,
    frame_gen_time: Option<Duration>,
}
impl Default for UIFields {
    fn default() -> Self {
        Self {
            width: 600,
            height: 400,
            frame_gen_time: None,
        }
    }
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
                    }
                    if ui.button("Apply Shader").clicked() {
                        self.apply_shader2(ctx);
                    }
                });
            });
        });
        
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| { 
                ui.horizontal_top(|ui| {
                    self.display_width_text_edit_field(ui);
                    self.display_height_text_edit_field(ui);
                    self.display_frame_generation_time(ui);
                });
                
                
                
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
                }
            });
        });
    }
}
