#version 130
// Adapted from https://godotshaders.com/shader/artsy-circle-blur-type-thingy/
varying lowp vec2 uv;
uniform sampler2D _ScreenTexture;

uniform float size = 10.0;

void main() {
  vec4 c = textureLod(_ScreenTexture, uv, 0.0);
  float length = dot(c, c);
  vec2 pixel_size = 1.0 / textureSize(_ScreenTexture, 0);
  for (float x = -size; x < size; x++) {
    for (float y = -size; y < size; ++y) {
      if (x * x + y * y > size * size) continue;
      vec4 new_c = texture2D(_ScreenTexture, uv + pixel_size * vec2(x, y));
      float new_length = dot(new_c, new_c);
      if (new_length > length) {
        length = new_length;
        c = new_c;
      }
    }
  }
  gl_FragColor = c;
}
