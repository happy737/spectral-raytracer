use std::ops::{AddAssign, Div, Mul, MulAssign};
use nalgebra::{Matrix3, Vector3};
use wide::f32x8;

pub const VISIBLE_LIGHT_WAVELENGTH_LOWER_BOUND: f32 = 380.0;
pub const VISIBLE_LIGHT_WAVELENGTH_UPPER_BOUND: f32 = 780.0;

/// A matrix which can be multiplied unto a [vec3](Vector3) to change the color space from XYZ to 
/// linear sRGB. To get to real sRGB, gamma correction has to be performed. 
const XYZ_TO_RGB_MATRIX: Matrix3<f32> = Matrix3::new(
    2.041369, -0.5649464, -0.3446944,
    -0.969266,  1.8760108,  0.0415560,
    0.0134474, -0.1183897,  1.0154096,
);

/// The Spectrum is a datatype designed to hold a spectrum of visible and non-visible wavelengths, 
/// together with their spectral radiance's. It supports various methods of creation to emulate 
/// realistic light sources, as well as allows typical mathematical operations to be performed on 
/// it, allowing for easy use in the shaders. It essentially replaces the r, g, b f32 triplet in 
/// closest-hit-shader calculations. <br>
/// Internally, the samples are stored in [SIMD facilitating structs](f32x8) which perform 
/// calculations in tuples of 8. Sample sizes of multiple of 8s are therefore most cost-efficient. 
#[derive(Clone, Copy, Debug)]
pub struct Spectrum {
    nbr_of_samples: usize,
    intensities: [f32; 128],
    spectrum_type: SpectrumType,    //currently useless, allows for distribution functions or similar to be used instead
}
impl Spectrum {
    //TODO as soon as spectrum_type is relevant, these constructors will be horrible. Maybe replace with factory?
    
    /// Creates a new Spectrum with the given field values. Essentially the short form of an 
    /// in-place creation. 
    fn new(intensities: &[f32; 128], spectrum_type: SpectrumType, nbr_of_samples: usize) -> Self {
        Spectrum {
            nbr_of_samples, 
            intensities: *intensities,
            spectrum_type,
        }
    }
    
    /// Creates a new Spectrum which essentially acts as a zero element. All samples are set to 
    /// zero and the amount of samples is set equal to the provided other Spectrum. 
    pub fn new_equal_size_empty_spectrum(other: &Spectrum) -> Self {    //TODO this might be optimized
        let nbr_of_samples = other.nbr_of_samples;
        let (lowest_wavelength, highest_wavelength) = match other.spectrum_type {
            SpectrumType::EquidistantSamples(lower,higher) => {
                (lower, higher)
            }
        };
        
        Self::new_singular_reflectance_factor(lowest_wavelength, highest_wavelength, nbr_of_samples, 0.0)
    }
    
    /// Creates a new Spectrum from a given list of intensities. Essentially allows custom 
    /// distributions to be submitted. 
    pub fn new_from_list(intensities: &[f32; 128], lowest_wavelength: f32, highest_wavelength: f32) -> Self {
        Spectrum {
            nbr_of_samples: 128,
            intensities: *intensities,
            spectrum_type: SpectrumType::EquidistantSamples(lowest_wavelength, highest_wavelength),
        }
    }

    /// # Currently does not work as intended! Blackbody radiation of the sun is used instead. 
    /// Creates a new Spectrum from experimental data portraying the sunlight spectrum - as received 
    /// below our atmosphere. 
    pub fn new_sunlight_spectrum(lowest_wavelength: f32, highest_wavelength: f32, nbr_of_samples: usize, multiplier: f32) -> Self {
        //TODO This does currently not work
        
        // let step = (highest_wavelength - lowest_wavelength) / (nbr_of_samples - 1) as f32;
        // let mut wavelengths = Vec::with_capacity(nbr_of_samples);
        // 
        // let mut current = lowest_wavelength;
        // while current <= highest_wavelength {
        //     let measured_value = spectral_data::get_sunlight_intensity(current);
        //     wavelengths.push(measured_value * multiplier);
        //     current += step;
        // }
        // 
        // Self::new_from_list(&wavelengths, lowest_wavelength, highest_wavelength)
        
        //workaround
        Self::new_temperature_spectrum(
            lowest_wavelength,
            highest_wavelength,
            6500.0,
            multiplier,
        )
    }
    
