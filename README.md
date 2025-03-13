# Non-hardware-accelerated wavelength based Ray Tracer
//TODO

## Shaders
Just as rasterization image synthesis is split into distinct steps, so-called shaders 
(f. ex. vertex shader, fragment shader, tesselation shader), this ray-tracer is split into 
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

### Ray Generation Shader
### Hit Shader
### Miss Shader
### Intersection Shader
### Any Hit Shader
### Ray Acceleration Structure