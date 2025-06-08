#![windows_subsystem = "windows"] //<- completely disables std::in/out/err. Uncomment only for final versions

mod shader;
mod custom_image;
mod spectrum;
mod spectral_data;
mod text_resources;

use std::cell::RefCell;
use std::cmp::PartialEq;
use std::fmt::{Display, Formatter};
use std::rc::Rc;
use std::sync::{mpsc, Arc, Mutex};
use std::sync::atomic::AtomicU32;
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::{Duration, Instant, UNIX_EPOCH};
use eframe::egui;
use eframe::egui::{menu, Color32, ComboBox, IconData, Sense, TextEdit, TopBottomPanel, Ui, UiBuilder};
use eframe::epaint::Vec2;
use image::DynamicImage;
use log::{error, warn};
use nalgebra::Vector3;
use threadpool::ThreadPool;
use crate::shader::{PixelPos, RaytracingUniforms};
use crate::spectrum::Spectrum;
use crate::text_resources::*;

const NBR_OF_THREADS_DEFAULT: usize = 20;
const NBR_OF_THREADS_MAX: usize = 64;
const NBR_OF_ITERATIONS_DEFAULT: u32 = 100;
const NBR_OF_SPECTRUM_SAMPLES_DEFAULT: usize = 32;

static COUNTER: AtomicU32 = AtomicU32::new(1);
fn get_id() -> u32 { COUNTER.fetch_add(1, core::sync::atomic::Ordering::Relaxed) }

fn main() -> eframe::Result {
    
    //////////////////////////////////////// TO ORIENT: ////////////////////////////////////////////
    // This is the entry point of the app, here logger and window settings are set.
    // After this, the eframe logic is started, calling main::App::update periodically, this is 
    // where the UI is defined. The UI contains buttons starting every other activity the app does.
    // The main data structure on which the entire app operates is main::App. 
    
    //Set up logging for the project
    std::env::set_var("RUST_LOG", "info");
    env_logger::init();

    //Set up the window which will open
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            //.with_inner_size([1280.0, 720.0])
            .with_maximized(true)
            .with_icon(IconData::default()),
        ..Default::default()
    };

    //Start the actual app business
    eframe::run_native(
        "Spectral Ray-Tracer with eframe GUI",
        options,
        Box::new(|cc| {
            //image support
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(App::new()))
        })
    )
}

//TODO implement serialization and deserialization of settings via the serde crate

/// Struct that forms the main data of the app. The struct contains data such as the generated 
/// images or the values input into the UI. 
struct App {
    ui_values: UIFields,
    image_actual: Option<DynamicImage>,
    image_eframe_texture: Option<egui::TextureHandle>,
    actions: Arc<Mutex<Vec<AppActions>>>,
    currently_rendering: Arc<Mutex<bool>>,
    rendering_since: Option<Instant>,
    app_to_render_channel: Option<mpsc::Sender<AppToRenderMessages>>,
}

impl App {
    fn new() -> Self {
        Self {
            ui_values: UIFields::default(),
            image_actual: None,
            image_eframe_texture: None,
            actions: Arc::new(Mutex::new(Vec::new())),
            currently_rendering: Arc::new(Mutex::new(false)),
            rendering_since: None,
            app_to_render_channel: None,
        }
    }

    /// Shortcut function to display the width text field including label horizontally.
    fn display_width_text_edit_field(&mut self, ui: &mut Ui) {
        ui.vertical_centered(|ui| { 
            ui.horizontal_top(|ui| {
                ui.label("Width:").on_hover_text(IMAGE_WIDTH_TOOLTIP);
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

                //diverse quick settings buttons
                if ui.button("HD").clicked() {
                    self.ui_values.width = 1280;
                    self.ui_values.height = 720;
                }
                if ui.button("FHD").clicked() {
                    self.ui_values.width = 1920;
                    self.ui_values.height = 1080;
                }
                if ui.button("QHD").clicked() {
                    self.ui_values.width = 2560;
                    self.ui_values.height = 1440;
                }
                if ui.button("UHD").clicked() {
                    self.ui_values.width = 3840;
                    self.ui_values.height = 2160;
                }
            });
        });
    }
    