    /// Creates a new Spectrum from one value, the spectrum will be entirely flat with only the 
    /// given value repeated. 
    pub fn new_singular_reflectance_factor(lowest_wavelength: f32, highest_wavelength: f32, 
                                           nbr_of_samples: usize, reflectance_factor: f32) -> Self 
    {
        let arr = [reflectance_factor; 128];
        
        Self::new_from_list(&arr, lowest_wavelength, highest_wavelength)
    }
    
    
    /// Creates a new Spectrum from a given temperature. The spectrum is taken from the blackbody
    /// radiation spectrum of the given temperature and each sample is scaled by the provided 
    /// multiplier. 
    //TODO this catastrophe is a candidate for rewriting when benchmark shows an improvement
    pub fn new_temperature_spectrum(lowest_wavelength: f32, highest_wavelength: f32, temp_in_kelvin: f32, multiplier: f32) -> Self {
        let step = (highest_wavelength - lowest_wavelength) / (128 - 1) as f32;
        let mut wavelengths = Vec::with_capacity(128);

        let mut current = lowest_wavelength;
        while current <= highest_wavelength + step / 2.0 {  //adding half a step to ensure proper floating point accuracy
            let temperature_value = black_body_radiation(current as f64, temp_in_kelvin as f64) as f32;
            wavelengths.push(temperature_value * multiplier);
            current += step;
        }

        Self::new_from_list(<&[f32; 128]>::try_from(&wavelengths[..]).unwrap(), lowest_wavelength, highest_wavelength)
    }

    /// Takes a list of spectral radiance's and copies them into a Vector containing [f32x8]s which 
    /// are filled with the first lists values. Additionally, the amount of actual values is 
    /// reported as well (the last f32x8 is padded with zeroes if necessary). 
    fn spectral_radiance_list_to_simd_list(intensities: &[f32]) -> (usize, Vec<f32x8>) {
        let len = intensities.len();
        let capacity = len / 8 + (if len % 8 == 0 { 0 } else { 1 });
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
        (len, vec)
    }
    
    /// Returns the sample at the given index. Will return [None](None) if the index is out of 
    /// bounds. Can by design not give a reference, therefore a copied value is returned instead. 
    pub fn get_sample(&self, index: usize) -> Option<f32> {
        match self.spectrum_type {
            SpectrumType::EquidistantSamples(_, _) => {
                if index >= 128 {
                    return None;
                }

                Some(self.intensities[index])
            }
        }
    }
    
    /// Returns the spectral radiance at the given wavelength. If no sample exists for the precise 
    /// value, the spectral radiance is linearly interpolated from the two nearest samples. If the 
    /// wavelength is outside the spectrum range, 0 is returned. 
    fn get_spectral_radiance_by_wavelength(&self, wavelength: f32) -> f32 {
        let (lower_bound, upper_bound) = self.get_range();
        
        if !(lower_bound..=upper_bound).contains(&wavelength) {
            return 0.0;
        }
        
        let index_norm = (wavelength - lower_bound) / (upper_bound - lower_bound);
        let index_frac = index_norm * (self.nbr_of_samples - 1) as f32;
        if index_frac.fract() == 0.0 {
            return self.get_sample(index_frac as usize).unwrap()
        }
        
        let index_lower = index_frac.floor() as usize;
        let index_upper = index_frac.ceil() as usize;
        let frac = index_frac.fract();
        let frac_inv = 1.0 - frac;
        
        self.get_sample(index_lower).unwrap() * frac + 
            self.get_sample(index_upper).unwrap() * frac_inv
    }
    
    /// Modifies the inner intensities to each be at least 0.0 via [fast_max](f32x8::fast_max). 
    /// Make sure none of the intensities are NaN. 
    pub fn max0(&mut self) {
        for elem in self.intensities.iter_mut() {
            *elem = elem.max(0.0);
        }
    }

