mod shader;
mod custom_image;

use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};
use eframe::egui;
use eframe::egui::{menu, IconData, TopBottomPanel, Ui};
use image::{DynamicImage, ImageBuffer};
use log::{info, warn};
use nalgebra::{point};
use threadpool::ThreadPool;
use crate::shader::PixelPos;

const NBR_OF_THREADS_DEFAULT: usize = 20;
const NBR_OF_THREADS_MAX: usize = 50;
const NBR_OF_ITERATIONS_DEFAULT: u32 = 32;

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

/// Struct that forms the main data of the app. The struct contains data such as the generated 
/// images or the values input into the UI. 
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
            thread_pool: ThreadPool::new(NBR_OF_THREADS_DEFAULT),
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
                        self.ui_values.nbr_of_iterations = NBR_OF_ITERATIONS_DEFAULT;
                    }
                } else if nbr_of_iterations_string.is_empty() {
                    self.ui_values.nbr_of_iterations = NBR_OF_ITERATIONS_DEFAULT;
                }
                
                if ui.button("Single Frame").clicked() {
                    self.ui_values.nbr_of_iterations = 1;
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
    
    /// Shortcut function to display various settings for the camera. The settings can be changed 
    /// and the updated values will be used in the rendering process. 
    fn display_camera_settings(&mut self, ui: &mut Ui) {
        //camera position
        ui.horizontal_top(|ui| {
            let mut pos_x_string = self.ui_values.ui_camera.pos_x.to_string();
            let mut pos_y_string = self.ui_values.ui_camera.pos_y.to_string();
            let mut pos_z_string = self.ui_values.ui_camera.pos_z.to_string();
            ui.label("Camera Position: (x:");
            ui.add_sized([80.0, 18.0], egui::TextEdit::singleline(&mut pos_x_string));
            ui.label("y:");
            ui.add_sized([80.0, 18.0], egui::TextEdit::singleline(&mut pos_y_string));
            ui.label("z:");
            ui.add_sized([80.0, 18.0], egui::TextEdit::singleline(&mut pos_z_string));
            ui.label(") CURRENTLY DOES NOTHING");   //TODO remove as soon as wrong

            if pos_x_string.parse::<f32>().is_ok() {
                self.ui_values.ui_camera.pos_x = pos_x_string.parse::<f32>().unwrap();
            }
            if pos_y_string.parse::<f32>().is_ok() {
                self.ui_values.ui_camera.pos_y = pos_y_string.parse::<f32>().unwrap();
            }
            if pos_z_string.parse::<f32>().is_ok() {
                self.ui_values.ui_camera.pos_z = pos_z_string.parse::<f32>().unwrap();
            }
        });
        
        //camera direction
        ui.horizontal_top(|ui| {
            let mut dir_x_string = self.ui_values.ui_camera.dir_x.to_string();
            let mut dir_y_string = self.ui_values.ui_camera.dir_y.to_string();
            let mut dir_z_string = self.ui_values.ui_camera.dir_z.to_string();

            ui.label("Camera Direction: (x:");
            ui.add_sized([80.0, 18.0], egui::TextEdit::singleline(&mut dir_x_string));
            ui.label("y:");
            ui.add_sized([80.0, 18.0], egui::TextEdit::singleline(&mut dir_y_string));
            ui.label("z:");
            ui.add_sized([80.0, 18.0], egui::TextEdit::singleline(&mut dir_z_string));
            ui.label(") CURRENTLY DOES NOTHING");   //TODO remove as soon as wrong

            if dir_x_string.parse::<f32>().is_ok() {
                self.ui_values.ui_camera.dir_x = dir_x_string.parse::<f32>().unwrap();
            }
            if dir_y_string.parse::<f32>().is_ok() {
                self.ui_values.ui_camera.dir_y = dir_y_string.parse::<f32>().unwrap();
            }
            if dir_z_string.parse::<f32>().is_ok() {
                self.ui_values.ui_camera.dir_z = dir_z_string.parse::<f32>().unwrap();
            }
        });
        
        //camera FOV
        ui.horizontal_top(|ui| {
            ui.label("Camera vertical FOV in degrees:");
            let mut fov_string = self.ui_values.ui_camera.fov_deg_y.to_string();

            ui.add_sized([80.0, 18.0], egui::TextEdit::singleline(&mut fov_string));

            if fov_string.parse::<f32>().is_ok() {
                self.ui_values.ui_camera.fov_deg_y = fov_string.parse::<f32>().unwrap();
            }
        });
    }
    
    /// Shortcut function to display various settings for a single Light object. The settings can 
    /// be changed and the updated values will be used in the rendering process. 
    fn display_light_source_settings(&mut self, ui: &mut Ui, index: usize) { 
        let light = &mut self.ui_values.ui_lights[index];
        
        //name
        ui.horizontal_top(|ui| {
            let name = format!("Light Source #{}", index);
            ui.label(name);
            ui.add_space(100.0);
            
            let delete_button = egui::widgets::Button::new("Delete this light source").fill(egui::Color32::LIGHT_RED);
            if ui.add(delete_button).clicked() {
            //if ui.button("Delete this light source").clicked() {
                self.ui_values.after_ui_actions.push(AfterUIActions::DeleteLight(index));
                info!("Light Source #{} has been scheduled for deletion.", index);
            }
        });
        
        //light position
        ui.horizontal_top(|ui| {
            let mut pos_x_string = light.pos_x.to_string();
            let mut pos_y_string = light.pos_y.to_string();
            let mut pos_z_string = light.pos_z.to_string();
            ui.label("Light Position: (x:");
            ui.add_sized([80.0, 18.0], egui::TextEdit::singleline(&mut pos_x_string));
            ui.label("y:");
            ui.add_sized([80.0, 18.0], egui::TextEdit::singleline(&mut pos_y_string));
            ui.label("z:");
            ui.add_sized([80.0, 18.0], egui::TextEdit::singleline(&mut pos_z_string));
            ui.label(")");

            if pos_x_string.parse::<f32>().is_ok() {
                light.pos_x = pos_x_string.parse::<f32>().unwrap();
            }
            if pos_y_string.parse::<f32>().is_ok() {
                light.pos_y = pos_y_string.parse::<f32>().unwrap();
            }
            if pos_z_string.parse::<f32>().is_ok() {
                light.pos_z = pos_z_string.parse::<f32>().unwrap();
            }
        });
        
        //light intensity
        ui.horizontal_top(|ui| {
            let mut intensity_string = light.intensity.to_string();
            ui.label("Light Intensity: ");
            ui.add_sized([80.0, 18.0], egui::TextEdit::singleline(&mut intensity_string));

            if intensity_string.parse::<f32>().is_ok() {
                let input = intensity_string.parse::<f32>().unwrap();
                if input >= 0.0 {
                    light.intensity = input;
                }
            }
        });
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
    fn apply_shader2(&mut self, _ctx: &egui::Context, uniforms: Arc<shader::RaytracingUniforms>) {
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
            lights: Arc::new(self.ui_values.ui_lights.iter().map(|uil| uil.into()).collect()),
            camera: shader::Camera::from(&self.ui_values.ui_camera),
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
    after_ui_actions: Vec<AfterUIActions>,
    ui_camera: UICamera,
    ui_lights: Vec<UILight>, 
}
impl Default for UIFields {
    fn default() -> Self {
        let ui_lights = vec![
            UILight::new(0.0, 2.0, -1.0, 10.0),
            UILight::new(0.0, 1_000.0, 0.0, 1_000_000.0),
        ];
        
        Self {
            width: 600,
            height: 400,
            frame_gen_time: None,
            nbr_of_iterations: NBR_OF_ITERATIONS_DEFAULT,
            nbr_of_threads: NBR_OF_THREADS_DEFAULT,
            tab: UiTab::Settings,
            after_ui_actions: Vec::new(),
            ui_camera: UICamera::default(),
            ui_lights,
        }
    }
}

/// This struct is a collection of values which can be assembled to a Light object. Coupled values
/// such as position x, y and z are separated here to allow for easier manipulation by the ui. 
#[derive(Debug)]
struct UILight {
    pos_x: f32,
    pos_y: f32,
    pos_z: f32,
    intensity: f32,
}

impl UILight {
    pub fn new(pos_x: f32, pos_y: f32, pos_z: f32, intensity: f32) -> Self {
        Self {
            pos_x,
            pos_y,
            pos_z,
            intensity,
        }
    }
}

/// This struct is a collection of values which can be assembled to a Camera object. Coupled values
/// such as position x, y and z are separated here to allow for easier manipulation by the ui. 
struct UICamera {
    pos_x: f32,
    pos_y: f32,
    pos_z: f32,
    dir_x: f32,
    dir_y: f32,
    dir_z: f32,
    fov_deg_y: f32,
}

impl Default for UICamera {
    fn default() -> Self {
        Self {
            pos_x: 0.0,
            pos_y: 0.0,
            pos_z: 0.0,
            dir_x: 0.0,
            dir_y: 0.0,
            dir_z: 1.0,
            fov_deg_y: 60.0,
        }
    }
}


/// This enum differentiates which tab is currently displayed in the apps main content window. 
enum UiTab {
    Settings,   //pre render settings such as width, height or number of frames
    Objects,    //3D models and lights defined in the scene
    Display,    //the screen ultimately displaying the result 
}

/// This enum describes a number of actions which have to be taken after the UI is displayed such 
/// as deleting objects. 
enum AfterUIActions {
    DeleteLight(usize),
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) { //UI is defined here
        //Top Menu bar (File, Edit, ...)
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
        
        //main content div. 
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.horizontal_top(|ui| {    //TODO these buttons might be replaceable by frames with zero outer margin
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
            
            //a dividing line between category buttons and the main content
            ui.add(egui::Separator::default().horizontal().grow(10.0));
            
            //content depending on the tab state 
            match self.ui_values.tab {
                UiTab::Settings => {
                    self.display_width_text_edit_field(ui);
                    self.display_height_text_edit_field(ui);
                    self.display_nbr_of_threads_edit_field(ui);
                    self.display_nbr_of_iterations_edit_field(ui);
                }
                UiTab::Objects => {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.label("Camera:");
                        egui::Frame::NONE.fill(egui::Color32::LIGHT_GRAY).inner_margin(5.0).show(ui, |ui| {
                            self.display_camera_settings(ui);
                        });
                        
                        ui.add_space(10.0);
                        
                        ui.vertical_centered(|ui| {
                            ui.horizontal_top(|ui| {
                                ui.label("Light Sources:");
                                ui.add_space(100.0);
                                if ui.button("Add New Light Source").clicked() {
                                    let light = UILight::new(0.0, 0.0, 0.0, 1.0);
                                    self.ui_values.ui_lights.push(light);
                                }
                            });
                        });
                        for index in 0..self.ui_values.ui_lights.len() {
                            egui::Frame::NONE.fill(egui::Color32::LIGHT_GRAY).inner_margin(5.0).show(ui, |ui| {
                                self.display_light_source_settings(ui, index);
                            });
                        }
                    });
                }
                UiTab::Display => {
                    ui.horizontal_top(|ui| {
                        self.display_frame_generation_time(ui);
                        //TODO implement progress bars per frame and overall
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

        //ui is finished drawing, but some actions have to be done after this point such as deleting
        //elements with a button press. 
        let mut lights_deleted = 0;
        for action in &self.ui_values.after_ui_actions {
            match action {
                AfterUIActions::DeleteLight(index) => {
                    self.ui_values.ui_lights.remove(*index - lights_deleted);
                    lights_deleted += 1;
                }
            }
        }
        self.ui_values.after_ui_actions.clear();
    }
}
