# Iris

This is a modified version of [`64/iris`](https://github.com/64/iris), targeted to be used for research purposes in a university project. The goal is to verify the effect of hero wavelength spectral sampling (HWSS) as presented by Wilkie et al. (2014) in comparison to single wavelength spectral sampling (SWSS).

Changes to the original include:

* `Cargo.toml` now includes a new features `hwss` (to activate hero wavelength sampling, else single wavelength is used). `openexr` has been removed from dependencies and the `progressive` feature is no longer available.

* Command line arguments have been added in format `<samples per pixel> <file name>`. When no sample number is given, 4 is used as default. When no file name is given, the render will open in progressive mode. Otherwise it will be saved to PNG.

* Saving as PNG file is now default if a file name is given via command line.

* Progressive rendering is now default if no file name is given via command line.

* `PathIntegrator` is a new integrator implementing the most basic forward path tracing without next-event estimation. It is based on the original's `SwssNaive` and `HwssNaive` integrators as well as the [documentation of PBRT to implement a path tracer](https://pbr-book.org/3ed-2018/Light_Transport_I_Surface_Reflection/Path_Tracing).