    /// Shortcut function to display the height text field including label horizontally. 
    fn display_height_text_edit_field(&mut self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            ui.horizontal_top(|ui| {
                ui.label("Height:").on_hover_text(IMAGE_HEIGHT_TOOLTIP);
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
                ui.label("Number of frames:").on_hover_text(NUMBER_OF_ITERATIONS_TOOLTIP);
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
                ui.label("Number of parallel threads:").on_hover_text(NUMBER_OF_PARALLEL_THREADS_TOOLTIP);
                ui.add(egui::Slider::new(&mut self.ui_values.nbr_of_threads, 1..=NBR_OF_THREADS_MAX));
                if ui.button(" - ").clicked() {
                    self.ui_values.nbr_of_threads -= 1;
                }
                if ui.button(" + ").clicked() {
                    self.ui_values.nbr_of_threads += 1;
                }
            });
        });
    }
    
    /// Shortcut function that generates and displays the time taken to render the image. 
    fn display_frame_generation_time(&mut self, ui: &mut Ui) {
        let s = match self.ui_values.frame_gen_time {
            Some(s) => format!("{:.3?}", s),
            None => "".to_string(),
        };
        ui.label(format!("Time to generate image: {s}"));
    }
    
    /// Shortcut function to display various settings for the camera. The settings can be changed 
    /// and the updated values will be used in the rendering process. 
    fn display_camera_settings(&mut self, ui: &mut Ui) {
        //camera position
        ui.horizontal_top(|ui| {
            let mut pos_x_string = self.ui_values.ui_camera.pos_x.to_string();
            let mut pos_y_string = self.ui_values.ui_camera.pos_y.to_string();
            let mut pos_z_string = self.ui_values.ui_camera.pos_z.to_string();
            ui.label("Camera Position: (x:").on_hover_text(CAMERA_POSITION_TOOLTIP);
            ui.add_sized([80.0, 18.0], TextEdit::singleline(&mut pos_x_string));
            ui.label("y:");
            ui.add_sized([80.0, 18.0], TextEdit::singleline(&mut pos_y_string));
            ui.label("z:");
            ui.add_sized([80.0, 18.0], TextEdit::singleline(&mut pos_z_string));
            ui.label(")");

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

            ui.label("Camera Direction: (x:").on_hover_text(CAMERA_DIRECTION_TOOLTIP);
            ui.add_sized([80.0, 18.0], TextEdit::singleline(&mut dir_x_string));
            ui.label("y:");
            ui.add_sized([80.0, 18.0], TextEdit::singleline(&mut dir_y_string));
            ui.label("z:");
            ui.add_sized([80.0, 18.0], TextEdit::singleline(&mut dir_z_string));
            ui.label(")");

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

        //camera up direction
        ui.horizontal_top(|ui| {
            let mut up_x_string = self.ui_values.ui_camera.up_x.to_string();
            let mut up_y_string = self.ui_values.ui_camera.up_y.to_string();
            let mut up_z_string = self.ui_values.ui_camera.up_z.to_string();

            ui.label("Camera Up: (x:").on_hover_text(CAMERA_UP_TOOLTIP);
            ui.add_sized([80.0, 18.0], TextEdit::singleline(&mut up_x_string));
            ui.label("y:");
            ui.add_sized([80.0, 18.0], TextEdit::singleline(&mut up_y_string));
            ui.label("z:");
            ui.add_sized([80.0, 18.0], TextEdit::singleline(&mut up_z_string));
            ui.label(")");

            if up_x_string.parse::<f32>().is_ok() {
                self.ui_values.ui_camera.up_x = up_x_string.parse::<f32>().unwrap();
            }
            if up_y_string.parse::<f32>().is_ok() {
                self.ui_values.ui_camera.up_y = up_y_string.parse::<f32>().unwrap();
            }
            if up_z_string.parse::<f32>().is_ok() {
                self.ui_values.ui_camera.up_z = up_z_string.parse::<f32>().unwrap();
            }
        });
        
        //camera FOV
        ui.horizontal_top(|ui| {
            ui.label("Camera vertical FOV in degrees:").on_hover_text(CAMERA_FOV_TOOLTIP);
            let mut fov_string = self.ui_values.ui_camera.fov_deg_y.to_string();

            ui.add_sized([80.0, 18.0], TextEdit::singleline(&mut fov_string));

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
            let backup_name = &format!("Light Source #{index}");
            display_name_with_edit(ui, &mut light.name, backup_name, &mut light.editing_name);
            ui.add_space(100.0);
            
            let delete_button = egui::widgets::Button::new("Delete this light source").fill(Color32::LIGHT_RED);
            if ui.add(delete_button).clicked() {
                self.ui_values.after_ui_action = Some(AfterUIActions::DeleteLight(index));
            }
        });
        
        //light position
        ui.horizontal_top(|ui| {
            let mut pos_x_string = light.pos_x.to_string();
            let mut pos_y_string = light.pos_y.to_string();
            let mut pos_z_string = light.pos_z.to_string();
            ui.label("Light Position: (x:").on_hover_text(LIGHT_SOURCE_TOOLTIP);
            ui.add_sized([80.0, 18.0], TextEdit::singleline(&mut pos_x_string));
            ui.label("y:");
            ui.add_sized([80.0, 18.0], TextEdit::singleline(&mut pos_y_string));
            ui.label("z:");
            ui.add_sized([80.0, 18.0], TextEdit::singleline(&mut pos_z_string));
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

        //light spectrum
        ui.horizontal_top(|ui| {
            let label_color = if !self.ui_values.spectra.contains(&light.spectrum) && is_time_even() {
                Color32::RED
            } else {
                Color32::DARK_GRAY
            };
            ui.colored_label(label_color, "Spectrum").on_hover_text(LIGHT_SPECTRUM_TOOLTIP);

            let borrow = light.spectrum.borrow();
            let selected_text = borrow.to_string();
            drop(borrow);
            
            ComboBox::new(format!("light source {index} spectrum"), "")
                .selected_text(selected_text)
                .show_ui(ui, |ui| {
                    for spectrum in &self.ui_values.spectra {
                        ui.selectable_value(&mut light.spectrum, spectrum.clone(), spectrum.borrow().to_string());
                    }
                }).response.on_hover_text(LIGHT_SPECTRUM_TOOLTIP);
        });
    }
    
    /// Shortcut function to display the settings for a single Object in the scene. The settings 
    /// can be changed and the updated values will be used in the rendering process. Each object is 
    /// differentiated according to their type, and the respective settings will be displayed.
    fn display_objects_settings(&mut self, ui: &mut Ui, index: usize) {
        let object = &mut self.ui_values.ui_objects[index];

        //name
        ui.horizontal_top(|ui| {
            let backup_name = &format!("{object} #{index}");
            display_name_with_edit(ui, &mut object.name, backup_name, &mut object.editing_name);
            ui.add_space(30.0);
            
            #[derive(PartialEq, Clone, Copy, Debug)]
            enum Type {
                PlainBox,
                Sphere,
            }
            impl Display for Type {
                fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                    let s = match self {
                        Type::PlainBox => "PlainBox",
                        Type::Sphere => "Sphere",
                    };
                    write!(f, "{s}")
                }
            }
            let mut selected = match object.ui_object_type {
                UIObjectType::PlainBox(_, _, _) => Type::PlainBox,
                UIObjectType::Sphere(_) => Type::Sphere,
            };
            ComboBox::new(index, "Type")
                .selected_text(format!("{}", selected))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut selected, Type::PlainBox, "Plain Box");
                    ui.selectable_value(&mut selected, Type::Sphere, "Sphere");
                }).response.on_hover_text(OBJECT_TYPE_TOOLTIP);
            let same = selected == match object.ui_object_type {
                UIObjectType::PlainBox(_, _, _) => Type::PlainBox,
                UIObjectType::Sphere(_) => Type::Sphere,
            };
            if !same {
                object.ui_object_type = match selected {
                    Type::PlainBox => UIObjectType::default_plain_box(),
                    Type::Sphere => UIObjectType::default_sphere(),
                }
            }
            ui.add_space(30.0);

            let delete_button = egui::widgets::Button::new("Delete this object").fill(Color32::LIGHT_RED);
            if ui.add(delete_button).clicked() {
                self.ui_values.after_ui_action = Some(AfterUIActions::DeleteObject(index));
            }
        });
        
        //object position
        ui.horizontal_top(|ui| {
            let mut pos_x_string = object.pos_x.to_string();
            let mut pos_y_string = object.pos_y.to_string();
            let mut pos_z_string = object.pos_z.to_string();
            ui.label("Object Position: (x:").on_hover_text(OBJECT_POSITION_TOOLTIP);
            ui.add_sized([80.0, 18.0], TextEdit::singleline(&mut pos_x_string));
            ui.label("y:");
            ui.add_sized([80.0, 18.0], TextEdit::singleline(&mut pos_y_string));
            ui.label("z:");
            ui.add_sized([80.0, 18.0], TextEdit::singleline(&mut pos_z_string));
            ui.label(")");

            if pos_x_string.parse::<f32>().is_ok() {
                object.pos_x = pos_x_string.parse::<f32>().unwrap();
            }
            if pos_y_string.parse::<f32>().is_ok() {
                object.pos_y = pos_y_string.parse::<f32>().unwrap();
            }
            if pos_z_string.parse::<f32>().is_ok() {
                object.pos_z = pos_z_string.parse::<f32>().unwrap();
            }
        });
        
        //metallicness
        ui.horizontal_top(|ui| {
            ui.label("Metallic?").on_hover_text(OBJECT_METALLICNESS_TOOLTIP);
            ui.checkbox(&mut object.metallicness, "");
        });
        
        //type specific information
        match object.ui_object_type {
            UIObjectType::PlainBox(x_length, y_length, z_length) => {
                //dimensions
                ui.horizontal_top(|ui| {
                    let mut dim_x_string = x_length.to_string();
                    let mut dim_y_string = y_length.to_string();
                    let mut dim_z_string = z_length.to_string();
                    ui.label("Object Dimensions: (x:").on_hover_text(OBJECT_PLAIN_BOX_DIMENSIONS_TOOLTIP);
                    ui.add_sized([80.0, 18.0], TextEdit::singleline(&mut dim_x_string));
                    ui.label("y:");
                    ui.add_sized([80.0, 18.0], TextEdit::singleline(&mut dim_y_string));
                    ui.label("z:");
                    ui.add_sized([80.0, 18.0], TextEdit::singleline(&mut dim_z_string));
                    ui.label(")");

                    if dim_x_string.parse::<f32>().is_ok() {
                        let new_length_x = dim_x_string.parse::<f32>().unwrap();
                        if new_length_x > 0.0 && new_length_x != x_length {
                            object.ui_object_type = UIObjectType::PlainBox(new_length_x, y_length, z_length);
                        }
                    }
                    if dim_y_string.parse::<f32>().is_ok() {
                        let new_length_y = dim_y_string.parse::<f32>().unwrap();
                        if new_length_y > 0.0 && new_length_y != y_length {
                            object.ui_object_type = UIObjectType::PlainBox(x_length, new_length_y, z_length);
                        }
                    }
                    if dim_z_string.parse::<f32>().is_ok() {
                        let new_length_z = dim_z_string.parse::<f32>().unwrap();
                        if new_length_z > 0.0 && new_length_z != z_length {
                            object.ui_object_type = UIObjectType::PlainBox(x_length, y_length, new_length_z);
                        }
                    }
                });
            }
            UIObjectType::Sphere(radius) => {
                //radius
                ui.horizontal_top(|ui| {
                    let mut radius_string = radius.to_string();
                    ui.label("Radius: ").on_hover_text(OBJECT_SPHERE_RADIUS_TOOLTIP);
                    ui.add_sized([80.0, 18.0], TextEdit::singleline(&mut radius_string));
                    
                    if radius_string.parse::<f32>().is_ok() {
                        let new_radius = radius_string.parse::<f32>().unwrap();
                        if new_radius > 0.0 {
                            object.ui_object_type = UIObjectType::Sphere(new_radius);
                        }
                    }
                });
            }
        }

        //reflecting spectrum
        ui.horizontal_top(|ui| {
            let label_color = if !self.ui_values.spectra.contains(&object.spectrum) && is_time_even() {
                Color32::RED
            } else {
                Color32::DARK_GRAY
            };
            ui.colored_label(label_color, "Reflecting factor Spectrum:").on_hover_text(OBJECT_SPECTRUM_REFLECTING_TOOLTIP);
            
            let borrow = object.spectrum.borrow();
            let selected_text = borrow.to_string();
            drop(borrow);

            ComboBox::new(format!("object reflecting {index} spectrum"), "")
                .selected_text(selected_text)
                .show_ui(ui, |ui| {
                    for spectrum in &self.ui_values.spectra {
                        ui.selectable_value(&mut object.spectrum, spectrum.clone(), spectrum.borrow().to_string());
                    }
                }).response.on_hover_text(OBJECT_SPECTRUM_REFLECTING_TOOLTIP);
        });
    }

    /// Displays the settings which all spectra must have in common, such as the number of samples.
    fn display_general_spectrum_settings(&mut self, ui: &mut Ui) {
        //nbr of samples
        ui.horizontal_top(|ui| {
            let nbr_of_samples = &mut self.ui_values.spectrum_number_of_samples;
            let mut nbr_of_samples_string = nbr_of_samples.to_string();
            let mut final_nbr_of_samples = *nbr_of_samples;

            ui.label("Number of samples in the spectra:").on_hover_text(SPECTRUM_NUMBER_OF_SAMPLES_TOOLTIP);
            ui.add_sized([80.0, 18.0], TextEdit::singleline(&mut nbr_of_samples_string));

            if nbr_of_samples_string.parse::<usize>().is_ok() {
                let new_nbr_of_samples = nbr_of_samples_string.parse::<usize>().unwrap();
                if new_nbr_of_samples > 1 {
                    final_nbr_of_samples = new_nbr_of_samples;
                }
            }

            if ui.button("-").clicked() {
                if *nbr_of_samples % 8 == 0 {
                    if *nbr_of_samples == 8 {
                        final_nbr_of_samples = 2;    //at least two samples have to be present
                    } else {
                        final_nbr_of_samples -= 8;   //subtract 8
                    }
                } else {
                    final_nbr_of_samples = (*nbr_of_samples / 8 * 8).max(2)  //drop down to the nearest multiple of 8, at least 2
                }
            }

            if ui.button("+").clicked() {
                if *nbr_of_samples % 8 == 0 {
                    final_nbr_of_samples += 8;   //add 8
                } else {
                    final_nbr_of_samples = (*nbr_of_samples / 8 + 1) * 8;    //go up to the nearest multiple of 8
                }
            }

            if final_nbr_of_samples != *nbr_of_samples {
                self.ui_values.spectrum_number_of_samples = final_nbr_of_samples;
                self.update_all_spectrum_sample_sizes(final_nbr_of_samples);
            }
        });

        //range
        ui.horizontal_top(|ui| {    //TODO implement non direct change
            let lower_bound = &mut self.ui_values.spectrum_lower_bound;
            let upper_bound = &mut self.ui_values.spectrum_upper_bound;
            let mut lower_bound_string = lower_bound.to_string();
            let mut upper_bound_string = upper_bound.to_string();

            ui.label("Spectrum range from:").on_hover_text(SPECTRUM_RANGE_TOOLTIP);
            //ui.add_sized([80.0, 18.0], egui::TextEdit::singleline(&mut lower_bound_string));  //uncomment to make wavelength bounds editable
            ui.add_enabled(false,
                           TextEdit::singleline(&mut lower_bound_string).desired_width(80.0))
                .on_disabled_hover_text(SPECTRUM_WAVELENGTH_EDIT_NOT_SUPPORTED_TOOLTIP);
            ui.label("nm to:");
            //ui.add_sized([80.0, 18.0], egui::TextEdit::singleline(&mut upper_bound_string));
            ui.add_enabled(false,
                           TextEdit::singleline(&mut upper_bound_string).desired_width(80.0))
                .on_disabled_hover_text(SPECTRUM_WAVELENGTH_EDIT_NOT_SUPPORTED_TOOLTIP);
            ui.label("nm");

            if lower_bound_string.parse::<f32>().is_ok() {
                let new_lower_bound = lower_bound_string.parse::<f32>().unwrap();
                if 0.0 < new_lower_bound && new_lower_bound < *upper_bound {
                    *lower_bound = new_lower_bound;
                }
            }
            if upper_bound_string.parse::<f32>().is_ok() {
                let new_upper_bound = upper_bound_string.parse::<f32>().unwrap();
                if *lower_bound < new_upper_bound {
                    *upper_bound = upper_bound_string.parse::<f32>().unwrap();
                }
            }
        });
    }

    /// Displays the simple settings for a single spectrum. Simple settings are the spectrum type or
    /// the brightness factor. More complicated settings such as individual sample values go to the
    /// dedicated settings on the right in
    /// [display_spectrum_right_side](App::display_spectrum_right_side).
    fn display_spectrum_settings(&mut self, ui: &mut Ui, index: usize) {
        let ui_spectrum = &mut self.ui_values.spectra[index];
        let mut ui_spectrum = ui_spectrum.borrow_mut();
        
        //name and delete button
        ui.horizontal_top(|ui| {
            //name
            //moving out the bool since multiple mutable access are not allowed for Ref<_>. 
            let mut editing_name = ui_spectrum.editing_name;
            let backup_name = &format!("Spectrum {}", index);
            display_name_with_edit(ui, &mut ui_spectrum.name, backup_name, &mut editing_name);
            ui_spectrum.editing_name = editing_name;
            
            ui.add_space(80.0);

            let delete_button = egui::widgets::Button::new("Delete this Spectrum").fill(Color32::LIGHT_RED);
            if ui.add(delete_button).clicked() {
                self.ui_values.after_ui_action = Some(AfterUIActions::DeleteSpectrum(index));
            }
        });

        //spectrum type
        ui.horizontal_top(|ui| {
            ui.label("Spectrum type:").on_hover_text(SPECTRUM_TYPE_TOOLTIP);
            
            let mut selected_type = ui_spectrum.spectrum_type;
            ComboBox::new(format!("spectrum{}", index), "")   //the format is the ID salt, ensuring that each dropdown is distinct
                .selected_text(selected_type.to_string())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut selected_type, UISpectrumType::Custom, format!("{}", UISpectrumType::Custom));
                    ui.selectable_value(&mut selected_type, UISpectrumType::Solar(1.0), format!("{}", UISpectrumType::Solar(1.0)));
                    ui.selectable_value(&mut selected_type, UISpectrumType::PlainReflective(1.0), format!("{}", UISpectrumType::PlainReflective(1.0)));
                    ui.selectable_value(&mut selected_type, UISpectrumType::Temperature(1000.0, 1.0), format!("{}", UISpectrumType::Temperature(1.0, 1.0)));
                }).response.on_hover_text(SPECTRUM_TYPE_TOOLTIP);
            
            if selected_type != ui_spectrum.spectrum_type {
                ui_spectrum.spectrum_type = selected_type;
                match selected_type {
                    UISpectrumType::Custom => {}
                    UISpectrumType::Solar(factor) => {
                        let lower = self.ui_values.spectrum_lower_bound;
                        let upper = self.ui_values.spectrum_upper_bound;
                        let nbr_of_samples = self.ui_values.spectrum_number_of_samples;
                        ui_spectrum.spectrum = Spectrum::new_sunlight_spectrum(lower, upper, nbr_of_samples, factor);
                    }
                    UISpectrumType::PlainReflective(factor) => {
                        let lower = self.ui_values.spectrum_lower_bound;
                        let upper = self.ui_values.spectrum_upper_bound;
                        let nbr_of_samples = self.ui_values.spectrum_number_of_samples;
                        ui_spectrum.spectrum = Spectrum::new_singular_reflectance_factor(lower, upper, nbr_of_samples, factor);
                    }
                    UISpectrumType::Temperature(temp, factor) => {
                        let lower = self.ui_values.spectrum_lower_bound;
                        let upper = self.ui_values.spectrum_upper_bound;
                        let nbr_of_samples = self.ui_values.spectrum_number_of_samples;
                        ui_spectrum.spectrum = Spectrum::new_temperature_spectrum(lower, upper, temp, factor); 
                    }
                }
                self.ui_values.after_ui_action = Some(AfterUIActions::UpdateSelectedSpectrum(index));
            }
        });
        
        //spectrum reflectance
        ui.horizontal_top(|ui| {
            ui.label("Behavior:").on_hover_text(SPECTRUM_EFFECT_TYPE_TOOLTIP);
            
            let mut selected_type = ui_spectrum.spectrum_effect_type;
            ComboBox::new(format!("spectrum effect {}", index), "")
                .selected_text(selected_type.to_string())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut selected_type, SpectrumEffectType::Emissive, format!("{}", SpectrumEffectType::Emissive));
                    ui.selectable_value(&mut selected_type, SpectrumEffectType::Reflective, format!("{}", SpectrumEffectType::Reflective));
                }).response.on_hover_text(SPECTRUM_EFFECT_TYPE_TOOLTIP);
            
            if selected_type != ui_spectrum.spectrum_effect_type {
                ui_spectrum.spectrum_effect_type = selected_type;
                self.ui_values.after_ui_action = Some(AfterUIActions::UpdateSelectedSpectrum(index));
            }
        });

        //spectrum type sub settings
        let mut changed = false;
        match &mut ui_spectrum.spectrum_type {
            UISpectrumType::Solar(factor) | UISpectrumType::PlainReflective(factor) => {
                changed = display_factor(ui, factor) || changed;
            }
            UISpectrumType::Temperature(temp, factor) => {
                //temperature
                ui.horizontal_top(|ui| {
                    let mut temp_string = temp.to_string();

                    ui.label("Black body radiation temperature:");
                    ui.add_sized([80.0, 18.0], TextEdit::singleline(&mut temp_string));
                    ui.label("K");  //TODO add support for different temperature units

                    if temp_string.parse::<f32>().is_ok() {
                        let new_temp = temp_string.parse::<f32>().unwrap();
                        if new_temp != *temp && new_temp > 0.0 {
                            *temp = new_temp;
                            changed = true;
                        }
                    }
                });

                //factor
                changed = display_factor(ui, factor) || changed;
            }
            UISpectrumType::Custom => {}
        }


        drop(ui_spectrum);  //I just pray that this is future-proof
        if changed {
            self.update_spectrum()
        }
    }

    /// Displays the right side of spectrum settings. Here the user can preview the color of
    /// the spectrum and each samples individual value.
    fn display_spectrum_right_side(&mut self, ui: &mut Ui) {
        todo!();
        // match self.ui_values.selected_spectrum.as_mut() {
        //     Some(selected) => {
        //         let spectrum = Spectrum::new_from_list(&selected.spectrum_values, selected.lower_bound, selected.upper_bound);
        //         let (r, g, b) = spectrum.to_rgb_early();
        // 
        //         ui.horizontal_top(|ui| {
        //             ui.colored_label(Color32::RED, "Any changes will not be applied unless saved. Selecting another spectrum will discard changes!");
        //             if ui.button("Save").clicked() {
        //                 self.ui_values.after_ui_action = Some(AfterUIActions::SaveSelectedSpectrum(selected.selected_spectrum));
        //             }
        //         });
        // 
        //         match selected.spectrum_effect_type {
        //             SpectrumEffectType::Emissive => {
        //                 //color squares
        //                 ui.horizontal_top(|ui| {
        //                     //observed color
        //                     ui.vertical(|ui| {
        //                         let r_byte = (r.clamp(0.0, 1.0) * 255.0) as u8;
        //                         let g_byte = (g.clamp(0.0, 1.0) * 255.0) as u8;
        //                         let b_byte = (b.clamp(0.0, 1.0) * 255.0) as u8;
        //                         let luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        //                         let contrasting_text_color = if luminance < 0.5 { Color32::WHITE } else {Color32::BLACK};
        // 
        //                         egui::Frame::NONE.fill(Color32::from_rgb(r_byte, g_byte, b_byte))
        //                             .stroke(egui::Stroke::new(1.0, Color32::LIGHT_GRAY))
        //                             .show(ui, |ui| {
        //                                 ui.set_max_size(Vec2::new(200.0, 100.0));
        //                                 ui.centered_and_justified(|ui| {
        //                                     ui.colored_label(contrasting_text_color, format!("{r_byte:02X}{g_byte:02X}{b_byte:02X}"));
        //                                 });
        //                             }).response.on_hover_text(OBSERVED_COLOR_TOOLTIP);
        //                         ui.label("Observed Color").on_hover_text(OBSERVED_COLOR_TOOLTIP);
        //                     });
        // 
        //                     //normalized color
        //                     ui.vertical(|ui| {
        //                         let max = r.max(g.max(b));
        //                         let r_byte= (r / max * 255.0 + 0.5) as u8;
        //                         let g_byte= (g / max * 255.0 + 0.5) as u8;
        //                         let b_byte= (b / max * 255.0 + 0.5) as u8;
        // 
        //                         egui::Frame::NONE.fill(Color32::from_rgb(r_byte, g_byte, b_byte))
        //                             .stroke(egui::Stroke::new(1.0, Color32::LIGHT_GRAY))
        //                             .show(ui, |ui| {
        //                                 ui.set_max_size(Vec2::new(200.0, 100.0));
        //                                 ui.centered_and_justified(|ui| {
        //                                     ui.label(format!("{r_byte:02X}{g_byte:02X}{b_byte:02X}"));
        //                                 });
        //                             }).response.on_hover_text(NORMALIZED_COLOR_TOOLTIP);
        //                         ui.label("Normalized Color").on_hover_text(NORMALIZED_COLOR_TOOLTIP);
        //                     });
        //                 });
        //             }
        //             SpectrumEffectType::Reflective => {
        //                 //no color squares
        //                 ui.label("Color Preview not (yet) available for reflective spectra.");
        //             }
        //         }
        //         ui.add_space(5.0);
        //         
        //         //radiance
        //         if selected.spectrum_effect_type != SpectrumEffectType::Reflective {
        //             ui.horizontal_top(|ui| {
        //                 ui.label(format!("Radiance of the spectrum: {}W/sr/m^2",
        //                                  spectrum.get_radiance()))
        //                     .on_hover_text(SPECTRUM_RADIANCE_TOOLTIP);
        //             });
        //             ui.add_space(5.0);
        //         }
        // 
        //         //samples
        //         let editable = matches!(selected.ui_spectrum_type, UISpectrumType::Custom);
        //         let slider_max = match selected.spectrum_effect_type {
        //             SpectrumEffectType::Emissive => {selected.max * 2.0},
        //             SpectrumEffectType::Reflective => 1.0,
        //         };
        //         let unit_label = if selected.spectrum_effect_type == SpectrumEffectType::Emissive 
        //             {"W/sr/m^2/nm"} else {""};
        //         egui::ScrollArea::vertical().id_salt("right scroll area").show(ui, |ui| {
        //             for ((wavelength, _), spectral_radiance) in spectrum.iter().zip(selected.spectrum_values.iter_mut()) {
        //                 //TODO make multiple sliders adjustable
        //                 ui.horizontal_top(|ui| {
        //                     ui.label(format!("{wavelength:.2}nm:"));
        //                     ui.style_mut().spacing.slider_width = 300.0;
        //                     ui.add_enabled(
        //                         editable,
        //                         egui::Slider::new(spectral_radiance, 0.0..=slider_max)
        //                             .fixed_decimals(3)
        //                             .step_by(0.001)
        //                     ).on_disabled_hover_text(SPECTRUM_RIGHT_SLIDER_DISABLED_TOOLTIP);
        //                     ui.label(unit_label);
        //                 });
        //             }
        //         });
        //     }
        //     None => {
        //         ui.label("Select a spectrum on the left to start editing...");
        //     }
        // }

    }

    /// Displays a single tab for the UITabs up top.
    fn display_tab_frame(&mut self, ui: &mut Ui, label: &str, color: Color32, tab: UiTab) {
        if ui.scope_builder(UiBuilder::new().sense(Sense::click()), |ui| {
            egui::Frame::NONE.fill(color)
                .outer_margin(0.0)
                .inner_margin(5.0)
                .show(ui, |ui| {
                    let label = egui::Label::new(label)
                        .selectable(false);
                    ui.add(label);
                });
        }).response.clicked()  {
            self.ui_values.tab = tab;
        };
    }

    /// Takes the information from the UISpectrum at the given index, takes out all working
    /// information, stores it in the UISelectedSpectrum and displays these on the right and sight.
    fn update_selected_spectrum(&mut self, index: usize) {
        let ui_spectrum = self.ui_values.spectra[index].borrow();
        let working_vec: Vec<f32> = ui_spectrum.spectrum.iter().map(|(_, value)| value).collect();
        let max = working_vec.iter().fold(f32::NEG_INFINITY, |acc, elem| acc.max(*elem));
        let (lower, upper) = ui_spectrum.spectrum.get_range();

        let ui_selected_spectrum = UISelectedSpectrum {
            selected_spectrum: index,
            max,
            spectrum_values: working_vec,
            spectrum_effect_type: ui_spectrum.spectrum_effect_type,
            lower_bound: lower,
            upper_bound: upper,
            ui_spectrum_type: ui_spectrum.spectrum_type,
        };
        self.ui_values.selected_spectrum = Some(ui_selected_spectrum);
    }

    /// The displayed time how long an image has been rendered is updated in this method, if the 
    /// app is currently rendering. 
    fn refresh_rendering_time(&mut self) {
        let rendering = self.currently_rendering.lock().unwrap();
        if *rendering {
            if self.rendering_since.is_none() {
                self.rendering_since = Some(Instant::now());
            }
            let rendering_since = self.rendering_since.unwrap();
            self.ui_values.frame_gen_time = Some(Instant::now() - rendering_since);
        } else {
            self.rendering_since = None;
        }
    }

    /// Iterates over all ui spectra. All non-custom Spectra are simply generated again with the new
    /// sample size, for each custom spectrum [resample](Spectrum::resample) is called.
    fn update_all_spectrum_sample_sizes(&mut self, nbr_of_samples: usize) {
        for ui_spectrum_ref in &mut self.ui_values.spectra {
            let mut ui_spectrum = ui_spectrum_ref.borrow_mut();
            let lowest = self.ui_values.spectrum_lower_bound;
            let highest = self.ui_values.spectrum_upper_bound;
            
            match ui_spectrum.spectrum_type {
                UISpectrumType::Custom => {
                    ui_spectrum.spectrum.resample(nbr_of_samples);
                }
                UISpectrumType::Solar(factor) => {
                    ui_spectrum.spectrum = Spectrum::new_sunlight_spectrum(lowest, highest, nbr_of_samples, factor);
                }
                UISpectrumType::PlainReflective(factor) => {
                    ui_spectrum.spectrum = Spectrum::new_singular_reflectance_factor(lowest, highest, nbr_of_samples, factor);
                }
                UISpectrumType::Temperature(temp, factor) => { 
                    ui_spectrum.spectrum = Spectrum::new_temperature_spectrum(lowest, highest, temp, factor);
                }
            }
        }
        self.ui_values.after_ui_action = Some(AfterUIActions::DeselectSelectedSpectrum);
    }

    /// Updates all spectra. Currently, this simply calls [App::update_all_spectrum_sample_sizes].
    /// This forces every non-custom UISpectrum to re-generate.
    fn update_spectrum(&mut self) {
        self.update_all_spectrum_sample_sizes(self.ui_values.spectrum_number_of_samples)
    }

    /// Generates a button to abort the current rendering process. The button is disabled when
    /// nothing is being rendered.
    fn display_abort_button(&mut self, ui: &mut Ui) {
        let enabled = self.app_to_render_channel.is_some();
        let button = egui::Button::new("Abort")
            .fill(Color32::LIGHT_RED);
        if ui.add_enabled(enabled, button)
            .on_hover_text(DISPLAY_ABORT_RENDERING_BUTTON_TOOLTIP).clicked() {
                self.app_to_render_channel.as_mut().unwrap()
                    .send(AppToRenderMessages::AbortRender).unwrap()
        }
    }
    
    /// Generates a button to start the render process. Is disabled if 
    /// [check_render_legality](App::check_render_legality) returns false.
    fn display_start_render_button(&mut self, ui: &mut Ui) {
        let button_render =  egui::Button::new("Start generating image");
        let enabled = self.check_render_legality(); //disable button when rendering would crash
        if ui.add_enabled(enabled, button_render)
            .on_disabled_hover_text(DISPLAY_START_RENDERING_BUTTON_DISABLED_TOOLTIP)
            .clicked() {
            self.dispatch_render();
        }
    }
    
    /// A single frame render process. Takes the uniforms and mixes the image into the 
    /// [CustomImage](custom_image::CustomImage) at the appropriate level. 
    fn apply_shader2(img: &mut custom_image::CustomImage, uniforms: Arc<RaytracingUniforms>, thread_pool: &ThreadPool) {
        let width = img.get_width();
        let height = img.get_height();
        
        let (channel_sender, channel_receiver) = mpsc::channel::<(u32, Vec<f32>)>();
        
        for y in 0..height {
            let sender = channel_sender.clone();
            let uniforms = uniforms.clone();
            
            thread_pool.execute(move || {
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
            let (y, row) = channel_receiver.recv().expect("During the rendering process, a thread has terminated prematurely!");
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
    }

    /// The overarching render process, best started in another thread. Calls
    /// [apply_shader2](App::apply_shader2) for each frame and gives the result to the main thread
    /// to be displayed to the user.
    fn render(mut image_float: custom_image::CustomImage, mut uniforms: RaytracingUniforms,
              thread_pool: ThreadPool, nbr_of_iterations: u32, rendering:  Arc<Mutex<bool>>,
              action_list: Arc<Mutex<Vec<AppActions>>>, receiver: Receiver<AppToRenderMessages>)
    {
        {   //letting the ui know the render process has begun
            let mut mutex_guard = rendering.lock().unwrap();
            *mutex_guard = true;
        }
        let begin_time = Instant::now();
        
        //actual render process in a for loop
        for frame_number in 0..nbr_of_iterations {
            uniforms.frame_id = frame_number;
            let uniforms_ref = Arc::new(uniforms.clone());
            Self::apply_shader2(&mut image_float, uniforms_ref.clone(), &thread_pool);
            
            {   //take the custom image, convert it into a DynamicImage and send it to the main app
                let mut action_list = action_list.lock().unwrap();
                action_list.push(AppActions::FrameUpdate(image_float.clone().into()));
                action_list.push(AppActions::RenderingProgressUpdate((
                    frame_number + 1) as f32 / nbr_of_iterations as f32));
            }

            //check if any messages have been passed back
            if let Ok(message) = receiver.try_recv() {
                match message {
                    AppToRenderMessages::AbortRender => {
                        break;  //simply jump out of loop to stop rendering
                    }
                }
            }
        }

        {   //letting the ui know the render process is finished
            let mut mutex_guard = rendering.lock().unwrap();
            *mutex_guard = false;
        }
        {   //giving the ui the final rendering time in case it cannot compute it on its own
            let mut action_list = action_list.lock().unwrap();
            action_list.push(AppActions::TrueTimeUpdate(Instant::now() - begin_time));

            //telling the app to destroy its render sender
            action_list.push(AppActions::DestroySender);
        }
    }

    /// The function which will dispatch the render process to another thread. Takes all relevant
    /// UI-side values, extracts the information such as the pure spectra necessary for rendering
    /// and passes these on to the next thread.
    fn dispatch_render(&mut self) {
        self.update_all_spectrum_sample_sizes(self.ui_values.spectrum_number_of_samples);
        //TODO more safety checks?
        
        if !self.check_render_legality() {
            error!("The values passed to the renderer are in an illegal state! The renderer will \
                crash! Aborting rendering. Turn to App::check_render_legality to start the \
                debugging process.");
            return;
        }
        
        let thread_pool = ThreadPool::new(self.ui_values.nbr_of_threads);
        
        let example_spectrum = Spectrum::new_singular_reflectance_factor(
            spectrum::VISIBLE_LIGHT_WAVELENGTH_LOWER_BOUND,
            spectrum::VISIBLE_LIGHT_WAVELENGTH_UPPER_BOUND,
            self.ui_values.spectrum_number_of_samples,
            0.0,
        );

        let uniforms = RaytracingUniforms{
            aabbs: Arc::new(self.ui_values.ui_objects.iter().map(|o| o.into()).collect()),
            lights: Arc::new(self.ui_values.ui_lights.iter().map(|l| l.into()).collect()),
            camera: shader::Camera::from(&self.ui_values.ui_camera),
            frame_id: 0,
            intended_frames_amount: self.ui_values.nbr_of_iterations,
            example_spectrum,
        };
        
        //input validation
        let dependent = are_linear_dependent(&uniforms.camera.direction, &uniforms.camera.up);
        if dependent {
            error!("View Direction and Up Direction are linearly dependent! \nDir: {} Up: {}",
                &uniforms.camera.direction, &uniforms.camera.up);
        }
        assert!(!dependent);
        
        let image = custom_image::CustomImage::new(self.ui_values.width, self.ui_values.height);
        let nbr_of_iterations = self.ui_values.nbr_of_iterations;
        let rendering = self.currently_rendering.clone();
        let action_list = self.actions.clone();

        let (sender, receiver) = mpsc::channel::<AppToRenderMessages>();
        self.app_to_render_channel = Some(sender);
        
        self.ui_values.tab = UiTab::Display;
        
        thread::spawn(move || {
            Self::render(image, uniforms, thread_pool, nbr_of_iterations, rendering, action_list, receiver);
        });
    }

    /// Takes the [DynamicImage] in [image_actual](App::image_actual) and generates an egui texture
    /// handle from it. This is necessary to display the image to the user.
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

    /// Checks if all values about to be passed to the renderer are in order. This function should
    /// return false if an error exists which will make the renderer crash. 
    fn check_render_legality(&self) -> bool {
        true
        // let lights_ok = self.check_lights_legality();    //TODO re-enable checks
        // 
        // let objects_ok = self.check_objects_legality();
        // 
        // let ui_sample_nbr = self.ui_values.spectrum_number_of_samples;
        // let spectra_ok = self.ui_values.spectra.iter()
        //     .map(|s| s.borrow().spectrum.get_nbr_of_samples() == ui_sample_nbr)
        //     .all(|b| b);
        // 
        // let not_currently_rendering = !*self.currently_rendering.lock().unwrap();
        // 
        // lights_ok && objects_ok && spectra_ok && not_currently_rendering
    }

    /// Checks if all [UILights](UILight) are in order. Returns false if the rendering process
    /// would fail.
    fn check_lights_legality(&self) -> bool {
        self.ui_values.ui_lights.iter()
            .map(|l| self.ui_values.spectra.contains(&l.spectrum))
            .all(|b| b)
    }
    
    fn check_objects_legality(&self) -> bool {
        self.ui_values.ui_objects.iter()
            .map(|o| self.ui_values.spectra.contains(&o.spectrum))
            .all(|b| b)
    }
}

/// Some threads, started by the UI, may need to write back to the main struct of the application
/// but do not have a reference to it. They can instead submit an AppAction which describes their
/// intent and the necessary data to complete these actions.
enum AppActions {
    /// The rendering thread has completed an image, which can now be written back to the main
    /// struct to be displayed for the user.
    FrameUpdate(DynamicImage),
    
    /// The rendering thread has completed the rendering process and reports back how long it took 
    /// exactly so that the UI may report it even if the ui did not update in a while. 
    TrueTimeUpdate(Duration),
    
    /// The rendering thread has completed a step in rendering the image and now reports the 
    /// current progress amount until it is finished, to be displayed in a progressbar. 
    RenderingProgressUpdate(f32),

    /// The rendering thread has completed and its receiver is destroyed. Consequently, the app's
    /// sender is useless and should be destroyed as well.
    DestroySender,
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
    after_ui_action: Option<AfterUIActions>,
    ui_camera: UICamera,
    ui_lights: Vec<UILight>, 
    ui_objects: Vec<UIObject>,
    progress_bar_progress: f32,
    spectra: Vec<Rc<RefCell<UISpectrum>>>,
    spectrum_lower_bound: f32,
    spectrum_upper_bound: f32,
    spectrum_number_of_samples: usize,
    selected_spectrum: Option<UISelectedSpectrum>,
}
impl Default for UIFields {
    fn default() -> Self {
        let sun10 = Spectrum::new_sunlight_spectrum(
            spectrum::VISIBLE_LIGHT_WAVELENGTH_LOWER_BOUND,
            spectrum::VISIBLE_LIGHT_WAVELENGTH_UPPER_BOUND,
            NBR_OF_SPECTRUM_SAMPLES_DEFAULT,
            0.001,
        );
        let sun10 = UISpectrum::new(
            "Close light spectrum".to_string(),
            UISpectrumType::Solar(0.001),
            SpectrumEffectType::Emissive,
            sun10,
        );
        let sun10 = Rc::from(RefCell::from(sun10));

        let sun1mil = Spectrum::new_sunlight_spectrum(
            spectrum::VISIBLE_LIGHT_WAVELENGTH_LOWER_BOUND,
            spectrum::VISIBLE_LIGHT_WAVELENGTH_UPPER_BOUND,
            NBR_OF_SPECTRUM_SAMPLES_DEFAULT,
            100.0,
        );
        let sun1mil = UISpectrum::new(
            "Far away sun spectrum".to_string(),
            UISpectrumType::Solar(100.0),
            SpectrumEffectType::Emissive,
            sun1mil,
        );
        let sun1mil = Rc::from(RefCell::from(sun1mil));
        let ui_lights = vec![
            UILight::new(0.0, 2.0, -1.0, sun10.clone(), "Close light".to_string()),
            UILight::new(0.0, 1_000.0, 0.0, sun1mil.clone(), "Far away sun light".to_string()),
        ];
        
        let spectrum_grey = Spectrum::new_singular_reflectance_factor(
            spectrum::VISIBLE_LIGHT_WAVELENGTH_LOWER_BOUND,
            spectrum::VISIBLE_LIGHT_WAVELENGTH_UPPER_BOUND,
            NBR_OF_SPECTRUM_SAMPLES_DEFAULT,
            0.7,
        );
        let spectrum_grey = UISpectrum::new(
            "Grey reflecting spectrum".to_string(),
            UISpectrumType::PlainReflective(0.7),
            SpectrumEffectType::Reflective,
            spectrum_grey,
        );
        let spectrum_grey = Rc::new(RefCell::new(spectrum_grey));

        let spectrum_white = Spectrum::new_singular_reflectance_factor(
            spectrum::VISIBLE_LIGHT_WAVELENGTH_LOWER_BOUND,
            spectrum::VISIBLE_LIGHT_WAVELENGTH_UPPER_BOUND,
            NBR_OF_SPECTRUM_SAMPLES_DEFAULT,
            1.0,
        );
        let spectrum_white = UISpectrum::new(
            "White reflecting spectrum".to_string(),
            UISpectrumType::PlainReflective(1.0),
            SpectrumEffectType::Reflective,
            spectrum_white,
        );
        let spectrum_white = Rc::new(RefCell::new(spectrum_white));

        let ui_objects = vec![
            UIObject::new(-1.5, 0.0, 1.0, true, spectrum_white.clone(), UIObjectType::PlainBox(0.25, 3.0, 30.0), "Left mirror".to_string()),
            UIObject::new(0.0, 0.0, 1.0, false, spectrum_grey.clone(), UIObjectType::Sphere(1.0), "Left sphere".to_string()),
            UIObject::new(1.0, 0.0, 1.0, false, spectrum_grey.clone(), UIObjectType::Sphere(1.0), "Right sphere".to_string()),
            UIObject::new(0.0, -1.0, 0.0, false, spectrum_grey.clone(), UIObjectType::PlainBox(50.0, 0.1, 50.0), "Floor".to_string()),
        ];

        let spectra = vec![
            sun10,
            sun1mil,

            spectrum_grey,
            spectrum_white,
        ];
        
        
        Self {
            width: 600,
            height: 400,
            frame_gen_time: None,
            nbr_of_iterations: NBR_OF_ITERATIONS_DEFAULT,
            nbr_of_threads: determine_optimal_thread_count(),
            tab: UiTab::Settings,
            after_ui_action: None,
            ui_camera: UICamera::default(),
            ui_lights,
            ui_objects,
            progress_bar_progress: 0.0,
            spectra,
            spectrum_lower_bound: spectrum::VISIBLE_LIGHT_WAVELENGTH_LOWER_BOUND,
            spectrum_upper_bound: spectrum::VISIBLE_LIGHT_WAVELENGTH_UPPER_BOUND,
            spectrum_number_of_samples: NBR_OF_SPECTRUM_SAMPLES_DEFAULT,
            selected_spectrum: None,
        }
    }
}

struct UISelectedSpectrum {
    pub selected_spectrum: usize,
    pub max: f32,
    pub spectrum_values: Vec<f32>,
    pub spectrum_effect_type: SpectrumEffectType,
    pub lower_bound: f32,
    pub upper_bound: f32,
    pub ui_spectrum_type: UISpectrumType,
}

/// A container for the [Spectrum] datatype. Holds additional information such as a label for 
/// convenience of the user.
#[derive(Clone, Debug)]
struct UISpectrum {
    id: u32,
    name: String,
    editing_name: bool,
    spectrum_type: UISpectrumType,
    spectrum_effect_type: SpectrumEffectType,
    spectrum: Spectrum,
}

impl UISpectrum {
    pub fn new(name: String, spectrum_type: UISpectrumType, spectrum_effect_type: SpectrumEffectType, spectrum: Spectrum) -> Self {
        Self {
            id: get_id(),
            name,
            editing_name: false,
            spectrum_type,
            spectrum_effect_type,
            spectrum,
        }
    }

    /// Updates the UISpectrum. Overwrites the attached spectrum with the changes made by the user.
    pub fn edit(&mut self, update: &UISelectedSpectrum) {
        todo!();
        // let spectrum = Spectrum::new_from_list(&update.spectrum_values, update.lower_bound, update.upper_bound);
        // self.spectrum = spectrum;
    }
}

impl Default for UISpectrum {
    fn default() -> Self {
        Self::new(
            "REPLACE ME".to_string(),
            UISpectrumType::PlainReflective(0.0),
            SpectrumEffectType::Emissive,
            Spectrum::new_singular_reflectance_factor(
                1.0,
                2.0,
                NBR_OF_SPECTRUM_SAMPLES_DEFAULT,
                0.0,
            )
        )
    }
}

impl Display for UISpectrum {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

/// An enum to differentiate between the uses of spectra. Emissive spectra are "true" spectra as in 
/// they portray the composition of light. Reflective spectra are not spectra per se, more are they 
/// tables of percentages for how much a given wavelength is reflected. In the shader however, they 
/// are the same datatype, therefore the UI does not discriminate on a type basis either.  
#[derive(Clone, Copy, Debug, PartialEq)]
enum SpectrumEffectType {
    Emissive,
    Reflective,
}

impl Display for SpectrumEffectType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SpectrumEffectType::Emissive => {
                write!(f, "Emissive")
            }
            SpectrumEffectType::Reflective => {
                write!(f, "Reflective")
            }
        }
    }
}

