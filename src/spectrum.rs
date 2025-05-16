use std::ops::{AddAssign, Div, Mul, MulAssign};
use nalgebra::{Matrix3, Vector3};
use wide::f32x8;
use crate::spectral_data;

pub const VISIBLE_LIGHT_WAVELENGTH_LOWER_BOUND: f32 = 380.0;
pub const VISIBLE_LIGHT_WAVELENGTH_UPPER_BOUND: f32 = 780.0;

const XYZ_TO_RGB_MATRIX: Matrix3<f32> = Matrix3::new(
    2.041369, -0.5649464, -0.3446944,
    -0.969266,  1.8760108,  0.0415560,
    0.0134474, -0.1183897,  1.0154096,
);

#[derive(Clone, Debug)]
pub struct Spectrum {
    nbr_of_samples: usize,
    intensities: Vec<f32x8>,
    spectrum_type: SpectrumType,    //currently useless, allows for distribution functions or similar to be used instead
}
impl Spectrum {
    //TODO as soon as spectrum_type is relevant, these constructors will be horrible. Maybe replace with factory?
    
    fn new(intensities: Vec<f32x8>, spectrum_type: SpectrumType, nbr_of_samples: usize) -> Self {
        Spectrum {
            nbr_of_samples, 
            intensities,
            spectrum_type,
        }
    }
    
    pub fn new_equal_size_empty_spectrum(other: &Spectrum) -> Self {    //TODO this might be optimized
        let nbr_of_samples = other.nbr_of_samples;
        let (lowest_wavelength, highest_wavelength) = match other.spectrum_type {
            SpectrumType::EquidistantSamples(lower,higher) => {
                (lower, higher)
            }
        };
        
        Self::new_singular_reflectance_factor(lowest_wavelength, highest_wavelength, nbr_of_samples, 0.0)
    }
    
    pub fn new_from_list(intensities: &[f32], lowest_wavelength: f32, highest_wavelength: f32) -> Self {
        let len = intensities.len();
        let capacity = len / 8 + ( if len % 8 == 0 { 0 } else { 1 } );
        let mut vec: Vec<f32x8> = Vec::with_capacity(capacity);
        let mut iter = intensities.iter().peekable();
        while iter.peek().is_some() {
            let simd = f32x8::from([
                *iter.next().unwrap(),
                *iter.next().unwrap_or(&0.0),
                *iter.next().unwrap_or(&0.0),
                *iter.next().unwrap_or(&0.0),
                
                *iter.next().unwrap_or(&0.0),
                *iter.next().unwrap_or(&0.0),
                *iter.next().unwrap_or(&0.0),
                *iter.next().unwrap_or(&0.0),
            ]);
            vec.push(simd);
        }
        
        Spectrum {
            nbr_of_samples: len,
            intensities: vec,
            spectrum_type: SpectrumType::EquidistantSamples(lowest_wavelength, highest_wavelength),
        }
    }

    pub fn new_sunlight_spectrum(lowest_wavelength: f32, highest_wavelength: f32, nbr_of_samples: usize, multiplier: f32) -> Self {
        let step = (highest_wavelength - lowest_wavelength) / (nbr_of_samples - 1) as f32;
        let mut wavelengths = Vec::with_capacity(nbr_of_samples);

        let mut current = lowest_wavelength;
        while current <= highest_wavelength {
            let measured_value = spectral_data::get_sunlight_intensity(current);
            wavelengths.push(measured_value * multiplier);
            current += step;
        }

        Self::new_from_list(&wavelengths, lowest_wavelength, highest_wavelength)
    }
    
    pub fn new_singular_reflectance_factor(lowest_wavelength: f32, highest_wavelength: f32, 
                                           nbr_of_samples: usize, reflectance_factor: f32) -> Self 
    {
        let vec = vec![reflectance_factor; nbr_of_samples];
        
        Self::new_from_list(&vec, lowest_wavelength, highest_wavelength)
    }
    
    //TODO make it not crash spectacularly when given wrong index //perhaps it does *not* crash *spectacularly*, instead parent waits indefinitely?
    pub fn get_sample(&self, index: usize) -> f32 {
        let simd = self.intensities[index / 8];
        let arr: [f32; 8] = simd.into();
        arr[index % 8]
    }
    
    /// Modifies the inner intensities to each be at least 0.0 via [fast_max](f32x8::fast_max). 
    /// Make sure none of the intensities are NaN. 
    pub fn max0(&mut self) {
        let simd_zeroes = f32x8::from([0.0; 8]);
        for elem in self.intensities.iter_mut() {
            *elem = elem.fast_max(simd_zeroes);
        }
    }

