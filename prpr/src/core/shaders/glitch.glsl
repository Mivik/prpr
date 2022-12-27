#version 130
// Adapted from https://godotshaders.com/shader/glitch-effect-shader/
varying lowp vec2 uv;
uniform sampler2D _ScreenTexture;
uniform float time;

uniform float power = 0.03;
uniform float rate = 0.6; // 0..1
uniform float speed = 5.0;
uniform float blockSize = 30.5;
uniform float colorRate = 0.01; // 0..1

float random(float seed) {
  return fract(543.2543 * sin(dot(vec2(seed, seed), vec2(3525.46, -54.3415))));
}

void main() {
  float enable_shift = float(random(trunc(time * speed)) < rate);

  vec2 fixed_uv = uv;
  fixed_uv.x += (random((trunc(uv.y * blockSize) / blockSize) + time) - 0.5) * power * enable_shift;

  vec4 pixel_color = textureLod(_ScreenTexture, fixed_uv, 0.0);
  pixel_color.r = mix(
    pixel_color.r,
    textureLod( _ScreenTexture, fixed_uv + vec2( colorRate, 0.0 ), 0.0 ).r,
    enable_shift
  );
  pixel_color.b = mix(
    pixel_color.b,
    textureLod( _ScreenTexture, fixed_uv + vec2( -colorRate, 0.0 ), 0.0 ).b,
    enable_shift
  );
  gl_FragColor = pixel_color;
}
