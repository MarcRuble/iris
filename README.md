# Iris

CPU path tracer written in Rust.

Features (WIP):
* Spectral rendering (including wavelength-dependent path generation) with [Hero Wavelength Spectral Sampling](https://cgg.mff.cuni.cz/~wilkie/Website/EGSR_14_files/WNDWH14HWSS.pdf)
* Spectral upsampling (Jakob et al.)
* Parallel and progressive refinement
* Russian roulette
* Next event estimation
* Multiple importance sampling
* HDR environment maps

TODO:
* SIMD more things (matmul, vec3, Spectrum eval, upsampling)
* Analytic light integration test (Le = 0.5, f = 0.5, radiance should be 1)
* Unify simd code for Vec4y types (Wavelength, Spectrum, PdfSet)
* More shapes
* Serialize scene from RON
* Write own sobol code
* BVH / other spatial accel
* MTL file handling
* Reconstruction filtering
* Adaptive sampling (?)
* Image / HDR output
* Tonemapping options (ACES)
* Camera lens sim + vigenetting + DoF
* Volume rendering
* Motion blur / animation
* Real time preview
* Own PNG / HDR code
* PGO
* Clean up normal offseting
* MIS compensation
* Triangles
* Fast shadow ray intersection routines
* Coherent ray bundles
* SDF shapes
* Mipmapping / texture filtering
* Catmull-Clark
* Denoising
* Make lighting match up with PBRT
* License
