# cgex
cgex is a tool for extracting assets from Macromedia/Adobe Director games. It currently supports:

- *Jönssonligan: Jakten på Mjölner*
- *Jönssonligan: Går på Djupet*
- *Bygg bilar med Mulle Meck*

cgex will extract the textures and audio files from the original game files. By default, cgex will also perform AI upscaling on the texture assets.

## Usage

### Windows

Step 1:
```bash
git clone https://github.com/kottz/cgex
cd cgex
mkdir disc_contents
mkdir output
```
2. Insert game CD or mount your .iso and copy everything on the CD into the `disc_contents` folder.
3. Run the program with `cargo run --release`
4. Assets can then be found in the `output` folder.

### Linux (using Docker)

1. Clone the repository
2. Insert game CD or mount your .iso and copy everything on the CD into the `disc_contents` folder.
3. Create an `output` folder for extracted assets.
4. cd into project folder.
5. Run the docker container with the command below.

```bash
git clone https://github.com/kottz/cgex
cd cgex
mkdir disc_contents
mkdir output
docker run --rm \
  -v ./disc_contents:/input:ro \
  -v ./output:/output \
  -e HOST_UID=$(id -u) \
  -e HOST_GID=$(id -g) \
  kottz/cgex:latest
```

If you want to skip the compression or upscaling step when running cgex with the docker container you can add these environment variables.
```bash
docker run --rm \
  -v ./disc_contents:/input \
  -v ./output:/output \
  -e HOST_UID=$(id -u) \
  -e HOST_GID=$(id -g) \
  -e NO_UPSCALE=true \
  -e NO_COMPRESSION=true \
  kottz/cgex:latest
```

Extracted assets will be placed in the `output` folder, organized by type and game area. Extraction process may take a long time depending on your system.

### Options

cgex will output upscaled and compressed WebP assets by default. Both of these steps can be skipped with the `--no-upscale` and `--no-compression` arguments.
`--no-compression` will skip the WebP compression step and output lossless png files. `--no-upscale` will skip the AI upscaling.
If you use both options at the same time cgex will output the original untouched 640x480 image assets in bmp format.

## Legal

This tool is for personal use only. Ensure you have the right to extract and use game assets in your region.

## TODO
- Provide a pre-compiled executable for Windows.
