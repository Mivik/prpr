#version 130
// Adapted from https://godotshaders.com/shader/pixelate-2/
varying lowp vec2 uv;
uniform sampler2D _ScreenTexture;

uniform float size = 10.0;

void main() {
  vec2 tex_size = textureSize(_ScreenTexture, 0);
  vec2 factor = tex_size / size;
  float x = round(uv.x * factor.x) / factor.x;
  float y = round(uv.y * factor.y) / factor.y;
  gl_FragColor = texture2D(_ScreenTexture, vec2(x, y));
}
