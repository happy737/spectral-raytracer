// settings 
pub const IMAGE_WIDTH_TOOLTIP: &str = "The width of the image in pixels.";
pub const IMAGE_HEIGHT_TOOLTIP: &str = "The height of the image in pixels.";
pub const NUMBER_OF_PARALLEL_THREADS_TOOLTIP: &str  = "The number of parallel threads computing the \
    image at any given time. The default value fully utilizes the CPU. If the computer is to be \
    used otherwise during the duration of the rendering process, it is recommended to reduce \
    the number by one or two.";
pub const NUMBER_OF_ITERATIONS_TOOLTIP: &str  = "The number of frames generated to form the final \
    image. Higher numbers take proportionally more time to render, but reduce the noise in the \
    image, as well as make the lighting more correct. For decent results, use numbers greater than \
    100. For good results, greater than 1000.";
pub const MAX_BOUNCES_TOOLTIP: &str = "The maximum number of rays that will be traced. 1 means only \
    the direct light of the hit object is considered. 2 means one additional ray will be shot to hit \
    other objects. 2 means the new hit object shoots one additional ray, etc. Lower \
    numbers mean better performance, but light will spread less and the image will look darker. \
    Use 30 as a default.";


// objects
pub const CAMERA_POSITION_TOOLTIP: &str = "The position of the camera in the scene."; 
pub const CAMERA_DIRECTION_TOOLTIP: &str = "The direction in which the camera looks. In the default \
    scene, positive X is to the right, positive Y is upwards and positive Z looks into the screen.";
pub const CAMERA_UP_TOOLTIP: &str = "The direction which the camera considers to be up. Changing \
    this value allows for tilted cameras.";
pub const CAMERA_FOV_TOOLTIP: &str = "The vertical FOV of the camera. The horizontal FOV is \
    dependent on the vertical FOV and the aspect ratio."; 
pub const LIGHT_SOURCE_TOOLTIP: &str = "The position of the light source in the scene.";
pub const OBJECT_TYPE_TOOLTIP: &str = "The type of the object. The type determines its shape and \
    collision detection speed. Having many complex types may drastically lower rendering speed."; 
pub const OBJECT_POSITION_TOOLTIP: &str = "The position of the object in the scene. The position \
    is defined as the point of the object where its local coordinates (0, 0) land.";
pub const OBJECT_METALLICNESS_TOOLTIP: &str = "The metallicness of the material of the object. \
    A metallic object is reflective like a mirror, whereas a non metallic object is reflective like \
    a simple piece of plastic"; 
pub const OBJECT_PLAIN_BOX_DIMENSIONS_TOOLTIP: &str = "The width, height and depth of an \
    axis-aligned box."; 
pub const OBJECT_SPHERE_RADIUS_TOOLTIP: &str = "The radius of the sphere.";
pub const LIGHT_SPECTRUM_TOOLTIP: &str = "The spectrum emitted by this light source. Individual \
    spectra can be adjusted in their respective tab.";
pub const OBJECT_SPECTRUM_REFLECTING_TOOLTIP: &str = "The spectrum reflected by the object. Each \
    sample value is the share of this wavelength that is reflected. A spectrum of only 1 will \
    fully reflect every wavelength, essentially a perfectly white body.";
pub const OBJECT_TYPE_PLAIN_BOX_TOOLTIP: &str = "The simplest shape, a simple box. This box can be \
    stretched and moved. It can, however, not be rotated, it is always axis-aligned. This shape is \
    the fastest to compute.";
pub const OBJECT_TYPE_SPHERE_TOOLTIP: &str = "A perfect sphere. Only its position and radius can \
    be adjusted.";
pub const OBJECT_TYPE_ROTATED_BOX_TOOLTIP: &str = "A box that can be rotated as well as stretched \
    and moved. It is more expensive to compute than the plain box.";
pub const OBJECT_ROTATED_BOX_DIMENSIONS_TOOLTIP: &str = "The width, height and depth of a rotated \
    box. Important: The dimensions are defined on a non-rotated box, only after will it be rotated.";
pub const OBJECT_ROTATED_BOX_ANGLES_TOOLTIP: &str = "The rotation angles around the X, Y and Z \
    axis. The angles are in radians. The three angles are treated as euler-angles.";


