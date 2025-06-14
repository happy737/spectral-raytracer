use image::{DynamicImage, RgbaImage};

const NBR_DATA_POINTS_PER_PIXEL: usize = 4;

/// CustomImage is a struct which is supposed to hold images whose values are stored in f32 for each
/// channel. Additionally, pixel blending support is included to ease layering multiple images over
/// each other.
#[derive(Clone)]
pub struct CustomImage {
    width: u32,
    height: u32,
    data: Vec<f32>,
}

impl CustomImage {
    /// Generates a new CustomImage with given width and height. All float values are set to 0.0, 
    /// black in standard interpretation. The length of the data is width * height * 4 (r, g, b, a). 
    pub fn new(width: u32, height: u32) -> CustomImage {
        let data = vec![0.0; (width * height * 4) as usize];
        
        CustomImage {width, height, data}
    }
    
    /// Generates a new CustomImage from a given width, height and data vec. Will return a 
    /// CustomImageError if the length of the data does not match the width and height. 
    pub fn new_from_data(width: u32, height: u32, data: Vec<f32>) -> Result<CustomImage, CustomImageError> {
        if width * height * 4 != data.len() as u32 {
            return Err(CustomImageError{error: "Data length does not match given width and height!".to_string()});
        }
        Ok(CustomImage { width, height, data })
    }
    
    /// Takes a row of Pixels and blends each pixel with the corresponding row in the data. The 
    /// Pixels are blended according to the supplied weight factor where the new Pixels are 
    /// multiplied by new_weight_factor, the old Pixels are multiplied by 1 - new_weight_factor and 
    /// the two values are added to form the blended Pixels. <br/>
    /// Returns a CustomImageError if the row length does not equal width or if the row number is 
    /// equal to or greater than height. 
    pub fn blend_row(&mut self, pixels: &[Pixel], row_number: usize, new_weight_factor: f32) -> Result<(), CustomImageError>{   //TODO SIMD optimisation?
        if pixels.len() != self.width as usize {
            return Err(CustomImageError {error: "Row too long or short!".to_owned()});
        }
        if row_number >= self.height as usize {
            return Err(CustomImageError {error: "Specified row number does not exist!".to_owned()});
        }

        let pixel_size = size_of::<Pixel>();
        let row_length = pixel_size * self.width as usize;

        for x in 0..row_length {
            self.blend_pixel(x, row_number, &pixels[x], new_weight_factor)?;
        }
        Ok(())
    }

    /// Blends a single Pixel at the given position with the old data. The new Pixel is multiplied 
    /// by new_weight_factor and the old Pixel by 1 - new_weight_factor, then added together. <br/>
    /// Returns a CustomImageError if x or y are out of bounds. 
    pub fn blend_pixel(&mut self, x: usize, y: usize, pixel: &Pixel, new_weight_factor: f32)    //TODO SIMD optimisation?
        -> Result<(), CustomImageError> {

        let pixel_size = NBR_DATA_POINTS_PER_PIXEL;
        let row_length = pixel_size * self.width as usize;
        assert_eq!(row_length * self.height as usize, self.data.len(),
                   "Internal error: data length mismatch. The image has been corrupted!");
        if x >= self.width as usize || y >= self.height as usize {
            return Err(CustomImageError {error: 
            format!("{x} or {y} out of bounds for width {} or height {}!", self.width, self.height)});
        }

        let old_factor = 1.0 - new_weight_factor;
        let index = y * row_length + x * pixel_size;
        self.data[index] = self.data[index] * old_factor + pixel.r * new_weight_factor;
        self.data[index + 1] = self.data[index + 1] * old_factor + pixel.g * new_weight_factor;
        self.data[index + 2] = self.data[index + 2] * old_factor + pixel.b * new_weight_factor;
        self.data[index + 3] = self.data[index + 3] * old_factor + pixel.a * new_weight_factor;

        Ok(())
    }
    
    /// Returns the images width. 
    pub fn get_width(&self) -> u32 {
        self.width
    }
    
    /// Returns the images height. 
    pub fn get_height(&self) -> u32 {
        self.height
    }
}

impl From<CustomImage> for DynamicImage {
    fn from(value: CustomImage) -> Self {
        let data_as_bytes = value.data.into_iter().map(|mut float| {
            float = float.clamp(0.0, 1.0);
            float *= 255.0;
            float as u8
        }).collect::<Vec<u8>>();
        RgbaImage::from_raw(value.width, value.height, data_as_bytes).unwrap().into()
    }
}

/// An error type used by the CustomImage struct to communicate issues with the supplied parameters 
/// in the API. Specific details of the error are given in the error String. 
#[derive(Debug)]
pub struct CustomImageError {
    pub error: String,
}

/// A symbolic struct representing a pixel where the four f32 values represent red, green, blue and
/// alpha in order. Each field is publicly accessible. 
#[derive(Copy, Clone, Debug)]
pub struct Pixel {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}
