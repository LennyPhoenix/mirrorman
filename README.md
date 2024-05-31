# MirrorMan

**MirrorMan** aims to provide a seamless solution for mirroring and converting a large hierarchy of files.

You might use this for:

- Converting your lossless audio library to smaller MP3s to be taken on the go (see [`example_filter.sh`](./example_filter.sh))
- Upscaling footage from your super-duper old video camera and converting into a reasonable format
- Turning your SD card full of raw images into the things normal people can view

## Usage

To make a new mirror: `mirrorman init {source} {mirror_path} [filters...]`

To sync existing mirrors, from within a directory with `.mmdb` files: `mirrorman sync`

## Filters

Filters are the core of the conversion side of things.

They tell `mirrorman` if a file should be converted and the new file extension after conversion.

(They also perform the actual important conversion part.)

A filter is just an executable program that has two operation modes:

- `{filter} ext {input_extension}` -> `output_extension`: Prints the desired extension, or returns an error code if the filter doesn't care about the input file.
- `{filter} run {input} {ouput}`: Converts the input file to the output file.

It's really that simple!

Refer to [the example filter](./example_filter.sh) for specifics.

## Todo

- Use timestamp comparisons before hash comparisons on database
- Better way of a filter ignoring a file, error codes should ideally be used for errors not passing info