/// An enum that represents the types of spectra a [UISpectrum] can have. When changing amount of 
/// samples f. ex. each type is handled differently. For custom, each value is linearly interpolated 
/// making the process quiet lossy. For every other type, the appropriate new [Spectrum] function is
/// called and a new spectrum used instead. 
#[derive(Clone, Copy, Debug)]
#[derive(PartialEq)]
enum UISpectrumType {
    Custom,
    Solar(f32),     //parameter = factor
    PlainReflective(f32),   //parameter = factor 0-1
    ///Parameter 0 = temp in Kelvin, parameter 1 = factor
    Temperature(f32, f32),  //parameter 0 = temp in Kelvin, parameter 1 = factor
}

impl Display for UISpectrumType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            UISpectrumType::Custom => write!(f, "Custom"),
            UISpectrumType::Solar(_) => write!(f, "Solar spectrum"),
            UISpectrumType::PlainReflective(_) => write!(f, "All the same"),
            UISpectrumType::Temperature(_, _) => write!(f, "Temperature"),
        }
    }
}

impl From<Spectrum> for UISpectrum {
    fn from(spectrum: Spectrum) -> Self {
        Self {
            id: get_id(),
            name: String::new(),
            editing_name: false,
            spectrum_type: UISpectrumType::Custom,
            spectrum_effect_type: SpectrumEffectType::Emissive,
            spectrum,
        }
    }
}