    /// This function is heavily subject to change! <br>
    /// Takes the spectrum and converts it into RGB values. <br>
    /// <br>
    /// The current approach is to convert the wavelengths to XYZ via an official CIE lookup table
    /// and then convert this to RGB. RGB is taken to be Adobes sRGB. <br>
    /// See https://stackoverflow.com/a/51639077 (saved website can be seen in ../research_materials )
    pub fn to_rgb_early(&self) -> (f32, f32, f32) {
        match self.spectrum_type {
            SpectrumType::EquidistantSamples(min, max) => {
                let mut xyz_values: Vec<Vector3<f32>> = Vec::with_capacity(self.nbr_of_samples);
                let sample_distance = (max - min) / (self.nbr_of_samples - 1) as f32;
            
                let mut wavelength = min;
                while wavelength <= max {
                    let xyz = wavelength_to_XYZ(wavelength).in2();
                    xyz_values.push(xyz / self.nbr_of_samples as f32);
                    wavelength += sample_distance;
                }
            
                for (i, xyz) in xyz_values.iter_mut().enumerate() {
                    *xyz *= self.get_sample(i).unwrap();
                }
            
                let fin = xyz_values.into_iter().fold(Vector3::new(0.0, 0.0, 0.0), |acc, x| acc + x);
                let rgb: Vector3<f32> = XYZ_TO_RGB_MATRIX * fin;
                //gamma_correction(&mut rgb);
                rgb.in2()
            }
        }
    }
    
    /// Getter for the lower and upper end of the spectrum in order. 
    pub fn get_range(&self) -> (f32, f32) {
        match self.spectrum_type {
            SpectrumType::EquidistantSamples(min, max) => {
                (min, max)
            }
        }
    } 
    
    /// Getter for the number of samples with which the spectrum is sampled.
    pub fn get_nbr_of_samples(&self) -> usize {
        self.nbr_of_samples
    }
    
    /// Takes the given bounds as the new lower and upper bound, adjusting the samples accordingly. 
    /// //TODO if sampling out of old bounds, nearest neighbor ?
    pub fn rebound(&mut self, _lower_bound: f32, _upper_bound: f32) {
        todo!()
    }
    
    /// Modifies the existing Spectrum to be sampled with new_sample_amount. Does nothing if the 
    /// new amount is the same as the old one. 
    pub fn resample(&mut self, new_sample_amount: usize) {
        //TODO reimplement
        
        // assert!(new_sample_amount > 1);
        // 
        // if new_sample_amount == self.nbr_of_samples {
        //     return;
        // }
        // 
        // let (lower_bound, upper_bound) = self.get_range();
        // let step = (upper_bound - lower_bound) / (new_sample_amount - 1) as f32;
        // 
        // let mut samples = Vec::with_capacity(new_sample_amount);
        // 
        // let mut current = lower_bound;
        // while current <= upper_bound {
        //     samples.push(self.get_spectral_radiance_by_wavelength(current));
        //     current += step;
        // }
        // 
        // let (_, new_simd_vec) = Self::spectral_radiance_list_to_simd_list(&samples);
        // 
        // self.nbr_of_samples = new_sample_amount;
        // self.intensities = new_simd_vec;
    }
    
    /// Generates an Iterator which will yield tuples of wavelengths and their respective spectral 
    /// radiance's. 
    pub fn iter(&self) -> SpectrumIterator {
        let (lower, upper) = self.get_range();
        let step = (upper - lower) / (self.nbr_of_samples - 1) as f32;
        
        SpectrumIterator {
            spectrum: self,
            index: 0,
            step,
        }
    }
    
    /// Calculates the radiance of the spectrum. This is the integral over the spectral radiance's.
    pub fn get_radiance(&self) -> f32 {
        let iter = self.iter();
        let step = iter.step;
        iter.map(|(_, spectral_radiance)| spectral_radiance * step)
            .fold(0f32, |acc, elem| acc + elem) 
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
        let mut new_arr = self.intensities.clone();
        for (elem, rhs_elem) in new_arr.iter_mut().zip(rhs.intensities.iter()) {
            *elem /= rhs_elem;
        }
        
        Spectrum::new(&new_arr, self.spectrum_type, self.nbr_of_samples)
    }
}

impl Mul<&Spectrum> for &Spectrum {
    type Output = Spectrum;
    
    fn mul(self, rhs: &Spectrum) -> Self::Output {
        let mut new_arr = self.intensities.clone();
        for (elem, rhs_elem) in new_arr.iter_mut().zip(rhs.intensities.iter()) {
            *elem *= rhs_elem;
        }
        
        Spectrum::new(&new_arr, self.spectrum_type, self.nbr_of_samples)
    }
}

impl MulAssign<f32> for Spectrum {
    fn mul_assign(&mut self, rhs: f32) {
        for elem in self.intensities.iter_mut() {
            *elem *= rhs;
        }
    }
}

