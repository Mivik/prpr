#version 130
// Adapted from https://godotshaders.com/shader/radical-blur-shader/
varying lowp vec2 uv;
uniform sampler2D _ScreenTexture;

uniform float centerX = 0.5;
uniform float centerY = 0.5;
uniform float power = 0.01; // 0..1
uniform float sampleCount = 6; // int, 1..64

void main() {
  vec2 direction = uv - vec2(centerX, centerY);
  vec3 c = vec3(0.0);
  int sample_count = int(round(sampleCount));
  float f = 1.0 / sample_count;
  vec2 screen_uv = uv / 2.0 + vec2(0.5, 0.5);
  for (int i = 0; i < sample_count; ++i) {
    c += texture2D(_ScreenTexture, uv - power * direction * float(i)).rgb * f;
  }
  gl_FragColor.rgb = c;
}
