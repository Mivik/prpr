#version 100
// Adapted from https://godotshaders.com/shader/pixelate-2/
precision mediump float;

varying lowp vec2 uv;
uniform vec2 screenSize;
uniform sampler2D screenTexture;

uniform float size; // %10.0%

void main() {
  vec2 factor = screenSize / size;
  float x = round(uv.x * factor.x) / factor.x;
  float y = round(uv.y * factor.y) / factor.y;
  gl_FragColor = texture2D(screenTexture, vec2(x, y));
}