impl Div<f32> for &Spectrum {
    type Output = Spectrum;
    
    fn div(self, rhs: f32) -> Self::Output {
        let mut new_arr = self.intensities.clone();
        for elem in new_arr.iter_mut() {
            *elem = *elem / rhs;
        }
        Spectrum::new(&new_arr, self.spectrum_type, self.nbr_of_samples)
    }
}

pub struct SpectrumIterator<'a> {
    spectrum: &'a Spectrum,
    index: usize,
    step: f32,
}
impl<'a> Iterator for SpectrumIterator<'a> {
    type Item = (f32, f32);

    fn next(&mut self) -> Option<Self::Item> {
        match self.spectrum.get_sample(self.index) {
            Some(spectral_radiance) => {
                let wavelength = self.spectrum.get_range().0 + self.step * self.index as f32;
                self.index += 1;
                
                Some((wavelength, spectral_radiance))
            }
            None => None
        }
    }
}

/// Determines the type of the Spectrum datatype. This exists to future-proof Spectrum to be usable 
/// with function approximations or other ways of storing distributions. 
#[derive(Clone, Copy, Debug)]
enum SpectrumType {
    /// The Spectrum holds a list of samples, each spaced with the same step width. The samples 
    /// represent a crude discretization of the underlying distribution. 
    EquidistantSamples(f32, f32),
    //TODO maybe add a zero type which makes addition and multiplication etc more performant. 
    // this could use of the fact that Vec::new() does not allocate and later operations can be skipped
    //TODO add second type which approximates a distribution
}

