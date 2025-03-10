#[allow(clippy::excessive_precision)]
const SQRT_3: f32 = 1.732050807568877293527446341505872367;

// /// Takes the three components red, green and blue as f32 floating point values in range \[0;1] and 
// /// returns the corresponding hue, saturation and value in range \[0;1]. <br/>
// /// <br/>
// /// The definition is taken from Wikipedia: //TODO
// pub fn rgb_to_hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
//     let hue = (SQRT_3 * (g - b)).atan2(2.0 * r - g - b);    //TODO apparently this is a cheap approximation
//     let value = r.max(g.max(b));
//     let chroma = value - r.min(g.min(b));
//     let saturation = if value == 0.0 {
//         0.0
//     } else {
//         chroma / value
//     };
// 
//     (hue, saturation, value)
// }

pub fn hsv_to_rgb(hue: f32, saturation: f32, value: f32) -> (f32, f32, f32) {
    let chroma = value * saturation;
    let hue_prime = hue.rem_euclid(1.0) * 6.0;
    let x = chroma * (1.0 - (hue_prime.rem_euclid(2.0) - 1.0).abs());
    
    let (r, g, b) = match hue_prime as u8 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        5 => (chroma, 0.0, x),
        _ => (0.0, 0.0, 0.0), //non failing error case
    };
    
    let m = value - chroma;

    (r + m, g + m, b + m)
}