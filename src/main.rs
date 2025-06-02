mod shader;
mod custom_image;
mod spectrum;
mod spectral_data;
mod text_ressources;

use std::cell::RefCell;
use std::fmt::{Display, Formatter};
use std::rc::Rc;
use std::sync::{mpsc, Arc, Mutex};
use std::sync::atomic::AtomicU32;
use std::thread;
use std::time::{Duration, Instant};
use eframe::egui;
use eframe::egui::{menu, Color32, CornerRadius, IconData, Sense, TopBottomPanel, Ui, UiBuilder};
use eframe::epaint::Vec2;
use image::{DynamicImage, ImageBuffer};
use log::{error, info, warn};
use nalgebra::Vector3;
use threadpool::ThreadPool;
use crate::shader::{PixelPos, RaytracingUniforms};
use crate::spectrum::Spectrum;
use crate::text_ressources::*;

const NBR_OF_THREADS_DEFAULT: usize = 20;
const NBR_OF_THREADS_MAX: usize = 64;
const NBR_OF_ITERATIONS_DEFAULT: u32 = 128;
const NBR_OF_SPECTRUM_SAMPLES_DEFAULT: usize = 64;  //TODO replace by ui selectable value

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
    image_float: Option<custom_image::CustomImage>,
    image_actual: Option<DynamicImage>,
    image_eframe_texture: Option<egui::TextureHandle>,
    actions: Arc<Mutex<Vec<AppActions>>>,
    currently_rendering: Arc<Mutex<bool>>,
    rendering_since: Option<Instant>,
}
impl App {
    fn new() -> Self {
        Self {
            ui_values: UIFields::default(),
            image_float: None,
            image_actual: None,
            image_eframe_texture: None,
            actions: Arc::new(Mutex::new(Vec::new())),
            currently_rendering: Arc::new(Mutex::new(false)),
            rendering_since: None,
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
            ui.add_sized([80.0, 18.0], egui::TextEdit::singleline(&mut pos_x_string));
            ui.label("y:");
            ui.add_sized([80.0, 18.0], egui::TextEdit::singleline(&mut pos_y_string));
            ui.label("z:");
            ui.add_sized([80.0, 18.0], egui::TextEdit::singleline(&mut pos_z_string));
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
            ui.add_sized([80.0, 18.0], egui::TextEdit::singleline(&mut dir_x_string));
            ui.label("y:");
            ui.add_sized([80.0, 18.0], egui::TextEdit::singleline(&mut dir_y_string));
            ui.label("z:");
            ui.add_sized([80.0, 18.0], egui::TextEdit::singleline(&mut dir_z_string));
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
            ui.add_sized([80.0, 18.0], egui::TextEdit::singleline(&mut up_x_string));
            ui.label("y:");
            ui.add_sized([80.0, 18.0], egui::TextEdit::singleline(&mut up_y_string));
            ui.label("z:");
            ui.add_sized([80.0, 18.0], egui::TextEdit::singleline(&mut up_z_string));
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
                self.ui_values.after_ui_action = Some(AfterUIActions::DeleteLight(index));
                info!("Light Source #{} has been scheduled for deletion.", index);
            }
        });
        
        //light position
        ui.horizontal_top(|ui| {
            let mut pos_x_string = light.pos_x.to_string();
            let mut pos_y_string = light.pos_y.to_string();
            let mut pos_z_string = light.pos_z.to_string();
            ui.label("Light Position: (x:").on_hover_text(LIGHT_SOURCE_TOOLTIP);
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
        // ui.horizontal_top(|ui| { //TODO reimplement with spectrum selector
        //     let mut intensity_string = light.intensity.to_string();
        //     ui.label("Light Intensity: ");
        //     ui.add_sized([80.0, 18.0], egui::TextEdit::singleline(&mut intensity_string));
        // 
        //     if intensity_string.parse::<f32>().is_ok() {
        //         let input = intensity_string.parse::<f32>().unwrap();
        //         if input >= 0.0 {
        //             light.intensity = input;
        //         }
        //     }
        // });
    }
    
    /// Shortcut function to display the settings for a single Object in the scene. The settings 
    /// can be changed and the updated values will be used in the rendering process. Each object is 
    /// differentiated according to their type and the respective settings will be displayed. 
    fn display_objects_settings(&mut self, ui: &mut Ui, index: usize) {
        let object = &mut self.ui_values.ui_objects[index];

        //name
        ui.horizontal_top(|ui| {
            let name = format!("{} #{}", object, index);
            ui.label(name);
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
            egui::ComboBox::new(index, "Type")
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

            let delete_button = egui::widgets::Button::new("Delete this object").fill(egui::Color32::LIGHT_RED);
            if ui.add(delete_button).clicked() {
                self.ui_values.after_ui_action = Some(AfterUIActions::DeleteObject(index));
                info!("Object #{} has been scheduled for deletion.", index);
            }
        });
        
        //object position
        ui.horizontal_top(|ui| {
            let mut pos_x_string = object.pos_x.to_string();
            let mut pos_y_string = object.pos_y.to_string();
            let mut pos_z_string = object.pos_z.to_string();
            ui.label("Object Position: (x:").on_hover_text(OBJECT_POSITION_TOOLTIP);
            ui.add_sized([80.0, 18.0], egui::TextEdit::singleline(&mut pos_x_string));
            ui.label("y:");
            ui.add_sized([80.0, 18.0], egui::TextEdit::singleline(&mut pos_y_string));
            ui.label("z:");
            ui.add_sized([80.0, 18.0], egui::TextEdit::singleline(&mut pos_z_string));
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
                    ui.add_sized([80.0, 18.0], egui::TextEdit::singleline(&mut dim_x_string));
                    ui.label("y:");
                    ui.add_sized([80.0, 18.0], egui::TextEdit::singleline(&mut dim_y_string));
                    ui.label("z:");
                    ui.add_sized([80.0, 18.0], egui::TextEdit::singleline(&mut dim_z_string));
                    ui.label(")");

                    if dim_x_string.parse::<f32>().is_ok() {
                        let new_length_x = dim_x_string.parse::<f32>().unwrap();
                        if new_length_x >= 0.0 && new_length_x != x_length {
                            object.ui_object_type = UIObjectType::PlainBox(new_length_x, y_length, z_length);
                        }
                    }
                    if dim_y_string.parse::<f32>().is_ok() {
                        let new_length_y = dim_y_string.parse::<f32>().unwrap();
                        if new_length_y >= 0.0 && new_length_y != y_length {
                            object.ui_object_type = UIObjectType::PlainBox(x_length, new_length_y, z_length);
                        }
                    }
                    if dim_z_string.parse::<f32>().is_ok() {
                        let new_length_z = dim_z_string.parse::<f32>().unwrap();
                        if new_length_z >= 0.0 && new_length_z != z_length {
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
                    ui.add_sized([80.0, 18.0], egui::TextEdit::singleline(&mut radius_string));
                    
                    if radius_string.parse::<f32>().is_ok() {
                        let new_radius = radius_string.parse::<f32>().unwrap();
                        if new_radius >= 0.0 {
                            object.ui_object_type = UIObjectType::Sphere(new_radius);
                        }
                    }
                });
            }
        }
    }

    fn display_general_spectrum_settings(&mut self, ui: &mut Ui) {
        //nbr of samples
        ui.horizontal_top(|ui| {
            let nbr_of_samples = &mut self.ui_values.spectrum_number_of_samples;
            let mut nbr_of_samples_string = nbr_of_samples.to_string();
            let mut final_nbr_of_samples = *nbr_of_samples;

            ui.label("Number of samples in the spectra:").on_hover_text(SPECTRUM_NUMBER_OF_SAMPLES_TOOLTIP);
            ui.add_sized([80.0, 18.0], egui::TextEdit::singleline(&mut nbr_of_samples_string));

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
                    final_nbr_of_samples = (*nbr_of_samples / 8 * 8).max(2)  //drop down to nearest multiple of 8, at least 2
                }
            }

            if ui.button("+").clicked() {
                if *nbr_of_samples % 8 == 0 {
                    final_nbr_of_samples += 8;   //add 8
                } else {
                    final_nbr_of_samples = (*nbr_of_samples / 8 + 1) * 8;    //go up to nearest multiple of 8
                }
            }

            if final_nbr_of_samples != *nbr_of_samples {
                self.update_all_spectrum_sample_sizes(final_nbr_of_samples)
            }
        });

        //range
        ui.horizontal_top(|ui| {    //TODO implement non direct change
            let lower_bound = &mut self.ui_values.spectrum_lower_bound;
            let upper_bound = &mut self.ui_values.spectrum_upper_bound;
            let mut lower_bound_string = lower_bound.to_string();
            let mut upper_bound_string = upper_bound.to_string();

            ui.label("Spectrum range from:").on_hover_text(SPECTRUM_RANGE_TOOLTIP);
            ui.add_sized([80.0, 18.0], egui::TextEdit::singleline(&mut lower_bound_string));
            ui.label("nm to:");
            ui.add_sized([80.0, 18.0], egui::TextEdit::singleline(&mut upper_bound_string));
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

    fn display_spectrum_settings(&mut self, ui: &mut Ui, index: usize) {
        let ui_spectrum = &mut self.ui_values.spectra[index];
        let mut ui_spectrum = ui_spectrum.borrow_mut();
        
        //name and delete button
        ui.horizontal_top(|ui| {
            //TODO enable name changing
            let name = if ui_spectrum.name.is_empty() {
                format!("Spectrum {}", index)
            } else {
                ui_spectrum.name.clone()
            };
            
            ui.label(name);
            
            ui.add_space(80.0);

            let delete_button = egui::widgets::Button::new("Delete this Spectrum").fill(egui::Color32::LIGHT_RED);
            if ui.add(delete_button).clicked() {
                //TODO enable deleting spectra
                warn!("User wants to delete Spectrum {index}, but deletion of spectra is not yet supported!");
            }
        });
        
        let spectrum = &mut ui_spectrum.spectrum;
        
        //TODO Spectrum settings maybe in second half of screen?
        //TODO make Spectrum type which will only resolve at the end and custom type
    }

    fn display_spectrum_right_side(&mut self, ui: &mut Ui) {
        match self.ui_values.selected_spectrum.as_mut() {
            Some(selected) => {
                let spectrum = Spectrum::new_from_list(&selected.spectrum_values, selected.lower_bound, selected.upper_bound);
                let (r, g, b) = spectrum.to_rgb_early();

                match selected.spectrum_effect_type {
                    SpectrumEffectType::Emissive => {
                        //color squares
                        ui.horizontal_top(|ui| {
                            //observed color
                            ui.vertical(|ui| {
                                let r_byte = (r.clamp(0.0, 1.0) * 255.0) as u8;
                                let g_byte = (g.clamp(0.0, 1.0) * 255.0) as u8;
                                let b_byte = (b.clamp(0.0, 1.0) * 255.0) as u8;
                                let luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b;
                                let contrasting_text_color = if luminance < 0.5 { Color32::WHITE } else {Color32::BLACK};

                                egui::Frame::NONE.fill(Color32::from_rgb(r_byte, g_byte, b_byte))
                                    .stroke(egui::Stroke::new(1.0, Color32::LIGHT_GRAY))
                                    .show(ui, |ui| {
                                        ui.set_max_size(Vec2::new(200.0, 100.0));
                                        ui.centered_and_justified(|ui| {
                                            ui.colored_label(contrasting_text_color, format!("{r_byte:02X}{g_byte:02X}{b_byte:02X}"));
                                        });
                                    }).response.on_hover_text(OBSERVED_COLOR_TOOLTIP);
                                ui.label("Observed Color").on_hover_text(OBSERVED_COLOR_TOOLTIP);
                            });

                            //normalized color
                            ui.vertical(|ui| {
                                let max = r.max(g.max(b));
                                let r_byte= (r / max * 255.0 + 0.5) as u8;
                                let g_byte= (g / max * 255.0 + 0.5) as u8;
                                let b_byte= (b / max * 255.0 + 0.5) as u8;

                                egui::Frame::NONE.fill(Color32::from_rgb(r_byte, g_byte, b_byte))
                                    .stroke(egui::Stroke::new(1.0, Color32::LIGHT_GRAY))
                                    .show(ui, |ui| {
                                        ui.set_max_size(Vec2::new(200.0, 100.0));
                                        ui.centered_and_justified(|ui| {
                                            ui.label(format!("{r_byte:02X}{g_byte:02X}{b_byte:02X}"));
                                        });
                                    }).response.on_hover_text(NORMALIZED_COLOR_TOOLTIP);
                                ui.label("Normalized Color").on_hover_text(NORMALIZED_COLOR_TOOLTIP);
                            });
                        });
                    }
                    SpectrumEffectType::Reflective => {
                        ui.label("Color Preview not available for reflective spectra.");
                    }
                }

                let editable = selected.is_custom;

                //samples
                egui::ScrollArea::vertical().id_salt("right scroll area").show(ui, |ui| {
                    for ((wavelength, _), spectral_radiance) in spectrum.iter().zip(selected.spectrum_values.iter_mut()) {
                        //TODO make multiple sliders adjustable
                        ui.horizontal_top(|ui| {
                            ui.label(format!("{wavelength:.2}nm:"));
                            ui.style_mut().spacing.slider_width = 300.0;
                            ui.add_enabled(
                                editable, 
                                egui::Slider::new(&mut *spectral_radiance, 0.0..=(selected.max * 2.0))
                                    .fixed_decimals(3)
                                    .step_by(0.001)
                            );
                        });
                    }
                });
            }
            None => {
                ui.label("Select a spectrum on the left to start editing...");
            }
        }

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

    fn update_all_spectrum_sample_sizes(&mut self, nbr_of_samples: usize) {
        todo!()
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
    
    /// Generates a new blank [CustomImage](custom_image::CustomImage) 
    /// and moves it to the main app. 
    fn generate_image_float(&mut self) {
        let width = self.ui_values.width;
        let height = self.ui_values.height;
        
        let img = custom_image::CustomImage::new(width, height);
        self.image_float = Some(img);
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
    
    fn render(mut image_float: custom_image::CustomImage, mut uniforms: RaytracingUniforms,
              thread_pool: ThreadPool, nbr_of_iterations: u32, rendering:  Arc<Mutex<bool>>,
              action_list: Arc<Mutex<Vec<AppActions>>>) 
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
        }

        {   //letting the ui know the render process is finished
            let mut mutex_guard = rendering.lock().unwrap();
            *mutex_guard = false;
        }
        {   //giving the ui the final rendering time in case it cannot compute it on its own
            let mut action_list = action_list.lock().unwrap();
            action_list.push(AppActions::TrueTimeUpdate(Instant::now() - begin_time));
        }
    }
    
    fn dispatch_render(&mut self) {
        let thread_pool = ThreadPool::new(self.ui_values.nbr_of_threads);
        
        let example_spectrum = Spectrum::new_singular_reflectance_factor(
            spectrum::VISIBLE_LIGHT_WAVELENGTH_LOWER_BOUND,
            spectrum::VISIBLE_LIGHT_WAVELENGTH_UPPER_BOUND,
            NBR_OF_SPECTRUM_SAMPLES_DEFAULT,
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
        
        thread::spawn(move || {
            Self::render(image, uniforms, thread_pool, nbr_of_iterations, rendering, action_list);
        });
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
    spectrum_lower_bound: f32,  //TODO change implementation
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
        let sun1mil = Spectrum::new_sunlight_spectrum(
            spectrum::VISIBLE_LIGHT_WAVELENGTH_LOWER_BOUND,
            spectrum::VISIBLE_LIGHT_WAVELENGTH_UPPER_BOUND,
            NBR_OF_SPECTRUM_SAMPLES_DEFAULT,
            100.0,
        );
        let ui_lights = vec![
            UILight::new(0.0, 2.0, -1.0, sun10.clone()),
            UILight::new(0.0, 1_000.0, 0.0, sun1mil.clone()),
        ];
        
        let spectrum_grey = Spectrum::new_singular_reflectance_factor(
            spectrum::VISIBLE_LIGHT_WAVELENGTH_LOWER_BOUND,
            spectrum::VISIBLE_LIGHT_WAVELENGTH_UPPER_BOUND,
            NBR_OF_SPECTRUM_SAMPLES_DEFAULT,
            0.7,
        );
        let spectrum_white = Spectrum::new_singular_reflectance_factor(
            spectrum::VISIBLE_LIGHT_WAVELENGTH_LOWER_BOUND,
            spectrum::VISIBLE_LIGHT_WAVELENGTH_UPPER_BOUND,
            NBR_OF_SPECTRUM_SAMPLES_DEFAULT,
            1.0,
        );
        let ui_objects = vec![
            UIObject::new(-1.5, 0.0, 1.0, true, spectrum_white.clone(), UIObjectType::PlainBox(0.25, 3.0, 3.0)),
            UIObject::new(0.0, 0.0, 1.0, false, spectrum_grey.clone(), UIObjectType::Sphere(1.0)),
            UIObject::new(1.0, 0.0, 1.0, false, spectrum_grey.clone(), UIObjectType::Sphere(1.0)),
            UIObject::new(0.0, -1.0, 0.0, false, spectrum_grey.clone(), UIObjectType::PlainBox(50.0, 0.1, 50.0)),
        ];

        let spectra = vec![
            Rc::from(RefCell::from(UISpectrum::from(sun10))),
            Rc::from(RefCell::from(UISpectrum::from(sun1mil))),

            Rc::from(RefCell::from(UISpectrum::from(spectrum_white))),
            Rc::from(RefCell::from(UISpectrum::from(spectrum_grey))),
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
    pub is_custom: bool,
}

/// A container for the [Spectrum] datatype. Holds additional information such as a label for 
/// convenience of the user.
#[derive(Clone, Debug)]
struct UISpectrum {
    id: u32,
    name: String,
    spectrum_type: UISpectrumType,
    spectrum_effect_type: SpectrumEffectType,
    spectrum: Spectrum,
}

#[derive(Clone, Copy, Debug)]
enum SpectrumEffectType {
    Emissive,
    Reflective,
}

#[derive(Clone, Copy, Debug)]
enum UISpectrumType {
    Custom,
    Solar(f32),     //parameter = factor
    PlainReflective(f32),   //parameter = factor 0-1
    Temperature(f32, f32),  //parameter 0 = temp in Kelvin, parameter 1 = factor
}

impl From<Spectrum> for UISpectrum {
    fn from(spectrum: Spectrum) -> Self {
        Self {
            id: get_id(),
            name: String::new(),
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
    spectrum: Spectrum,
}

impl UILight {
    pub fn new(pos_x: f32, pos_y: f32, pos_z: f32, spectrum: Spectrum) -> Self {
        Self {
            pos_x,
            pos_y,
            pos_z,
            spectrum,
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
    spectrum: Spectrum,
    ui_object_type: UIObjectType,
}

impl UIObject {
    pub fn new(pos_x: f32, pos_y: f32, pos_z: f32, metallicness: bool, spectrum: Spectrum, ui_object_type: UIObjectType) -> Self {
        Self {
            pos_x,
            pos_y,
            pos_z,
            metallicness, 
            spectrum,
            ui_object_type,
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

impl Default for UIObject {
    fn default() -> Self {
        Self {
            pos_x: 0.0, 
            pos_y: 0.0, 
            pos_z: 0.0,
            metallicness: false,
            spectrum: Spectrum::new_singular_reflectance_factor(
                spectrum::VISIBLE_LIGHT_WAVELENGTH_LOWER_BOUND, 
                spectrum::VISIBLE_LIGHT_WAVELENGTH_UPPER_BOUND, 
                32, 0.7),
            ui_object_type: UIObjectType::PlainBox(2.0, 2.0, 2.0),
        }
    }
}

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

//TODO undo redo stack for actions such as creating new elements or deleting old ones
//TODO the entire UI could use an overhaul
//TODO maybe give UIObjects a string field to be able to name them? (ie. object such as "wall" or "floor")
//TODO maybe start a parallel thread which calls a frame update every second when rendering 
//TODO disable start rendering button when already rendering
impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) { //UI is defined here
        //Top Menu bar (File, Edit, ...)
        TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.add_enabled(self.image_actual.is_some(), 
                                      egui::Button::new("Save Image"))
                        .clicked() {
                        
                        let dialog = rfd::FileDialog::new() //TODO make it save as certain datatypes only, currently "image" without datatype is valid
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
                    // if ui.button("Generate new blank Image").clicked() {
                    //     self.generate_image_actual(ctx);
                    //     self.generate_image_float();
                    // }
                    if ui.button("Start Rendering").clicked() {
                        self.dispatch_render();
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
                ui.horizontal_top(|ui| {    //TODO these buttons might be replaceable by frames with zero outer margin //frames are not clickable, check workaround
                    if ui.button("Settings").clicked() {
                        self.ui_values.tab = UiTab::Settings;
                    }
                    if ui.button("Objects").clicked() {
                        self.ui_values.tab = UiTab::Objects;
                    }
                    if ui.button("Spectra and Materials").clicked() {
                        self.ui_values.tab = UiTab::SpectraAndMaterials;
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
                        //camera settings
                        ui.label("Camera:");
                        egui::Frame::NONE.fill(egui::Color32::LIGHT_GRAY).inner_margin(5.0).show(ui, |ui| {
                            self.display_camera_settings(ui);
                        });
                        ui.add_space(10.0);
                        
                        //Light sources management
                        ui.vertical_centered(|ui| {
                            ui.horizontal_top(|ui| {
                                ui.label("Light Sources:");
                                ui.add_space(100.0);
                                if ui.button("Add New Light Source").clicked() {    //TODO reimplement
                                    todo!()
                                    // let light = UILight::new(0.0, 0.0, 0.0, 1.0);
                                    // self.ui_values.ui_lights.push(light);
                                }
                            });
                        });
                        for index in 0..self.ui_values.ui_lights.len() {
                            egui::Frame::NONE.fill(egui::Color32::LIGHT_GRAY).inner_margin(5.0).show(ui, |ui| {
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
                                    let object = UIObject::default();
                                    self.ui_values.ui_objects.push(object);
                                }
                            });
                        });
                        for index in 0..self.ui_values.ui_objects.len() {
                            egui::Frame::NONE.fill(egui::Color32::LIGHT_GRAY).inner_margin(5.0).show(ui, |ui| {
                                self.display_objects_settings(ui, index);   //TODO ui setting for reflectivity
                            });
                        }
                    });
                }
                UiTab::SpectraAndMaterials => {
                    //TODO remove
                    ui.vertical_centered_justified(|ui| {
                        ui.label("ATTENTION! THIS PAGE IS NOT YET FUNCTIONAL!");
                    });

                    ui.horizontal_top(|ui| {
                        //left
                        ui.vertical(|ui| {
                            egui::ScrollArea::vertical().show(ui, |ui| {

                                ui.label("General Spectrum Settings:");
                                egui::Frame::NONE.fill(egui::Color32::LIGHT_GRAY).inner_margin(5.0).show(ui, |ui| {
                                    self.display_general_spectrum_settings(ui);
                                });
                                ui.add_space(10.0);

                                ui.label("Spectra:");
                                for index in 0..self.ui_values.spectra.len() {
                                    let mut color = Color32::LIGHT_GRAY;
                                    if let Some(selected_index) = &mut self.ui_values.selected_spectrum {
                                        let selected_index = selected_index.selected_spectrum;
                                        if selected_index == index {
                                            color = Color32::LIGHT_BLUE;
                                        }
                                    }

                                    if ui.scope_builder(UiBuilder::new().sense(Sense::click()), |ui| {
                                        egui::Frame::NONE.fill(color).inner_margin(5.0).show(ui, |ui| {
                                            self.display_spectrum_settings(ui, index);
                                        });
                                    }).response.clicked()  {
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
                                            is_custom: matches!(ui_spectrum.spectrum_type, UISpectrumType::Custom),
                                        };
                                        info!("Set new selected spectrum!");
                                        self.ui_values.selected_spectrum = Some(ui_selected_spectrum);
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
                    ui.horizontal_top(|ui| {
                        self.refresh_rendering_time();
                        self.display_frame_generation_time(ui);
                        egui::Frame::NONE.inner_margin(5.0).show(ui, |ui| {
                            ui.add(egui::ProgressBar::new(self.ui_values.progress_bar_progress));
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
                            // self.generate_image_actual(ctx);
                            // self.generate_image_float();
                            self.dispatch_render();
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
            }
        }
    }
}
