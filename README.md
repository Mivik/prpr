# prpr - <ins>P</ins>hig<ins>R</ins>os <ins>P</ins>layer, written in <ins>R</ins>ust

## Usage

To begin with, clone the repo:

```shell
git clone https://github.com/Mivik/prpr.git && cd prpr
```

For compactness's sake, `font.ttf` used to render the text is not included in this repo. As the fallback, `prpr` will use the default pixel font. You could fetch `font.ttf` from [https://mivik.moe/prpr/font.ttf].

```shell
wget https://mivik.moe/prpr/font.ttf -O assets/font.ttf
```

Then place your chart in the `assets` folder. The folder structure should be like this:

```
prpr
├── assets
|   ├── charts
|   │   └── mychart
|   │       ├── info.yml
|   │       ├── chart.json
|   │       ├── song.mp3
|   │       └── ...
|   ├── texture
|   │   └── ...
|   ├── (font.ttf)
|   └── ...
└── ...
```

That is to say, you should unzip your textures into the `texture` folder, and your chart in the `charts` folder, with `song.mp3` and `chart.json` in it.

Attention! You should create a `info.yml` in the chart folder as well. Its format is elaborated in [this section](#infoyml-format)

Finally, run `prpr` with your chart's name.

```shell
cargo run --release --bin prpr-player mychart
```

## `info.yml` format

Available configurations are listed here:

```yml
title: (string) (default: 'UK')
level: (string) (default: 'UK Lv.?')
charter: (string) (default: 'UK')
composer: (string) (default: 'UK')
illustrator: (string) (default: 'UK')

chart: (string, the path of the chart file) (default: 'chart.json')
format: (string, the format of the chart) (default: 'rpe', available: 'rpe', 'pgr', 'pec')
music: (string, the path of the music file) (default: 'music.mp3')
illustration: (string, the path of the illustration) (default: none)

aggresive: (bool, enable aggresive optimization, may cause inconsistent render result) (default: true)
aspect-ratio: (float, the aspect ratio of the screen (w / h)) (default: 16 / 9)
autoplay: (bool, to enable the auto play mode) (default: true)
line-length: (float, half the length of the judge line) (default: 6)
particle: (bool, should particle be enabled or not) (default: false)
speed: (float, the speed of the chart) (default: 1)
volume-music: (float, the volume of the music) (default: 1)
volume-sfx: (float, the volume of sound effects) (default: 1)
```

## Acknowledgement

Some assets come from [@lchzh3473](https://github.com/lchzh3473).

Thanks [@inokana](https://github.com/GBTP) for hints on implementation!