//spectra and materials
pub const SPECTRUM_NUMBER_OF_SAMPLES_TOOLTIP: &str = "The number of samples used to sample the \
    Spectrum. Higher numbers mean clearer images and more accurate numbers but also higher \
    computing times. Multiples of 8 are most cost-efficient.";
pub const SPECTRUM_RANGE_TOOLTIP: &str = "The lower and upper bound of the spectrum. The default \
    values are the range of visible light.";
pub const OBSERVED_COLOR_TOOLTIP: &str = "The color of the spectrum when looking directly at it. \
    Welding sparks and lightning flashes are not true white, but they are so bright that they \
    subjectively appear white. If this light source is bright enough, any color can appear white \
    here.";
pub const NORMALIZED_COLOR_TOOLTIP: &str = "The color of the spectrum when it is sufficiently \
    dimmed or brightened. This view shows what kind of color a light source could throw unto a \
    distant object.";
pub const REFLECTED_COLOR_TOOLTIP: &str = "The color of an (approximately) white spectrum when it \
    is reflected by a surface with this reflective spectrum.";
pub const SPECTRUM_TYPE_TOOLTIP: &str = "The preliminary type of the spectrum. The type \
    determines the initial shape of the spectrum. The type can be changed to custom, which \
    allows for direct editing of the samples. \nBeware: In opposition to all other types, changing \
    the number of samples of a custom spectrum can lead to unexpected results!";
pub const SPECTRUM_EFFECT_TYPE_TOOLTIP: &str = "The way the spectrum is intended to behave. \
    There are two primary ways:\n\
    1. Emitting: The spectrum is a light source. Use this for light sources. The values can take \
    any form, typically larger than 1 in many places.\n\
    2. Reflecting: The spectrum is not emitted. Instead it describes the share of each wavelength \
    that is reflected. Under white light, a reflecting spectrum with only 0.5 as its values will \
    appear as a medium gray. Reflection values must be in range [0; 1].";
pub const SPECTRUM_RADIANCE_TOOLTIP: &str = "The radiance of the spectrum. The higher the number, \
    the greater the energy that is emitted. Can be thought of as the apparent brightness of the \
    spectrum. Note however that smaller wavelengths carry more energy, thereby skewing the numbers \
    a bit.";
pub const SPECTRUM_WAVELENGTH_EDIT_NOT_SUPPORTED_TOOLTIP: &str = "Editing the wavelength is not \
    yet supported. Currently, only the entire visible spectrum can be used";
pub const SPECTRUM_RIGHT_SLIDER_DISABLED_TOOLTIP: &str = "Editing spectra is not allowed unless \
    their type has been changed to custom. After a spectrum is converted to custom, the number of \
    samples should no longer be changed.";
pub const CUSTOM_SPECTRUM_FACTOR_ADJUST_TOOLTIP: &str = "Multiply the spectrum by this value. The \
    values will only be adjusted upon pressing the Apply button.";


//display
pub const DISPLAY_START_RENDERING_BUTTON_DISABLED_TOOLTIP: &str = "Cannot start rendering right \
    now. Maybe some lights or objects have illegal spectra assigned or you are already rendering \
    something?";
pub const DISPLAY_ABORT_RENDERING_BUTTON_TOOLTIP: &str = "Aborts the current rendering process. \
    The current frame will be finished, which may take a few more seconds.";
pub const DISPLAY_IMAGE_TOOLTIP: &str = "Hold and drag the image to move it. Alternatively, use \
    the mouse scroll wheel to move up and down. Hold down [shift] and scroll to move left and \
    right. Hold down [ctrl] and scroll to zoom in and out.";


//other stuff
pub const EDIT_BUTTON_PENCIL_EMOJI: &str = "✏";
pub const EDIT_BUTTON_TOOLTIP: &str = "Change the name of this element.";
pub const HELP_MENU_LABEL: &str = "For a simple tutorial, see the README.md file. For explanations \
    what the different settings do, hover over them for a short period of time to see their \
    tooltips.";
pub const COPIED_ELEMENT_NAME_INDICATOR: &str = " (copy)";