    //TODO this function probably is inefficient, measure runtimes and if appropriate rewrite it    //oof runtime * 10
    /// This function is heavily subject to change! <br>
    /// Takes the spectrum and converts it into RGB values. <br>
    /// <br>
    /// The current approach is to convert the wavelengths to XYZ via an official CIE lookup table
    /// and then convert this to RGB. RGB is taken to be Adobes sRGB. <br>
    /// See https://stackoverflow.com/a/51639077 (saved website can be seen in ../research_materials )
    pub fn to_rgb_early(&self) -> (f32, f32, f32) {
        match self.spectrum_type {
            SpectrumType::EquidistantSamples(min, max) => {
                let mut rgb_values: Vec<Vector3<f32>> = Vec::with_capacity(self.nbr_of_samples);
                let sample_distance = (max - min) / (self.nbr_of_samples - 1) as f32;
                
                let mut wavelength = min;
                while wavelength <= max {
                    //TODO use crate "kolor" to change RGB scaling to xyY scaling and _then_ go to RGB
                    let rgb = XYZ_TO_RGB_MATRIX * wavelength_to_XYZ(wavelength).in2();
                    rgb_values.push(rgb / self.nbr_of_samples as f32);
                    wavelength += sample_distance;
                }
                
                for i in 0..self.nbr_of_samples {
                    rgb_values[i] *= self.get_sample(i);
                }
                
                rgb_values.into_iter().fold(Vector3::new(0.0, 0.0, 0.0), |acc, x| acc + x).in2()
            }
        }
    }
}

impl AddAssign<&Spectrum> for Spectrum {
    fn add_assign(&mut self, rhs: &Spectrum) {
        assert_eq!(self.nbr_of_samples, rhs.nbr_of_samples);

        for elem in self.intensities.iter_mut().zip(rhs.intensities.iter()) {
            let (lhs, rhs) = elem;
            *lhs += rhs;
        }
    }
}

impl MulAssign<&Spectrum> for Spectrum {
    fn mul_assign(&mut self, rhs: &Spectrum) {
        assert_eq!(self.nbr_of_samples, rhs.nbr_of_samples);
        
        for elem in self.intensities.iter_mut().zip(rhs.intensities.iter()) {
            let (lhs, rhs) = elem;
            *lhs *= rhs;
        }
    }
}

impl Div<&Spectrum> for &Spectrum {
    type Output = Spectrum;

    fn div(self, rhs: &Spectrum) -> Self::Output {  //TODO this should be differentiated by spectrum_type (match ...)
        assert_eq!(self.nbr_of_samples, rhs.nbr_of_samples);
        
        let mut vec = Vec::with_capacity(self.intensities.len());
        for elem in self.intensities.iter().zip(rhs.intensities.iter()) {
            let (lhs, rhs) = elem;
            vec.push(*lhs / rhs);
        }
        
        Spectrum::new(vec, self.spectrum_type, self.nbr_of_samples)
    }
}

impl Mul<&Spectrum> for &Spectrum {
    type Output = Spectrum;
    
    fn mul(self, rhs: &Spectrum) -> Self::Output {
        assert_eq!(self.nbr_of_samples, rhs.nbr_of_samples);
        
        let mut vec = Vec::with_capacity(self.intensities.len());
        for elem in self.intensities.iter().zip(rhs.intensities.iter()) {
            let (lhs, rhs) = elem;
            vec.push(*lhs * rhs);
        }
        Spectrum::new(vec, self.spectrum_type, self.nbr_of_samples)
    }
}

impl MulAssign<f32> for Spectrum {
    fn mul_assign(&mut self, rhs: f32) {
        for elem in self.intensities.iter_mut() {
            *elem = *elem * rhs;
        }
    }
}

impl Div<f32> for &Spectrum {
    type Output = Spectrum;
    
    fn div(self, rhs: f32) -> Self::Output {
        let vec = self.intensities.iter().map(|elem| {
            *elem / rhs
        }).collect::<Vec<f32x8>>();
        Spectrum::new(vec, self.spectrum_type, self.nbr_of_samples)
    }
}

#[derive(Clone, Copy, Debug)]
enum SpectrumType {
    EquidistantSamples(f32, f32)
}

trait In2<T> {
    fn in2(self) -> T;
}

impl In2<Vector3<f32>> for (f32, f32, f32) {
    fn in2(self) -> Vector3<f32> {
        Vector3::new(self.0, self.1, self.2)
    }
}
impl In2<(f32, f32, f32)> for Vector3<f32> {
    fn in2(self) -> (f32, f32, f32) {
        (
            self.x,
            self.y,
            self.z,
        )
    }
}

/// Computes the color in the XYZ colorspace of a given light wavelength. The wavelength unit must 
/// be nanometers. If no precise sample exists for the given wavelength, it is instead linearly
/// interpolated. 
//magical values here come from const WAVELENGTH_TO_XYZ_TABLE
#[allow(non_snake_case)]    //allowing non snake case because color space XYZ != color space xyz
fn wavelength_to_XYZ(wavelength: f32) -> (f32, f32, f32) {
    //filter out non-visible light
    if !(380.0..=780.0).contains(&wavelength) {
        return (0.0, 0.0, 0.0);
    }

    //wavelength can be immediately cast to table lookup
    if wavelength % 5.0 == 0.0 {
        let index = (wavelength as usize - 380) / 5;
        return WAVELENGTH_TO_XYZ_TABLE[index];
    }

    //linear interpolation between two closest values
    let w_adjusted = (wavelength - 380.0) / 5.0;
    let index_lower = w_adjusted as usize;
    let index_upper = index_lower + 1;
    
    let value_lower = WAVELENGTH_TO_XYZ_TABLE[index_lower];
    let value_upper = WAVELENGTH_TO_XYZ_TABLE[index_upper];
    let fract = w_adjusted.fract();
    let fract_inv = 1.0 - fract;

    (
        value_lower.0 * fract + value_upper.0 * fract_inv,
        value_lower.1 * fract + value_upper.1 * fract_inv,
        value_lower.2 * fract + value_upper.2 * fract_inv,
    )
}


