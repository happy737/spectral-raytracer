# Non-hardware-accelerated Spectral Raytracer
The spectral raytracer is a proof of concept for a raytracer which instead of working with 
simplified RGB colors, emulates the entire spectrum of (visible) light and converts it 
into RGB at the end. This approach is far more expensive calculation-wise but provides 
the opportunity to render more realistic images with effects such as wavelength-based 
refraction, which is responsible f. ex. for the rainbow behind a prism. \
This project uses no raytracing specific libraries, instead fundamentally using only two 
big libraries, _eframe_ - for the graphical user interface and _nalgebra_ - for fast, 
type-safe linear algebra. The implementation is hardware independent, a GPU is neither 
needed nor used. \
The project is currently under development, anything written here is subject to change. 
Many features are yet to be implemented or improved, the raytracing shaders are far from
complete and true to nature, and the UI currently is only a functional placeholder. 

## How do I use the Spectral Raytracer?
### Installing the Raytracer
As of now, the raytracer is only one statically linked executable. If an executable file 
is already provided, then this executable can be placed anywhere in the file system and 
executed. Otherwise, the sourcecode has to be compiled manually via rusts cargo. 
"cargo build -r" will provide the most optimized executable. (To install cargo on your
machine, download and execute _rustup_ for your operating system.) \
In the future, the raytracer will generate a bunch of files where settings and previous
project configurations will be stored, it is therefore recommended to place the executable 
in its own folder. 

### A short tutorial on how to use the Spectral Raytracer
To start the raytracer, simply execute the executable. You will be greeted by the general
settings menu where options such as the dimensions of the generated image as well as the 
number of threads used during the rendering process can be set. The default value will 
fully utilize the CPU. If the computer is to be used for anything else during the duration, 
it is recommended to lower the number by one or two. \
In the second tab called "Objects", objects in the to-be-rendered scene can be placed,
scaled, rotated, etc. This includes visible objects such as a simple sphere, but also 
the position, orientation and FOV of the camera, or the position and spectrum of light 
sources in the scene. \
The third tab "Spectra and Materials" allows the modification of the spectra emitted 
and reflected by light sources and objects, as well as the objects materials, essentially 
the description of their physical reflection behaviours. \
Finally, in the last tab "Display" the image will be displayed as soon as the rendering 
process begins. 

## Understanding the General Architecture of the software
The main data structure of the project is `main::App`. Here every relevant value, such
as the final rendered image, is stored. The program starts in `main::main`. There 
_eframe's_ runtime is started. _eframe_ calls `main::App::update` for every frame update, 
essentially every time the UI has to be redrawn. This defines and manages the UI, which 
in turn has the power to start all other subroutines. \
The most relevant subroutine would be `main::App::dispatch_render`, which consolidates 
the settings, such as object positions, for the image generation from the ui and then
starts the rendering process in another thread. Down the line, the 
`shader::ray_generation_shader` is called for every fragment (pixel), each row being 
calculated in its own thread for maximum parallel performance. 

## Shader structure of the raytracing engine
Just as rasterization image synthesis is split into distinct steps, so-called shaders 
(f. ex. vertex shader, fragment shader, tesselation shader), this raytracer is split into 
multiple shaders as well, based on the Vulkan Raytracing pipeline. Additionally, a normally
non-programmable part of the pipeline, the ray acceleration structure, has to be calculated 
as well. Contrary to the rasterization process however, the shaders are not distinct steps 
happening in a fixed order, but interleave in different orders for each pixel, in general 
being executed multiple times before finally producing a color. \
The general layout of shader execution is as follows: For a fragment at position (x, y),
the Ray Generation Shader is called. This shader generates a ray which is then submitted 
to the Ray Acceleration Structure. This structure determines which objects the ray could
intersect. To be certain, it asks the Intersection Shader, which in turn gives its result 
to the Any Hit shader, which may be used to determine the desired hit. This result is fed 
back to the Ray Acceleration Structure which uses these results to determine whether the
ray hit anything (Hit Shader) or missed (Miss Shader). The Hit Shader may create new rays 
or instruct the Ray Generation Shader to generate more Rays which are then submitted to 
the Ray Acceleration Structure and the cycle continues. 