trait In2<T> {  //dirty hack
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

const SPEED_OF_LIGHT: f64 = 299_792_458_f64; // m/s 
const PLANCK_CONSTANT: f64 = 6.62607015e-34;
const BOLTZMANN_CONSTANT: f64 = 1.380649e-23;

///                 2hc^2           1       
///     B_l(l, T) = ----- * ------------------
///                  l^5    e^(hc/l*T*k_B) - 1 
/// l = lambda = Wavelength                         <br>
/// h = Planck constant                             <br>
/// c = speed of light in a vacuum                  <br>
/// k = Boltzmann constant                          <br>
/// v = frequency of electromagnetic radiation      <br>
/// T = absolute temperature (in Kelvin)            <br>
/// <br>
/// Calculates the _Spectral Radiance_ (W / sr / m^2 / nm) of a given wavelength (in Nanometers) at 
/// the given temperature (in Kelvin) according to the above formula. The values should be accurate 
/// across all ranges, but no guarantees are given beyond the visual spectrum. <br><br>
/// Will panic if: 
/// 1. wavelength_nm is not positive. 
/// 2. temperature_k is negative. 
fn black_body_radiation(wavelength_nm: f64, temperature_k: f64) -> f64 {
    assert!(wavelength_nm > 0.0, "Wavelengths must be physical, real, positive values. Got: {wavelength_nm}nm.");
    assert!(temperature_k >= 0.0, "Temperatures in Kelvin are real, non-negative values. Got: {temperature_k}K.");
    
    let lambda = wavelength_nm / 1e9;  //nanometer to meter
    let hc22 = 2.0 * PLANCK_CONSTANT * SPEED_OF_LIGHT * SPEED_OF_LIGHT;
    let l5 = lambda * lambda * lambda * lambda * lambda;
    let hc = PLANCK_CONSTANT * SPEED_OF_LIGHT;
    let ltk = lambda * temperature_k * BOLTZMANN_CONSTANT;
    let big_denominator = f64::exp(hc / ltk) - 1.0;

    (hc22 / l5) * (1.0 / big_denominator)  * 1e-9   //*1e-9 = to /nanometer
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

#[cfg(test)]
mod test {
    use crate::shader::F32_DELTA;
    use super::*;

    #[test]
    #[allow(non_snake_case)]
    fn test_wavelength_to_XYZ() {
        //wavelength is too low to be visible
        assert_eq!(wavelength_to_XYZ(379.0), (0.0, 0.0, 0.0));

        //wavelength is too high to be visible
        assert_eq!(wavelength_to_XYZ(781.0), (0.0, 0.0, 0.0));

        //visible wavelength straight from the table
        assert_eq!(wavelength_to_XYZ(750.0), (0.000251, 0.000098, 0.000000));

        //interpolate perfect middle
        let xyz_702_5 = wavelength_to_XYZ(702.5);
        assert!(
            (xyz_702_5.0 - 0.008_091).abs() <= F32_DELTA &&
                (xyz_702_5.1 - 0.003_141_5).abs() <= F32_DELTA &&
                xyz_702_5.2 == 0.0
        );

        //interpolate skewed
        let xyz_776 = wavelength_to_XYZ(776.0);
        assert!(
            (xyz_776.0 - 0.000_043_4).abs() <= F32_DELTA &&
                (xyz_776.1 - 0.000_017).abs() <= F32_DELTA &&
                xyz_776.2 == 0.0
        )
    }

    #[test]
    fn test_spectrum_to_rgb() {
        //assert the XYZ to RGB part works
        let d65 = Vector3::new(95.047, 100.0, 108.883); //<- pure white
        let white = XYZ_TO_RGB_MATRIX * d65;
        assert!(
            (white.x - 100.0).abs() <= 0.01 &&
                (white.y - 100.0).abs() <= 0.01 &&
                (white.z - 100.0).abs() <= 0.01
        );

        //assert the sun produces white light
        let sun = Spectrum::new_sunlight_spectrum(
            VISIBLE_LIGHT_WAVELENGTH_LOWER_BOUND,
            VISIBLE_LIGHT_WAVELENGTH_UPPER_BOUND,
            64,
            1.0,
        );
        let (r, g, b) = sun.to_rgb_early();
        assert!((r - g).abs() < 0.01, "Red ({r}) and green ({g}) too different to be greyscale!");
        assert!((g - b).abs() < 0.01, "Green ({g}) and blue ({b}) too different to be greyscale!");
        assert!((r - b).abs() < 0.01, "Red ({r}) and blue ({b}) too different to be greyscale!");
        
        //TODO more useful tests as soon as the current one passes :,(  
    }
    
    #[test]
    fn test_black_body_calculation() {
        const DELTA: f64 = 0.0001;
        
        let wavelength = 500.0;
        let temperature = 5000.0;
        let expected = 12_107.190_590_398;
        let spectral_radiance = black_body_radiation(wavelength, temperature);
        let close_enough = (1.0 - spectral_radiance / expected).abs() < DELTA;
        assert!(close_enough, "Spectral Radiance for wavelength {wavelength}nm, temperature \
        {temperature}K significantly diverges from expected value. Expected: {expected}, Actual: \
        {spectral_radiance}");

        let wavelength = 500.0;
        let temperature = 1000.0;
        let expected = 0.000_001_213_4;
        let spectral_radiance = black_body_radiation(wavelength, temperature);
        let close_enough = (1.0 - spectral_radiance / expected).abs() < DELTA;
        assert!(close_enough, "Spectral Radiance for wavelength {wavelength}nm, temperature \
        {temperature}K significantly diverges from expected value. Expected: {expected}, Actual: \
        {spectral_radiance}");

        let wavelength = 700.0;
        let temperature = 2000.0;
        let expected = 24.390_318_624;
        let spectral_radiance = black_body_radiation(wavelength, temperature);
        let close_enough = (1.0 - spectral_radiance / expected).abs() < DELTA;
        assert!(close_enough, "Spectral Radiance for wavelength {wavelength}nm, temperature \
        {temperature}K significantly diverges from expected value. Expected: {expected}, Actual: \
        {spectral_radiance}");

        let wavelength = 400.0;
        let temperature = 500.0;
        let spectral_radiance = black_body_radiation(wavelength, temperature);
        assert!(spectral_radiance < 0.0000000001, "Spectral Radiance for wavelength {wavelength}nm, temperature \
        {temperature}K significantly diverges from expected value. Expected: 0, Actual: \
        {spectral_radiance}");
    }
    
    #[test]
    #[should_panic]
    fn test_illegal_parameter_temperature_black_body_calculation() {
        let wavelength = 100.0;
        let illegal_temperature = -1.0;
        let _ = black_body_radiation(wavelength, illegal_temperature);
    }

    #[test]
    #[should_panic]
    fn test_illegal_parameter_wavelength_black_body_calculation() {
        let illegal_wavelength = 0.0;
        let temperature = 1000.0;
        let _ = black_body_radiation(illegal_wavelength, temperature);
    }
}