/// A lookup table to convert color in terms of a light wavelength to the XYZ color space. The table
/// contains samples at 5 nanometer intervals. The smallest available sample is 380nm and the
/// largest available sample is 780nm. Anything beyond can be taken as (0, 0, 0).
//CHANGES HERE MUST BE REFLECTED IN fn wavelength_to_XYZ !
const WAVELENGTH_TO_XYZ_TABLE: [(f32, f32, f32); 81] = [
    (0.00016, 0.000017, 0.000705),      //380nm
    (0.000662, 0.000072, 0.002928),     //385nm
    (0.002362, 0.000253, 0.010482),     //...
    (0.007242, 0.000769, 0.032344),
    (0.01911, 0.002004, 0.086011),      //400nm
    (0.0434, 0.004509, 0.197120),
    (0.084736, 0.008756, 0.389366),
    (0.140638, 0.014456, 0.656760),
    (0.204492, 0.021391, 0.972542),
    (0.264737, 0.029497, 1.28250),
    (0.314679, 0.038676, 1.55348),
    (0.357719, 0.049602, 1.79850),
    (0.383734, 0.062077, 1.96728),
    (0.386726, 0.074704, 2.02730),
    (0.370702, 0.089456, 1.99480),     //450nm
    (0.342957, 0.106256, 1.90070),
    (0.302273, 0.128201, 1.74537),
    (0.254085, 0.152761, 1.55490),
    (0.195618, 0.18519, 1.31756),
    (0.132349, 0.21994, 1.03020),
    (0.080507, 0.253589, 0.772125),
    (0.041072, 0.297665, 0.570060),
    (0.016172, 0.339133, 0.415254),
    (0.005132, 0.395379, 0.302356),
    (0.003816, 0.460777, 0.218502),     //500nm
    (0.015444, 0.53136, 0.159249),
    (0.037465, 0.606741, 0.112044),
    (0.071358, 0.68566, 0.082248),
    (0.117749, 0.761757, 0.060709),
    (0.172953, 0.82333, 0.043050),
    (0.236491, 0.875211, 0.030451),
    (0.304213, 0.92381, 0.020584),
    (0.376772, 0.961988, 0.013676),
    (0.451584, 0.9822, 0.007918),
    (0.529826, 0.991761, 0.003988),     //550nm
    (0.616053, 0.99911, 0.001091),
    (0.705224, 0.99734, 0.000000),
    (0.793832, 0.98238, 0.000000),
    (0.878655, 0.955552, 0.000000),
    (0.951162, 0.915175, 0.000000),
    (1.01416, 0.868934, 0.000000),
    (1.0743, 0.825623, 0.000000),
    (1.11852, 0.777405, 0.000000),
    (1.1343, 0.720353, 0.000000),
    (1.12399, 0.658341, 0.000000),      //600nm
    (1.0891, 0.593878, 0.000000),
    (1.03048, 0.527963, 0.000000),
    (0.95074, 0.461834, 0.000000),
    (0.856297, 0.398057, 0.000000),
    (0.75493, 0.339554, 0.000000),
    (0.647467, 0.283493, 0.000000),
    (0.53511, 0.228254, 0.000000),
    (0.431567, 0.179828, 0.000000),
    (0.34369, 0.140211, 0.000000),
    (0.268329, 0.107633, 0.000000),     //650nm
    (0.2043, 0.081187, 0.000000),
    (0.152568, 0.060281, 0.000000),
    (0.11221, 0.044096, 0.000000),
    (0.081261, 0.0318, 0.000000),
    (0.05793, 0.022602, 0.000000),
    (0.040851, 0.015905, 0.000000),
    (0.028623, 0.01113, 0.000000),
    (0.019941, 0.007749, 0.000000),
    (0.013842, 0.005375, 0.000000),
    (0.009577, 0.003718, 0.000000),     //700nm
    (0.006605, 0.002565, 0.000000),
    (0.004553, 0.001768, 0.000000),
    (0.003145, 0.001222, 0.000000),
    (0.002175, 0.000846, 0.000000),
    (0.001506, 0.000586, 0.000000),
    (0.001045, 0.000407, 0.000000),
    (0.000727, 0.000284, 0.000000),
    (0.000508, 0.000199, 0.000000),
    (0.000356, 0.00014, 0.000000),
    (0.000251, 0.000098, 0.000000),     //750nm
    (0.000178, 0.00007, 0.000000),
    (0.000126, 0.00005, 0.000000),
    (0.00009, 0.000036, 0.000000),
    (0.000065, 0.000025, 0.000000),
    (0.000046, 0.000018, 0.000000),
    (0.000033, 0.000013, 0.000000),     //780nm
];
