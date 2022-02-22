# Image of images
so-manieth repository which contains a tool for generating an image composed of multiple other images

## How to use

### Use the user interface
Download one of the binaries and execute it:
| Platform | Download |
|--------|-------|
| Linux   | [image_of_images_gui_linux.zip](http://nilsgolembiewski.nl:8060/public_files/image_of_images_gui_linux.zip)  |
| Windows (not tested)  | [image_of_images_gui_windows.zip](http://nilsgolembiewski.nl:8060/public_files/image_of_images_gui_windows.zip)  |
<!-- | Apple (failed) | Build failed -->

Or compile & run using cargo:
```
cargo run -p image_of_images_gui --release
```

### Or use the cli
```
cargo run -p image_of_images_cli --release -- --in-folder <folder_with_images> --target-img <jpg_image_to_replicate> --out-dir <folder_to_store_results>
```

## Example
A logo composed of images from [Flickr8K](https://www.kaggle.com/adityajn105/flickr8k/activity) dataset.

Generated with:
```
cargo run -p image_of_images_cli --release -- --input-dir data/archive --target-img resources/bch_logo_no_bg.jpg --output-dir resources
```

The original logo:

<img src="resources/bch_logo_no_bg.jpg" width="200"/>

The logo composed of images:

<img src="resources/result.png" width="800"/>
