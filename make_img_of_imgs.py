from turtle import width
from typing import List
import click
import pathlib
import os
from PIL import Image
import numpy as np
from tqdm import tqdm
import warnings
import random
from joblib import Parallel, delayed
from multiprocessing import cpu_count
from functools import cache


def get_imgs_in_folder(folder: pathlib.Path) -> list:
    img_paths = [f for f in folder.glob("**/*.jpg")] + [
        f for f in folder.glob("**/*.png")
    ]
    return [str(f) for f in img_paths]


def transform_resize_img(img: Image.Image, width: int, height: int) -> Image.Image:
    im_width, im_height = img.size

    width_scale = width / im_width
    height_scale = height / im_height

    scale = max(width_scale, height_scale)

    img = img.resize((int(im_width * scale), int(im_height * scale)))

    im_width, im_height = img.size

    left = (im_width - width) // 2
    top = (im_height - height) // 2
    right = (im_width + width) // 2
    bottom = (im_height + height) // 2

    img = img.crop((left, top, right, bottom))

    return img


def load_and_resize_images(
    img_paths: list, width: int, height: int
) -> List[np.ndarray]:
    imgs = []
    for img_path in tqdm(img_paths):
        with Image.open(img_path) as img:

            img = img.convert("RGBA")
            img = transform_resize_img(img, width, height)

            img = np.asarray(img).astype(np.float32)

            if list(img.shape) == [height, width, 4]:
                imgs.append(img)
            else:
                warnings.warn(f"{img_path} has wrong shape: {img.shape}")

    return imgs


def load_and_resize_target_img(target_img_path, width) -> np.ndarray:
    with Image.open(target_img_path) as img:
        im_width, _ = img.size

        scale = width / im_width

        img = img.convert("RGBA")
        img = img.resize((int(im_width * scale), int(im_width * scale)))
        img = np.asarray(img).astype(np.float32)
        return img


def squared_error(img1, img2):
    return np.mean((img1 - img2) ** 2)


def fill_target_img(
    target_img: np.ndarray,
    imgs: List[np.ndarray],
    sub_img_width,
    sub_img_height,
    pop_img=True,
) -> np.ndarray:
    target_img = target_img.copy()

    n_width = target_img.shape[1] // sub_img_width
    n_height = target_img.shape[0] // sub_img_height

    n_pad_width = target_img.shape[1] % sub_img_width
    n_pad_height = target_img.shape[0] % sub_img_height

    target_img = target_img[
        n_pad_height // 2 : -n_pad_height // 2, n_pad_width // 2 : -n_pad_width // 2, :
    ]

    height_idxs = list(range(n_height))
    width_idxs = list(range(n_width))

    random.shuffle(height_idxs)
    for i in tqdm(height_idxs):
        random.shuffle(width_idxs)
        for j in width_idxs:
            y_start = i * sub_img_height
            y_end = y_start + sub_img_height

            x_start = j * sub_img_width
            x_end = x_start + sub_img_width

            patch = target_img[y_start:y_end, x_start:x_end, :]

            errors = [squared_error(patch, img) for img in imgs]
            # errors = Parallel(n_jobs=cpu_count())(delayed(squared_error)(patch, img) for img in imgs)
            min_error_idx = np.argmin(errors)

            img = imgs[min_error_idx]

            if pop_img:
                imgs[min_error_idx] = imgs[-1]
                imgs.pop()

            target_img[y_start:y_end, x_start:x_end, :] = img

    return target_img


@click.command()
@click.option("--input-dir", type=click.Path(exists=True))
@click.option("--target-img", type=click.Path(exists=True))
@click.option("--output-dir", type=click.Path())
@click.option("--target-width", type=int, default=10000)
@click.option("--num-horizontal-imgs", type=int, default=30)
@click.option("--num-vertical-imgs", type=int, default=60)
@click.option("--max-imgs", type=int, default=None)
@click.option("--do-pop/--no-pop", type=bool, default=True)
def main(
    input_dir: str,
    target_img: str,
    output_dir: str,
    target_width: int,
    num_horizontal_imgs: int,
    num_vertical_imgs: int,
    max_imgs: int,
    do_pop: bool,
):
    input_dir = pathlib.Path(input_dir)
    output_dir = pathlib.Path(output_dir)
    output_dir.mkdir(exist_ok=True)

    target_img = load_and_resize_target_img(target_img, width=target_width)

    img_width = target_img.shape[1] // num_horizontal_imgs
    img_height = target_img.shape[0] // num_vertical_imgs

    imgs = get_imgs_in_folder(input_dir)
    if max_imgs is not None:
        imgs = random.sample(imgs, max_imgs)

    imgs = load_and_resize_images(imgs, width=img_width, height=img_height)

    result = fill_target_img(
        target_img,
        imgs,
        sub_img_width=img_width,
        sub_img_height=img_height,
        pop_img=do_pop,
    ).astype(np.uint8)
    result = Image.fromarray(result)

    result.save(output_dir / "result.png")


if __name__ == "__main__":
    main()