impl PartialEq for UISpectrum {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

/// This struct is a collection of values which can be assembled to a Light object. Coupled values
/// such as position x, y and z are separated here to allow for easier manipulation by the ui. 
#[derive(Debug)]
struct UILight {
    pos_x: f32,
    pos_y: f32,
    pos_z: f32,
    spectrum: Rc<RefCell<UISpectrum>>,
    name: String,
    editing_name: bool,
}

impl UILight {
    pub fn new(pos_x: f32, pos_y: f32, pos_z: f32, spectrum: Rc<RefCell<UISpectrum>>, name: String) -> Self {
        Self {
            pos_x,
            pos_y,
            pos_z,
            spectrum,
            name,
            editing_name: false,
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
    up_x: f32,
    up_y: f32,
    up_z: f32,
    fov_deg_y: f32,
}

impl Default for UICamera {
    fn default() -> Self {
        Self {
            pos_x: 0.0,
            pos_y: 0.0,
            pos_z: -2.0,
            dir_x: 0.0,
            dir_y: 0.0,
            dir_z: 1.0,
            up_x: 0.0,
            up_y: 1.0,
            up_z: 0.0,
            fov_deg_y: 60.0,
        }
    }
}

/// The UIObject struct represents an object in the scene, bound in an AABB, in its primitive UI
/// form. The UI form allows for easier manipulation through the UI, for rendering it is later
/// assembled into a proper AABB. <br>
/// The struct holds the position as well as more type specific data.
struct UIObject {
    pos_x: f32,
    pos_y: f32,
    pos_z: f32,
    metallicness: bool, 
    spectrum: Rc<RefCell<UISpectrum>>,
    ui_object_type: UIObjectType,
    name: String,
    editing_name: bool,
}

impl UIObject {
    pub fn new(pos_x: f32, pos_y: f32, pos_z: f32, metallicness: bool, spectrum: Rc<RefCell<UISpectrum>>, ui_object_type: UIObjectType, name: String) -> Self {
        Self {
            pos_x,
            pos_y,
            pos_z,
            metallicness, 
            spectrum,
            ui_object_type,
            name,
            editing_name: false,
        }
    }

    pub fn default(app: &App) -> Self {
        let spectrum = match app.ui_values.spectra.first() {
            Some(spec_ref) => {
                spec_ref.clone()
            }
            None => {
                let plain_spectrum = Spectrum::new_singular_reflectance_factor(
                    app.ui_values.spectrum_lower_bound,
                    app.ui_values.spectrum_upper_bound,
                    app.ui_values.spectrum_number_of_samples,
                    0.7);
                Rc::new(RefCell::new(UISpectrum::new(
                    "REPLACE ME".to_string(),
                    UISpectrumType::PlainReflective(0.7),
                    SpectrumEffectType::Reflective,
                    plain_spectrum,
                )))
            }
        };

        Self {
            pos_x: 0.0,
            pos_y: 0.0,
            pos_z: 0.0,
            metallicness: false,
            spectrum,
            ui_object_type: UIObjectType::PlainBox(2.0, 2.0, 2.0),
            name: "New Object".to_string(),
            editing_name: false,
        }
    }
}

impl Display for UIObject {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let s = match self.ui_object_type {
            UIObjectType::PlainBox(_, _, _) => "Plain Box",
            UIObjectType::Sphere(_) => "Sphere",
        };
        write!(f, "{}", s)
    }
}

/// An enum which differentiates the type of the [UIObjects](UIObject). Different types will be 
/// assembled to different geometric shapes in the render process. 
enum UIObjectType {
    PlainBox(f32, f32, f32),
    Sphere(f32),
}

impl UIObjectType {
    fn default_plain_box() -> Self {
        UIObjectType::PlainBox(2.0, 2.0, 2.0)
    }
    
