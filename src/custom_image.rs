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
    pub fn new(width: u32, height: u32) -> CustomImage {
        let data = vec![0.0; (width * height * 4) as usize];
        
        CustomImage {width, height, data}
    }
    
    pub fn new_from_data(width: u32, height: u32, data: Vec<f32>) -> Result<CustomImage, CustomImageError> {
        if width * height * 4 != data.len() as u32 {
            return Err(CustomImageError{error: "Data length does not match given width and height!".to_string()});
        }
        Ok(CustomImage { width, height, data })
    }
    
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
    
    pub fn get_width(&self) -> u32 {
        self.width
    }
    
    pub fn get_height(&self) -> u32 {
        self.height
    }
}

impl Into<DynamicImage> for CustomImage {   //TODO replace with implementation DynamicImage::from::<CustomImage>
    fn into(self) -> DynamicImage {
        let data_as_bytes = self.data.into_iter().map(|mut float| {
            float *= 255.0;
            float as u8
        }).collect::<Vec<u8>>();

        RgbaImage::from_raw(self.width, self.height, data_as_bytes).unwrap().into()
    }
}

#[derive(Debug)]
pub struct CustomImageError {
    pub error: String,
}

#[derive(Copy, Clone, Debug)]
pub struct Pixel {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}
