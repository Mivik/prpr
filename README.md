# prpr - <ins>P</ins>hig<ins>R</ins>os <ins>P</ins>layer, written in <ins>R</ins>ust

[中文文档](https://mivik.moe/prpr-docs/)

[Resource Pack Collection](https://prprblog.kevin2106.top/)

测试 QQ 群：660488396

## Usage

To begin with, clone the repo:

```shell
git clone https://github.com/Mivik/prpr.git && cd prpr
```

For compactness's sake, `font.ttf` used to render the text is not included in this repo. As the fallback, `prpr` will use the default pixel font. You could fetch `font.ttf` from [https://mivik.moe/prpr/font.ttf].

```shell
wget https://mivik.moe/prpr/font.ttf -O assets/font.ttf
```

Finally, run `prpr` with your chart's path.

```shell
# .pez file can be recognized
cargo run --release --bin prpr-player mychart.pez

# ... or unzipped folder
cargo run --release --bin prpr-player ./mychart/

# Run with configuration file
cargo run --release --bin prpr-player ./mychart/ conf.yml
```

## Chart information

`info.txt` and `info.csv` are supported. But if `info.yml` is provided, the other two will be ignored. 

The specifications of `info.yml` are as below.

```yml
id: (string) (default: none)

name: (string) (default: 'UK')
difficulty: (float) (default: 10)
level: (string) (default: 'UK Lv.?')
charter: (string) (default: 'UK')
composer: (string) (default: 'UK')
illustrator: (string) (default: 'UK')

chart: (string, the path of the chart file) (default: 'chart.json')
format: (string, the format of the chart) (default: 'rpe', available: 'rpe', 'pgr', 'pec')
music: (string, the path of the music file) (default: 'music.mp3')
illustration: (string, the path of the illustration) (default: 'background.png')

previewTime: (float, preview time of the music) (default: 0)
aspectRatio: (float, the aspect ratio of the screen (w / h)) (default: 16 / 9)
lineLength: (float, half the length of the judge line) (default: 6)
tip: (string) (default: 'Tip: 欢迎来到 prpr！')

intro: (string, introduction to this chart) (default: empty)
tags: ([string], tags of this chart) (default: [])
```

## Global configuration

The optional second parameter of `prpr-player` is the path to the configuration file. The specifications are as below.

```yml
adjustTime: (bool, whether automatical time alignment adjustment should be enabled) (default: true)
aggresive: (bool, enables aggresive optimization, may cause inconsistent render result) (default: true)
aspectRatio: (float, overrides the aspect ratio of chart) (default: none)
autoplay: (bool, enables the auto play mode) (default: true)
challengeColor: (enum, the color of the challenge mode badge, one of 'white', 'green', 'blue', 'red', 'golden', 'rainbow') (default: golden)
challengeRank: (int, the rank in the challenge mode badge) (default: 45)
disableEffect: (bool, whether to disable effects) (default: false)
fixAspectRatio: (bool, forces to keep the aspect ratio specified in chart) (default: false)
fxaa: (bool, whether FXAA is enabled) (default: false)
interactive: (bool, whether the GUI is interactive) (default: true)
multipleHint: (bool, whether to highlight notes with the same time) (default: true)
noteScale: (float, scale of note size) (default: 1)
offset: (float, global chart offset) (default: 0)
particle: (bool, should particle be enabled or not) (default: false)
playerName: (string, the name of the player) (default: 'Mivik')
playerRks: (float, the ranking score of the player) (default: 15)
sampleCount: (float, MSAA sampling count) (default: 4)
resPackPath: (string, optional, the path to the custom resource pack (can be folder or ZIP archive)) (default: none)
speed: (float, the speed of the chart) (default: 1)
volumeMusic: (float, the volume of the music) (default: 1)
volumeSfx: (float, the volume of sound effects) (default: 1)
```

## Acknowledgement

Some assets come from [@lchzh3473](https://github.com/lchzh3473).

Thanks [@inokana](https://github.com/GBTP) for hints on implementation!

## License

This project is licensed under [GNU General Public License v3.0](LICENSE).

The resource assets under `assets/respack` are from [https://github.com/MisaLiu/phi-chart-render], and are licensed under [CC BY-NC 4.0](https://creativecommons.org/licenses/by-nc/4.0/). Assets used here are compressed and some of them are resized for easier usage.