    fn default_sphere() -> Self {
        UIObjectType::Sphere(1.0)
    }
}

/// This enum differentiates which tab is currently displayed in the apps main content window.
#[derive(Debug, Clone, Copy, PartialEq)]
enum UiTab {
    Settings,   //pre render settings such as width, height or number of frames
    Objects,    //3D models and lights defined in the scene
    SpectraAndMaterials,    //reflectance and light spectra as well as object materials defined here
    Display,    //the screen ultimately displaying the result 
}

/// This enum describes a number of actions which have to be taken after the UI is displayed such 
/// as deleting objects. 
enum AfterUIActions {
    DeleteLight(usize),
    DeleteObject(usize),
    SaveSelectedSpectrum(usize),
    DeleteSpectrum(usize),
    UpdateSelectedSpectrum(usize),
    DeselectSelectedSpectrum,
}

/// An enum to send messages from the UI thread over to the currently rendering thread.
enum AppToRenderMessages {
    AbortRender,
}

/// Takes 2 3-dimensional vectors and checks if they are linearly dependent (point in the same
/// direction). This is done by checking if the cross product is (very close to) the 0 vector.
fn are_linear_dependent(vec1: &Vector3<f32>, vec2: &Vector3<f32>) -> bool {
    let cross = vec1.cross(vec2);
    cross.x.abs() < shader::F32_DELTA && cross.y.abs() < shader::F32_DELTA && cross.z.abs() < shader::F32_DELTA
}

/// Queries rusts API to determine the optimal amount of parallel threads used for computations 
/// ([thread::available_parallelism]). Should this call fail [a default](NBR_OF_THREADS_DEFAULT) is 
/// returned instead and the error is logged. 
fn determine_optimal_thread_count() -> usize {
    match thread::available_parallelism() {
        Ok(num) => { 
            num.into() 
        }
        Err(error) => {
            error!("An error occurred while trying to determine the optimal amount of used virtual cores! Using sub-optimal default instead.");
            error!("{}", error);
            NBR_OF_THREADS_DEFAULT
        }
    }
}

/// Generates a singular horizontal ui line, inserts the label "Brightness factor" and puts the
/// supplied factor into a text edit field with the only check being: factor > 0.0.
/// Returns true iff the value has been changed.
fn display_factor(ui: &mut Ui, factor: &mut f32) -> bool {
    let mut changed = false;
    ui.horizontal_top(|ui| {
        let mut factor_string = factor.to_string();

        ui.label("Brightness factor:");
        ui.add_sized([80.0, 18.0], TextEdit::singleline(&mut factor_string));

        if factor_string.parse::<f32>().is_ok() {
            let new_factor = factor_string.parse::<f32>().unwrap();
            if new_factor != *factor && new_factor > 0.0 {
                *factor = new_factor;
                changed = true;
            }
        }
    });
    changed
}

/// Displays a button with a pencil emoji as label to indicate that something can be edited. 
fn display_edit_name_button(ui: &mut Ui, changing_value: &mut bool) {
    if ui.button(EDIT_BUTTON_PENCIL_EMOJI).on_hover_text(EDIT_BUTTON_TOOLTIP).clicked() {
        *changing_value = !*changing_value;
    }
}

/// Displays the name of an object in a label or in a text field, depending on the value in editing. 
/// Additionally, displays an edit button to toggle between the two states. 
fn display_name_with_edit(ui: &mut Ui, name: &mut String, backup: &String, editing: &mut bool) {
    if *editing {
        if ui.text_edit_singleline(name).lost_focus() {
            *editing = false;
        }
    } else {
        let label_content = if name.is_empty() {
            backup
        } else {
            name
        };
        ui.label(label_content);
    }
    display_edit_name_button(ui, editing);
}

/// Returns true for one second, false for the next, then true again, etc. 
fn is_time_even() -> bool {
    std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() % 2 == 0
}

//TODO undo redo stack for actions such as creating new elements or deleting old ones
//TODO the entire UI could use an overhaul
//TODO new dedicated render button, for the second, etc render calls
//TODO way to disable an object without actually deleting it
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
                            .add_filter("PNG", &["png"])
                            .add_filter("JPG", &["jpg"])
                            .add_filter("BMP", &["bmp"])
                            .add_filter("TIFF", &["tiff"])
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
                    self.display_start_render_button(ui);
                    if ui.button("Reset Settings to default").clicked() {
                        self.ui_values = UIFields::default();
                    }
                });
            });
        });
        
        //main content div. 
        egui::CentralPanel::default().show(ctx, |ui| {
            //tab "buttons"
            ui.vertical_centered(|ui| {
                ui.horizontal_top(|ui| {
                    let old_spacing = ui.style().spacing.clone();
                    ui.style_mut().spacing.item_spacing.x = 0.0;
                    ui.style_mut().spacing.item_spacing.y = 0.0;

                    //settings
                    let color = if self.ui_values.tab == UiTab::Settings {Color32::LIGHT_BLUE} else {Color32::LIGHT_GRAY};
                    self.display_tab_frame(ui, "Settings", color, UiTab::Settings);

                    //objects
                    let mut color = if self.ui_values.tab == UiTab::Objects {Color32::LIGHT_BLUE} else {Color32::LIGHT_GRAY};
                    if !(self.check_lights_legality() && self.check_objects_legality()) && is_time_even() {
                        color = Color32::LIGHT_RED;
                    }
                    self.display_tab_frame(ui, "Objects", color, UiTab::Objects);

                    //spectra and materials
                    let color = if self.ui_values.tab == UiTab::SpectraAndMaterials {Color32::LIGHT_BLUE} else {Color32::LIGHT_GRAY};
                    self.display_tab_frame(ui, "Spectra and Materials", color, UiTab::SpectraAndMaterials);

                    //display
                    let color = if self.ui_values.tab == UiTab::Display {Color32::LIGHT_BLUE} else {Color32::LIGHT_GRAY};
                    self.display_tab_frame(ui, "Display", color, UiTab::Display);

                    ui.style_mut().spacing = old_spacing;
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
                        //camera settings
                        ui.label("Camera:");
                        egui::Frame::NONE.fill(Color32::LIGHT_GRAY).inner_margin(5.0).show(ui, |ui| {
                            self.display_camera_settings(ui);
                        });
                        ui.add_space(10.0);
                        
                        //Light sources management
                        ui.vertical_centered(|ui| {
                            ui.horizontal_top(|ui| {
                                ui.label("Light Sources:");
                                ui.add_space(100.0);
                                if ui.button("Add New Light Source").clicked() {
                                    let spectrum = match self.ui_values.spectra.first() {
                                        Some(spectrum) => spectrum.clone(),
                                        None => {Rc::new(RefCell::new(UISpectrum::default()))}
                                    };
                                    let light = UILight::new(0.0, 0.0, 0.0, spectrum, "New Light Source".to_string());
                                    self.ui_values.ui_lights.push(light);
                                }
                            });
                        });
                        for index in 0..self.ui_values.ui_lights.len() {
                            egui::Frame::NONE.fill(Color32::LIGHT_GRAY).inner_margin(5.0).show(ui, |ui| {
                                self.display_light_source_settings(ui, index);
                            });
                        }
                        ui.add_space(10.0);
                        
                        //Objects management
                        ui.vertical_centered(|ui| {
                            ui.horizontal_top(|ui| {
                                ui.label("Objects:");
                                ui.add_space(100.0);
                                if ui.button("Add New Object").clicked() {
                                    let object = UIObject::default(self);
                                    self.ui_values.ui_objects.push(object);
                                }
                            });
                        });
                        for index in 0..self.ui_values.ui_objects.len() {
                            egui::Frame::NONE.fill(Color32::LIGHT_GRAY).inner_margin(5.0).show(ui, |ui| {
                                self.display_objects_settings(ui, index);   //TODO ui setting for reflectivity
                            });
                        }
                    });
                }
                UiTab::SpectraAndMaterials => {
                    ui.horizontal_top(|ui| {
                        //left
                        ui.vertical(|ui| {
                            egui::ScrollArea::vertical().show(ui, |ui| {

                                ui.label("General Spectrum Settings:");
                                egui::Frame::NONE.fill(Color32::LIGHT_GRAY).inner_margin(5.0).show(ui, |ui| {
                                    self.display_general_spectrum_settings(ui);
                                });
                                ui.add_space(10.0);

                                //name and add button
                                ui.horizontal_top(|ui| {
                                    ui.label("Spectra:");
                                    ui.add_space(100.0);
                                    if ui.button("Add new Spectrum").clicked() {
                                        let spectrum = UISpectrum::new(
                                            "New Spectrum".to_string(),
                                            UISpectrumType::Solar(0.001),
                                            SpectrumEffectType::Emissive,
                                            Spectrum::new_sunlight_spectrum(
                                                self.ui_values.spectrum_lower_bound,
                                                self.ui_values.spectrum_upper_bound,
                                                self.ui_values.spectrum_number_of_samples,
                                                0.001,
                                            )
                                        );
                                        self.ui_values.spectra.push(
                                            Rc::new(RefCell::new(spectrum))
                                        );
                                    }
                                });

                                //individual spectra
                                for index in 0..self.ui_values.spectra.len() {
                                    //determine color
                                    let mut color = Color32::LIGHT_GRAY;
                                    if let Some(selected_index) = &mut self.ui_values.selected_spectrum {
                                        let selected_index = selected_index.selected_spectrum;
                                        if selected_index == index {
                                            color = Color32::LIGHT_BLUE;
                                        }
                                    }

                                    //add actual spectrum UI elements
                                    if ui.scope_builder(UiBuilder::new().sense(Sense::click()), |ui| {
                                        egui::Frame::NONE.fill(color).inner_margin(5.0).show(ui, |ui| {
                                            self.display_spectrum_settings(ui, index);
                                        });
                                    }).response.clicked()  {
                                        self.update_selected_spectrum(index);
                                    };
                                }
                                ui.add_space(10.0);
                                //TODO material settings
                            });
                        });

                        //divider
                        ui.separator();

                        //right side
                        ui.vertical(|ui| {
                            self.display_spectrum_right_side(ui);
                        });
                    });
                }
                UiTab::Display => {
                    //user information about rendering time
                    ui.horizontal_top(|ui| {
                        self.display_start_render_button(ui);
                        self.display_abort_button(ui);
                        self.refresh_rendering_time();
                        self.display_frame_generation_time(ui);
                        egui::Frame::NONE.inner_margin(5.0).show(ui, |ui| {
                            ui.add(egui::ProgressBar::new(self.ui_values.progress_bar_progress));
                        });
                    });

                    //image display frame
                    egui::Frame::NONE.fill(Color32::GRAY).show(ui, |ui| {
                        if let Some(ref img) = self.image_eframe_texture {
                            egui::ScrollArea::both().show(ui, |ui| {
                                ui.add(
                                    egui::Image::from_texture(img).fit_to_original_size(1.0)
                                );
                            });
                        } else {
                            ui.centered_and_justified(|ui| {
                                self.display_start_render_button(ui);
                            });
                        }
                    });
                }
            }
        });

        /////////////////////////////////// UI IS DONE BY HERE /////////////////////////////////////

        //ui is finished drawing, but some actions have to be done after this point such as deleting
        //elements with a button press. 
        if self.ui_values.after_ui_action.is_some() {
            match self.ui_values.after_ui_action.take().unwrap() {
                AfterUIActions::DeleteLight(index) => {
                    self.ui_values.ui_lights.remove(index);
                }
                AfterUIActions::DeleteObject(index) => {
                    self.ui_values.ui_objects.remove(index);
                }
                AfterUIActions::SaveSelectedSpectrum(index) => {
                    let selected = self.ui_values.selected_spectrum.take().unwrap();
                    self.ui_values.spectra[index].borrow_mut().edit(&selected);
                }
                AfterUIActions::DeleteSpectrum(index) => {
                    self.ui_values.spectra.remove(index);
                }
                AfterUIActions::UpdateSelectedSpectrum(index) => {
                    self.update_selected_spectrum(index);
                }
                AfterUIActions::DeselectSelectedSpectrum => {
                    self.ui_values.selected_spectrum = None;
                }
            }
        }
        
        //Other frames may have finished work
        let mut actions_list = self.actions.lock().unwrap();
        let mut separate_action_list = Vec::new();
        while actions_list.len() > 0 {
            separate_action_list.push(actions_list.remove(0));
        }
        drop(actions_list);
        
        for action in separate_action_list {
            match action {
                AppActions::FrameUpdate(image) => {
                    self.image_actual = Some(image);
                    self.renew_texture_handle(ctx);
                }
                AppActions::TrueTimeUpdate(duration) => {
                    self.ui_values.frame_gen_time = Some(duration);
                }
                AppActions::RenderingProgressUpdate(progress) => {
                    self.ui_values.progress_bar_progress = progress;
                }
                AppActions::DestroySender => {
                    self.app_to_render_channel = None;
                }
            }
        }

        //assert that at least once every second a frame is drawn
        //a request repaint call is cleared as soon as a frame is drawn, meaning this line does 
        // nothing as long as one continues moving their mouse
        ctx.request_repaint_after_secs(1.0);
    }
}
