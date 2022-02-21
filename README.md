# Image of_images
so-manieth repository which contains a tool for generating an image composed of multiple other images


## How to use
```
python make_img_of_imgs.py --in-folder <folder_with_images> --target-img <jpg_image_to_replicate> --out-dir <folder_to_store_results>
```

## Example
A logo composed of images from [Flickr8K](https://www.kaggle.com/adityajn105/flickr8k/activity) dataset.

Generated with:
```
python make_img_of_imgs.py --input-dir data/archive --target-img resources/bch_logo_no_bg.jpg --output-dir resources --target-width 1000
```

The original logo:

<img src="resources/bch_logo_no_bg.jpg" width="200"/>

The logo composed of images:

<img src="resources/result.png" width="800"/>